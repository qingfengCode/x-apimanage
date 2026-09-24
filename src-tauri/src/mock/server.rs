use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::db::repos::mock_route::MockRoute;

/// 运行时路由表（server 读取，由命令刷新）
pub type SharedRoutes = Arc<Mutex<Vec<MockRoute>>>;

/// 请求日志环形缓冲（最近 MAX_LOGS 条，含未命中 404 的请求）
pub type SharedLogs = Arc<std::sync::Mutex<Vec<MockLogEntry>>>;
const MAX_LOGS: usize = 200;

/// 单条 Mock 命中日志
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MockLogEntry {
    pub id: String,
    /// 请求到达时间（毫秒）
    pub ts: i64,
    pub method: String,
    pub path: String,
    /// 响应状态码（未命中 = 404）
    pub status: u16,
    /// 命中的路由模式（None = 未命中任何路由）
    pub matched_pattern: Option<String>,
    pub route_id: Option<String>,
    pub delay_ms: i64,
}

fn push_log(logs: &SharedLogs, entry: MockLogEntry) {
    // 锁中毒/满时丢弃即可，不能影响响应
    if let Ok(mut buf) = logs.lock() {
        buf.push(entry);
        let overflow = buf.len().saturating_sub(MAX_LOGS);
        if overflow > 0 {
            buf.drain(..overflow);
        }
    }
}

/// 运行中的 server 句柄
pub struct MockServer {
    pub addr: SocketAddr,
    pub cancel: CancellationToken,
    pub handle: JoinHandle<()>,
}

/// 单个响应头（与 DB JSON 对应）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HeaderPair {
    pub key: String,
    pub value: String,
}

/// 启动 mock server。返回绑定地址。
pub async fn start(
    port: u16,
    routes: SharedRoutes,
    logs: SharedLogs,
) -> std::io::Result<(SocketAddr, CancellationToken, JoinHandle<()>)> {
    let cancel = CancellationToken::new();
    let cancel_for_listener = cancel.clone();

    // 显式构造 TcpListener 以便在端口冲突时给出明确错误
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    let addr = listener.local_addr()?;

    let app = Router::new()
        // 捕获任意方法和路径
        .route("/*path", any(handler))
        .route("/", any(handler))
        .with_state(AppState { routes, logs });

    let handle = tokio::spawn(async move {
        // 用 cancel 包裹 accept 循环
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                cancel_for_listener.cancelled().await;
            })
            .await
            .ok();
    });

    Ok((addr, cancel, handle))
}

#[derive(Clone)]
struct AppState {
    routes: SharedRoutes,
    logs: SharedLogs,
}

async fn handler(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Response<Body> {
    // 不用 Path 提取器：注册了 "/" 静态路由时，Path<String> 会因缺少捕获段
    // 直接 500（“Expected 1 but got 0”），根路径永远无法命中 mock 路由。
    // 这里直接从 URI 取路径（axum 已剔除 query），根路径 "/" 正常进入匹配。
    let method = req.method().clone();
    let raw_path = req.uri().path().to_string();
    // 规范化路径：保证以 / 开头
    let path = if raw_path.starts_with('/') {
        raw_path
    } else {
        format!("/{raw_path}")
    };

    // 匹配后克隆数据立即放锁：sleep 延迟期间不持有共享路由锁，
    // 否则所有并发 mock 请求会被串行化，refresh/start/stop 也会被阻塞
    let matched = {
        let routes = state.routes.lock().await;
        find_route(&routes, &method, &path).cloned()
    };

    let Some(route) = matched else {
        push_log(
            &state.logs,
            MockLogEntry {
                id: uuid::Uuid::new_v4().to_string(),
                ts: chrono::Utc::now().timestamp_millis(),
                method: method.as_str().to_string(),
                path: path.clone(),
                status: 404,
                matched_pattern: None,
                route_id: None,
                delay_ms: 0,
            },
        );
        return Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("Content-Type", "application/json")
            .body(Body::from(
                r#"{"error":"no mock route matched","hint":"check method/path in mock manager"}"#,
            ))
            .unwrap();
    };

    // 延迟
    if route.delay_ms > 0 {
        tokio::time::sleep(std::time::Duration::from_millis(route.delay_ms as u64)).await;
    }

    // 解析自定义响应头
    let mut headers = HeaderMap::new();
    let parsed: Vec<HeaderPair> = serde_json::from_str(&route.response_headers).unwrap_or_default();
    let mut content_type_val: Option<String> = None;
    for h in parsed {
        if h.key.eq_ignore_ascii_case("content-type") {
            content_type_val = Some(h.value.clone());
        }
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(h.key.as_bytes()),
            HeaderValue::from_str(&h.value),
        ) {
            headers.append(name, val);
        }
    }
    if content_type_val.is_none() {
        headers.insert(
            HeaderName::from_static("content-type"),
            HeaderValue::from_static("application/json"),
        );
    }

    // body：JSON 响应（默认或显式 json Content-Type）下非对象/数组文本包成 JSON 字符串，
    // 其它 Content-Type（text/plain、html 等）原样返回，不强制加引号
    let ct_is_json = content_type_val
        .as_deref()
        .map(|v| v.to_ascii_lowercase().contains("json"))
        .unwrap_or(true); // 未设置时上方默认 application/json
    let body_trimmed = route.response_body.trim_start();
    let body_bytes = if !ct_is_json {
        route.response_body.clone().into_bytes()
    } else if body_trimmed.starts_with('{') || body_trimmed.starts_with('[') {
        route.response_body.clone().into_bytes()
    } else {
        serde_json::to_vec(&serde_json::Value::String(route.response_body.clone()))
            .unwrap_or_default()
    };

    let status = StatusCode::from_u16(route.status as u16).unwrap_or(StatusCode::OK);
    // 记录命中日志（状态码按配置记录，即使非法回退 200 也按实际响应记录）
    push_log(
        &state.logs,
        MockLogEntry {
            id: uuid::Uuid::new_v4().to_string(),
            ts: chrono::Utc::now().timestamp_millis(),
            method: method.as_str().to_string(),
            path,
            status: status.as_u16(),
            matched_pattern: Some(route.path.clone()),
            route_id: Some(route.id.clone()),
            delay_ms: route.delay_ms,
        },
    );
    Response::builder()
        .status(status)
        .body(Body::from(body_bytes))
        .map(|mut r| {
            let h = r.headers_mut();
            for (k, v) in headers.iter() {
                h.append(k.clone(), v.clone());
            }
            r
        })
        .unwrap()
}

/// 路由匹配：精确方法 + 路径匹配（支持 `:param` 占位与 `**` 通配）
fn find_route<'a>(routes: &'a [MockRoute], method: &Method, path: &str) -> Option<&'a MockRoute> {
    let method_s = method.as_str().to_uppercase();
    let target_segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();

    for r in routes {
        if !r.enabled {
            continue;
        }
        let m = r.method.to_uppercase();
        if m != method_s && m != "ANY" {
            continue;
        }
        if path_match(&r.path, &target_segments) {
            return Some(r);
        }
    }
    None
}

/// 简易路径匹配：支持 `:name`/`*`（单段）和 `**`（任意多段，可出现在中间）
fn path_match(pattern: &str, target: &[&str]) -> bool {
    // 去掉 query
    let pattern = pattern.split('?').next().unwrap_or(pattern);
    let pat_segments: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    match_segments(&pat_segments, target)
}

fn match_segments(pat: &[&str], target: &[&str]) -> bool {
    if pat.is_empty() {
        return target.is_empty();
    }
    let p = pat[0];
    if p == "**" {
        // `**` 匹配任意数量段（含 0 段）：枚举所有可能的跳过位置回溯匹配
        for skip in 0..=target.len() {
            if match_segments(&pat[1..], &target[skip..]) {
                return true;
            }
        }
        return false;
    }
    // 单段通配或字面量匹配
    let single_ok = !target.is_empty()
        && (p == "*" || p.starts_with(':') || p == target[0]);
    if single_ok {
        return match_segments(&pat[1..], &target[1..]);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::path_match;

    fn segs(path: &str) -> Vec<&str> {
        path.split('/').filter(|s| !s.is_empty()).collect()
    }

    #[test]
    fn literal_and_single_segment_patterns() {
        assert!(path_match("/", &segs("/")));
        assert!(path_match("/api", &segs("/api")));
        assert!(!path_match("/api", &segs("/api/x")));
        assert!(path_match("/api/:id", &segs("/api/123")));
        assert!(!path_match("/api/:id", &segs("/api/123/x")));
        assert!(path_match("/api/*", &segs("/api/x")));
        // pattern 带 query 时忽略 query 部分
        assert!(path_match("/api/x?foo=1", &segs("/api/x")));
    }

    #[test]
    fn double_wildcard_requires_trailing_segments() {
        // `/api/**/detail` 不应匹配缺少 detail 段的路径（旧实现的缺陷）
        assert!(!path_match("/api/**/detail", &segs("/api/users")));
        assert!(!path_match("/api/**/detail", &segs("/api/users/posts")));
        // 尾段为 detail 时命中（可跨多段）
        assert!(path_match("/api/**/detail", &segs("/api/users/detail")));
        assert!(path_match("/api/**/detail", &segs("/api/a/b/detail")));
        // 尾部 `**` 通配任意剩余段（含 0 段）
        assert!(path_match("/api/**", &segs("/api")));
        assert!(path_match("/api/**", &segs("/api/a/b/c")));
        // 中段 `**` 可匹配 0 段
        assert!(path_match("/a/**/b/c", &segs("/a/b/c")));
    }
}
