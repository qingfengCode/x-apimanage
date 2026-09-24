//! 应用级设置：出站 HTTP 代理。
//!
//! 代理影响所有出站请求（发请求 / AI 对话 / MCP 工具 / 应用自更新），
//! 因此保存后要立刻重建共享客户端，不能等到下次启动。

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::State;

use crate::db::repos::app_setting;
use crate::db::SharedDb;
use crate::error::{AppError, AppResult};
use crate::http::engine::HttpClients;
use crate::http::proxy::{self, ProxySettings};

/// 代理连通性测试的超时上限
const TEST_TIMEOUT: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTestResult {
    pub status: u16,
    pub time_ms: u64,
    /// 实际请求的地址（回显给用户确认目标没填错）
    pub url: String,
}

/// 读取代理设置
#[tauri::command]
pub fn get_proxy_settings(db: State<'_, SharedDb>) -> AppResult<ProxySettings> {
    let conn = db.lock().expect("db mutex poisoned");
    Ok(app_setting::get(&conn.conn)?.proxy)
}

/// 保存代理设置：校验 → 落库 → 更新进程内配置 → 重建共享客户端。
#[tauri::command]
pub fn save_proxy_settings(
    db: State<'_, SharedDb>,
    http: State<'_, HttpClients>,
    settings: ProxySettings,
) -> AppResult<()> {
    let settings = proxy::normalize(&settings)?;
    {
        let conn = db.lock().expect("db mutex poisoned");
        let mut current = app_setting::get(&conn.conn)?;
        current.proxy = settings.clone();
        app_setting::save(&conn.conn, &current)?;
    }
    proxy::set(settings);
    // 重建失败不还原配置：落库的新值下次启动生效，本次沿用旧客户端
    http.rebuild()
}

/// 用「待保存」的代理配置试发一个 GET，验证地址/端口/账号是否正确。
/// 不落库、不影响当前生效配置，便于用户先试后存。
#[tauri::command]
pub async fn test_proxy(settings: ProxySettings, url: String) -> AppResult<ProxyTestResult> {
    let settings = proxy::normalize(&settings)?;
    let target = reqwest::Url::parse(url.trim())
        .map_err(|e| AppError::InvalidUrl(format!("测试地址无效：{e}")))?;

    let mut builder = reqwest::Client::builder()
        .user_agent("x-apimanage/proxy-check")
        .timeout(TEST_TIMEOUT);
    // 未启用代理时同样发请求：直连也失败就说明问题不在代理
    if proxy::active_url(&settings).is_some() {
        builder = builder.proxy(proxy::build(&settings)?);
    }
    let client = builder.build().map_err(|e| AppError::Http(e.to_string()))?;

    let start = Instant::now();
    let resp = client.get(target.clone()).send().await?;
    let time_ms = start.elapsed().as_millis() as u64;

    Ok(ProxyTestResult {
        status: resp.status().as_u16(),
        time_ms,
        url: target.to_string(),
    })
}

/// 启动时把持久化的代理配置装载进全局（供 lib.rs 在构造客户端前调用）
pub fn load_proxy_from_db(db: &SharedDb) {
    let stored = {
        let Ok(conn) = db.lock() else { return };
        app_setting::get(&conn.conn)
            .map(|s| s.proxy)
            .unwrap_or_default()
    };
    proxy::load_stored(stored);
}
