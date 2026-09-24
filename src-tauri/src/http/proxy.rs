//! 全局 HTTP 代理配置。
//!
//! 应用内所有出站请求（发送请求 / AI 对话 / MCP 工具 / 应用自更新）共用一份代理配置：
//! 持久化在 `app_settings` 表，进程内缓存于全局变量，客户端构造时统一经 [`apply`] 注入。
//! 配置变更后靠 [`rev`] 让按需缓存的自建客户端（见 [`cached`]）失效重建。

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, RwLock};

use reqwest::ClientBuilder;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 本机地址默认直连。开启代理后访问本地 Mock 服务 / 本地 Ollama 时，
/// 若也走代理会直接失败，是最常见的踩坑点，因此无需用户配置也始终放行。
const LOCAL_BYPASS: &str = "localhost,127.0.0.1,::1";

/// 应用级代理配置（前端 `ProxySettings`）
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxySettings {
    pub enabled: bool,
    /// 代理地址，如 http://127.0.0.1:7890、socks5://127.0.0.1:1080
    /// （未写协议时按 http 处理，保存时会补全后落库）
    pub url: String,
    /// 直连名单（逗号分隔），支持域名后缀、IP、CIDR 与 `*`
    pub bypass: String,
}

/// 实际生效的代理地址：未启用或地址为空时为 None
pub fn active_url(cfg: &ProxySettings) -> Option<&str> {
    let url = cfg.url.trim();
    if cfg.enabled && !url.is_empty() {
        Some(url)
    } else {
        None
    }
}

/// 校验并规范化配置。
/// 未启用代理时不校验地址（允许先填一半再开关），启用时补全 scheme 并校验协议/主机。
pub fn normalize(cfg: &ProxySettings) -> AppResult<ProxySettings> {
    let mut out = cfg.clone();
    out.url = out.url.trim().to_string();
    out.bypass = out.bypass.trim().to_string();
    if out.enabled {
        out.url = normalize_url(&out.url)?;
    }
    Ok(out)
}

/// 补全 scheme 并校验代理地址（用户习惯只填 `127.0.0.1:7890`）。
/// 只做格式校验，不发起任何网络请求。
pub fn normalize_url(raw: &str) -> AppResult<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(AppError::Other("代理地址不能为空".into()));
    }
    let candidate = if raw.contains("://") {
        raw.to_string()
    } else {
        format!("http://{raw}")
    };
    let parsed = reqwest::Url::parse(&candidate)
        .map_err(|e| AppError::Other(format!("代理地址无效：{e}")))?;
    match parsed.scheme() {
        "http" | "https" | "socks4" | "socks4a" | "socks5" | "socks5h" => {}
        other => {
            return Err(AppError::Other(format!(
                "不支持的代理协议 {other}（可用 http、https、socks5）"
            )))
        }
    }
    if parsed.host_str().is_none() {
        return Err(AppError::Other("代理地址缺少主机名".into()));
    }
    Ok(candidate)
}

/// 构造 reqwest 代理对象（含直连名单）。
/// 单独暴露给连通性测试用——测试要试的是「待保存」的配置，而不是已生效的全局配置。
pub fn build(cfg: &ProxySettings) -> AppResult<reqwest::Proxy> {
    let url = active_url(cfg)
        .ok_or_else(|| AppError::Other("未启用代理或代理地址为空".into()))?;
    let proxy = reqwest::Proxy::all(url)
        .map_err(|e| AppError::Other(format!("代理地址无效：{e}")))?;
    Ok(proxy.no_proxy(reqwest::NoProxy::from_string(&merged_bypass(&cfg.bypass))))
}

/// 把当前生效的代理配置注入客户端构造器
pub fn apply(builder: ClientBuilder) -> AppResult<ClientBuilder> {
    let cfg = get();
    match active_url(&cfg) {
        Some(_) => Ok(builder.proxy(build(&cfg)?)),
        // 未启用代理时不注入：reqwest 默认行为（按环境变量判断）保持不变
        None => Ok(builder),
    }
}

/// 本机默认直连名单 + 用户直连名单
fn merged_bypass(user: &str) -> String {
    let user = user.trim().trim_matches(',').trim();
    if user.is_empty() {
        LOCAL_BYPASS.to_string()
    } else {
        format!("{LOCAL_BYPASS},{user}")
    }
}

// ---- 进程内全局配置 ----

static CONFIG: OnceLock<RwLock<ProxySettings>> = OnceLock::new();
static REV: AtomicU64 = AtomicU64::new(0);

fn slot() -> &'static RwLock<ProxySettings> {
    CONFIG.get_or_init(|| RwLock::new(ProxySettings::default()))
}

/// 当前生效的代理配置
pub fn get() -> ProxySettings {
    slot().read().expect("proxy config poisoned").clone()
}

/// 写入全局配置（须先经 [`normalize`]），并递增版本号使已缓存的客户端失效
pub fn set(cfg: ProxySettings) {
    *slot().write().expect("proxy config poisoned") = cfg;
    REV.fetch_add(1, Ordering::SeqCst);
}

/// 启动时装载持久化配置：非法配置只告警不阻断启动（返回是否生效）
pub fn load_stored(cfg: ProxySettings) -> bool {
    match normalize(&cfg) {
        Ok(cfg) => {
            set(cfg);
            true
        }
        Err(e) => {
            eprintln!("[proxy] 已保存的代理配置无效，本次启动不使用代理：{e}");
            false
        }
    }
}

/// 代理配置版本号：数值变化即代表需要重建缓存的客户端
pub fn rev() -> u64 {
    REV.load(Ordering::SeqCst)
}

/// 测试专用的全局锁：代理配置是进程级单例，用例并行跑会互相污染。
#[cfg(test)]
pub(crate) fn global_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

/// 按需构建并缓存客户端：代理配置未变时复用（保留连接池），变了才重建。
/// 供 updater 这类自持客户端的模块使用。
pub fn cached(
    slot: &'static OnceLock<Mutex<Option<(u64, reqwest::Client)>>>,
    configure: impl FnOnce(ClientBuilder) -> ClientBuilder,
) -> AppResult<reqwest::Client> {
    let mut guard = slot
        .get_or_init(|| Mutex::new(None))
        .lock()
        .expect("http client cache poisoned");
    if let Some((cached_rev, client)) = guard.as_ref() {
        if *cached_rev == rev() {
            return Ok(client.clone());
        }
    }
    let client = apply(configure(reqwest::Client::builder()))?
        .build()
        .map_err(|e| AppError::Http(e.to_string()))?;
    *guard = Some((rev(), client.clone()));
    Ok(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, url: &str, bypass: &str) -> ProxySettings {
        ProxySettings {
            enabled,
            url: url.into(),
            bypass: bypass.into(),
        }
    }

    #[test]
    fn bare_host_port_defaults_to_http() {
        assert_eq!(
            normalize_url("127.0.0.1:7890").unwrap(),
            "http://127.0.0.1:7890"
        );
        assert_eq!(
            normalize_url("  proxy.corp.com:8080 ").unwrap(),
            "http://proxy.corp.com:8080"
        );
    }

    #[test]
    fn explicit_schemes_are_kept() {
        for url in [
            "http://127.0.0.1:7890",
            "https://proxy.corp.com:8443",
            "socks5://127.0.0.1:1080",
            "socks5h://127.0.0.1:1080",
            "http://user:pass@proxy.corp.com:8080",
        ] {
            assert_eq!(normalize_url(url).unwrap(), url);
        }
    }

    #[test]
    fn unsupported_scheme_and_empty_url_are_rejected() {
        assert!(normalize_url("ftp://127.0.0.1:21").is_err());
        assert!(normalize_url("").is_err());
        // 只给了协议没有主机
        assert!(normalize_url("http://").is_err());
    }

    #[test]
    fn disabled_config_skips_url_validation() {
        // 未启用时允许地址是半成品，方便用户先填后开
        let out = normalize(&cfg(false, "  ", "  ")).unwrap();
        assert_eq!(out.url, "");
        assert_eq!(out.bypass, "");
        assert_eq!(active_url(&out), None);
        // 启用后同样内容必须报错
        assert!(normalize(&cfg(true, "", "")).is_err());
    }

    #[test]
    fn enabled_config_trims_and_normalizes() {
        let out = normalize(&cfg(true, " 127.0.0.1:7890 ", " .corp.com ")).unwrap();
        assert_eq!(out.url, "http://127.0.0.1:7890");
        assert_eq!(out.bypass, ".corp.com");
    }

    #[test]
    fn local_addresses_are_always_bypassed() {
        assert_eq!(merged_bypass(""), LOCAL_BYPASS);
        assert_eq!(merged_bypass(" , "), LOCAL_BYPASS);
        assert_eq!(
            merged_bypass(".corp.com, 10.0.0.0/8"),
            format!("{LOCAL_BYPASS},.corp.com, 10.0.0.0/8")
        );
    }

    #[test]
    fn active_url_requires_enabled_non_empty_url() {
        assert!(active_url(&cfg(true, "http://127.0.0.1:7890", "")).is_some());
        assert!(active_url(&cfg(false, "http://127.0.0.1:7890", "")).is_none());
        assert!(active_url(&cfg(true, "   ", "")).is_none());
    }

    #[test]
    fn set_bumps_rev_and_exposes_config() {
        let _g = global_lock();
        let before = rev();
        set(normalize(&cfg(true, "127.0.0.1:7890", "")).unwrap());
        assert!(rev() > before);
        assert_eq!(get().url, "http://127.0.0.1:7890");
        // 复位，避免影响同进程内其它用例
        set(ProxySettings::default());
    }

    #[test]
    fn load_stored_ignores_invalid_config() {
        let _g = global_lock();
        set(ProxySettings::default());
        assert!(!load_stored(cfg(true, "", "")));
        assert_eq!(active_url(&get()), None);
    }
}
