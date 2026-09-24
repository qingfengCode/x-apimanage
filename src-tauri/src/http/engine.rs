use std::sync::{Arc, RwLock as StdRwLock};
use std::time::{Duration, Instant};

use base64::Engine as _;
use reqwest::{redirect::Policy, Client, Method};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};

use crate::error::{AppError, AppResult};

use super::model::{HttpRequest, HttpResponse, KeyValue, RequestBody, ResponseBody};
use super::proxy;

const USER_AGENT: &str = "x-apimanage/0.1";

/// 按当前代理配置构造客户端。
/// `redirect` / `timeout` 为 None 时用 reqwest 默认（跟随重定向、超时由单请求覆盖）。
fn build_client_inner(redirect: Option<Policy>, timeout: Option<Duration>) -> AppResult<Client> {
    let mut builder = Client::builder()
        .cookie_store(true)
        .user_agent(USER_AGENT)
        .gzip(true)
        .brotli(true)
        .deflate(true);
    if let Some(policy) = redirect {
        builder = builder.redirect(policy);
    }
    if let Some(t) = timeout {
        builder = builder.timeout(t);
    }
    proxy::apply(builder)?
        .build()
        .map_err(|e| AppError::Http(e.to_string()))
}

/// 构建共享的 reqwest 客户端（带 cookie store，走当前代理配置）
pub fn build_client() -> AppResult<Client> {
    build_client_inner(None, None)
}

/// 共享 HTTP 客户端句柄。
/// reqwest 客户端一旦构造就无法改代理，因此代理设置变更时整体重建，
/// 让所有出站请求（发请求 / AI 对话 / MCP 工具）立即走新代理。
#[derive(Clone)]
pub struct HttpClients(Arc<StdRwLock<Client>>);

impl HttpClients {
    pub fn new(client: Client) -> Self {
        Self(Arc::new(StdRwLock::new(client)))
    }

    /// 取出当前客户端（reqwest::Client 内部是 Arc，clone 很廉价且共享连接池）
    pub fn get(&self) -> Client {
        self.0.read().expect("http client poisoned").clone()
    }

    /// 按当前代理配置重建。会丢弃 cookie store（已建立的会话 Cookie 需重新获取）。
    pub fn rebuild(&self) -> AppResult<()> {
        let client = build_client()?;
        *self.0.write().expect("http client poisoned") = client;
        Ok(())
    }
}

/// 执行一次 HTTP 请求
pub async fn execute(client: &Client, req: HttpRequest) -> AppResult<HttpResponse> {
    // 构建带 query 的 URL
    let url = reqwest::Url::parse(&req.url).map_err(|e| AppError::InvalidUrl(e.to_string()))?;

    let enabled_params: Vec<(&str, &str)> = req
        .params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| (p.key.as_str(), p.value.as_str()))
        .collect();
    let mut url = url;
    if !enabled_params.is_empty() {
        let mut pairs = url.query_pairs_mut();
        for (k, v) in &enabled_params {
            pairs.append_pair(k, v);
        }
    }
    let final_url = url.clone();

    // 构建方法
    let method = parse_method(&req.method);

    // 构建请求
    let timeout = std::time::Duration::from_millis(req.timeout_ms.unwrap_or(30_000));
    let follow = req.follow_redirects.unwrap_or(true);

    let mut request = if follow {
        client
            .request(method, final_url.clone())
            .timeout(timeout)
    } else {
        // follow_redirects=false 时需要用独立 client（reqwest 单请求无法覆盖重定向策略，
        // 这里用 Policy::none 临时构造一个，同样走当前代理）
        let no_redirect_client = build_client_inner(Some(Policy::none()), Some(timeout))?;
        no_redirect_client
            .request(method, final_url.clone())
            .timeout(timeout)
    };

    // headers
    // multipart body 的 Content-Type 必须由引擎注入（含 boundary），
    // 用户手动设置的同名头会被 append 成重复头，这里直接剔除
    let is_multipart = matches!(req.body, Some(RequestBody::Multipart { .. }));
    let mut headers = build_headers(&req.headers);
    if is_multipart {
        headers.remove(reqwest::header::CONTENT_TYPE);
    }
    // 用户是否手动设置了 Content-Type（raw body 注入 mime 时避让，防止重复头）
    let has_content_type = headers.contains_key(reqwest::header::CONTENT_TYPE);
    if !headers.is_empty() {
        request = request.headers(headers);
    }

    // body
    if let Some(body) = &req.body {
        request = apply_body(request, body, has_content_type)?;
    }

    // 发送并计时
    let start = Instant::now();
    let resp = request.send().await?;
    let time_ms = start.elapsed().as_millis() as u64;

    // 收集响应
    let status = resp.status().as_u16();
    let status_text = resp
        .status()
        .canonical_reason()
        .unwrap_or("")
        .to_string();

    let mut headers_out: Vec<(String, String)> = Vec::new();
    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let content_type_lower = content_type.to_ascii_lowercase();
    for (k, v) in resp.headers().iter() {
        if let Ok(val) = v.to_str() {
            headers_out.push((k.as_str().to_string(), val.to_string()));
        }
    }

    let size = resp.content_length().unwrap_or(0);

    // 按内容类型决定 text / binary
    let bytes = resp.bytes().await?;
    let size = if size == 0 {
        bytes.len() as u64
    } else {
        size
    };

    let body = if is_textual(&content_type_lower, &bytes) {
        let text = String::from_utf8_lossy(&bytes).to_string();
        ResponseBody::Text {
            text,
            mime: content_type,
        }
    } else {
        let base64_str = base64::engine::general_purpose::STANDARD.encode(&bytes);
        ResponseBody::Binary {
            base64: base64_str,
            mime: content_type,
        }
    };

    Ok(HttpResponse {
        status,
        status_text,
        headers: headers_out,
        body,
        time_ms,
        size,
        url: final_url.to_string(),
    })
}

fn parse_method(s: &str) -> Method {
    match s.to_ascii_uppercase().as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "PATCH" => Method::PATCH,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        other => Method::from_bytes(other.as_bytes()).unwrap_or(Method::GET),
    }
}

fn build_headers(items: &[KeyValue]) -> HeaderMap {
    let mut headers = HeaderMap::new();
    for kv in items.iter().filter(|h| h.enabled && !h.key.is_empty()) {
        if let (Ok(name), Ok(val)) = (
            HeaderName::from_bytes(kv.key.as_bytes()),
            HeaderValue::from_str(&kv.value),
        ) {
            headers.append(name, val);
        }
    }
    headers
}

fn apply_body(
    mut request: reqwest::RequestBuilder,
    body: &RequestBody,
    has_content_type: bool,
) -> AppResult<reqwest::RequestBuilder> {
    match body {
        RequestBody::Raw { raw, mime_type } => {
            // 用户在 Headers 里手动设置的 Content-Type 优先（与 Postman 行为一致），
            // 避免与 body mime 注入的头重复
            if !has_content_type {
                let mime = mime_type
                    .parse::<mime::Mime>()
                    .unwrap_or(mime::APPLICATION_OCTET_STREAM);
                request = request.header(CONTENT_TYPE, mime.as_ref());
            }
            request = request.body(raw.clone());
        }
        RequestBody::Form { items } => {
            let form: Vec<(String, String)> = filter_enabled(items)
                .into_iter()
                .map(|kv| (kv.key.clone(), kv.value.clone()))
                .collect();
            request = request.form(&form);
        }
        RequestBody::Multipart { items } => {
            let mut form = reqwest::multipart::Form::new();
            for kv in filter_enabled(items) {
                form = form.text(kv.key.clone(), kv.value.clone());
            }
            request = request.multipart(form);
        }
    }
    Ok(request)
}

fn filter_enabled(items: &[KeyValue]) -> Vec<&KeyValue> {
    items.iter().filter(|kv| kv.enabled && !kv.key.is_empty()).collect()
}

/// 判断响应是否为可显示为文本的内容
fn is_textual(content_type_lower: &str, bytes: &[u8]) -> bool {
    if is_textual_mime(content_type_lower) {
        return true;
    }
    // 明确是二进制媒体类型（音频 / 视频 / 图片 / 压缩包等）时不做 UTF-8 猜测，
    // 否则裸 PCM 这类数据可能被误判成文本，前端就拿不到音频播放器了
    if is_binary_mime(content_type_lower) {
        return false;
    }
    // content-type 缺失或无法识别时，按字节内容启发式判断
    if bytes.len() < 5 * 1024 * 1024 {
        return looks_like_text(bytes);
    }
    false
}

/// 明确按文本处理的 content-type 关键字
fn is_textual_mime(m: &str) -> bool {
    [
        "json",
        "xml",
        "text",
        "javascript",
        "ecmascript",
        "html",
        "svg",
        "urlencoded",
        "yaml",
        "csv",
        "graphql",
        "x-sh",
    ]
    .iter()
    .any(|k| m.contains(k))
}

/// 明确的二进制媒体类型
fn is_binary_mime(m: &str) -> bool {
    if m.is_empty() {
        return false;
    }
    // image/* 中只有 svg 是文本
    if m.starts_with("image/") && !m.contains("svg") {
        return true;
    }
    [
        "audio/",
        "video/",
        "font/",
        "octet-stream",
        "protobuf",
        "msgpack",
        "cbor",
        "wasm",
        "pdf",
        "zip",
        "gzip",
        "x-tar",
        "7z",
        "rar",
    ]
    .iter()
    .any(|k| m.contains(k))
}

/// 字节内容是否像文本：合法 UTF-8、无空字节、控制字符占比极低
fn looks_like_text(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    if bytes.contains(&0u8) || std::str::from_utf8(bytes).is_err() {
        return false;
    }
    let control = bytes
        .iter()
        .filter(|b| **b < 0x20 && **b != b'\t' && **b != b'\n' && **b != b'\r')
        .count();
    control * 100 <= bytes.len()
}

#[cfg(test)]
mod tests {
    use super::is_textual;
    use super::{build_client, execute};
    use crate::http::model::HttpRequest;
    use crate::http::proxy::{self, ProxySettings};
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// 一段裸 PCM（含空字节）不能被当成文本，否则前端拿不到音频播放器
    fn raw_pcm() -> Vec<u8> {
        (0..2048u32).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn audio_and_media_are_binary() {
        assert!(!is_textual("audio/wav", b"RIFF\x00\x00\x00\x00WAVEfmt "));
        assert!(!is_textual("audio/pcm", &raw_pcm()));
        assert!(!is_textual("audio/mpeg", b"ID3\x04\x00\x00"));
        assert!(!is_textual("video/mp4", b"\x00\x00\x00\x18ftypmp42"));
        assert!(!is_textual("image/png", b"\x89PNG\r\n\x1a\n"));
        assert!(!is_textual("application/octet-stream", b"{\"a\":1}"));
        assert!(!is_textual("application/pdf", b"%PDF-1.7"));
    }

    #[test]
    fn textual_types_still_render_as_text() {
        assert!(is_textual("application/json", b"{\"a\":1}"));
        assert!(is_textual("application/json; charset=utf-8", b"[]"));
        assert!(is_textual("text/plain", b"hello"));
        assert!(is_textual("text/event-stream", b"data: 1\n\n"));
        assert!(is_textual("application/xml", b"<a/>"));
        // svg 是文本型图片
        assert!(is_textual("image/svg+xml", b"<svg/>"));
    }

    #[test]
    fn unknown_content_type_falls_back_to_content_sniffing() {
        // 合法 UTF-8、无空字节 → 文本
        assert!(is_textual("", "中文响应".as_bytes()));
        assert!(is_textual("application/x-unknown", b"plain ascii body"));
        // 含空字节 / 非法 UTF-8 / 含控制字符 → 二进制
        assert!(!is_textual("", b"abc\x00def"));
        assert!(!is_textual("", &[0xff, 0xfe, 0xfd, 0x00]));
        assert!(!is_textual("", &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08]));
        // 空响应体按文本处理
        assert!(is_textual("", b""));
    }

    // ---- 代理走通性（真实 TCP，不需要外网） ----

    /// 极简 HTTP 代理：记录收到的请求行，直接回一个固定响应（不转发）。
    /// 返回监听地址与"最近一次请求行"句柄。
    async fn spawn_fake_proxy() -> (String, Arc<Mutex<Option<String>>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let seen: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
        let seen_out = seen.clone();

        tokio::spawn(async move {
            while let Ok((mut sock, _)) = listener.accept().await {
                let seen = seen_out.clone();
                tokio::spawn(async move {
                    // 读到请求头结束：不足以判定"整个请求到齐"，但足以拿到请求行
                    let mut buf = vec![0u8; 4096];
                    let mut head = Vec::new();
                    loop {
                        match sock.read(&mut buf).await {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                head.extend_from_slice(&buf[..n]);
                                if head.windows(4).any(|w| w == b"\r\n\r\n") {
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(line) = head.split(|b| *b == b'\n').next() {
                        *seen.lock().unwrap() =
                            Some(String::from_utf8_lossy(line).trim().to_string());
                    }
                    let body = "via-proxy";
                    let resp = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        body.len(),
                        body
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.shutdown().await;
                });
            }
        });

        (addr, seen)
    }

    fn req(url: &str) -> HttpRequest {
        HttpRequest {
            method: "GET".into(),
            url: url.into(),
            params: vec![],
            headers: vec![],
            body: None,
            timeout_ms: Some(10_000),
            follow_redirects: Some(true),
        }
    }

    /// 开启代理后，请求必须以绝对形式打到代理上（而不是直连目标站）。
    /// 目标用 example.com：真实 DNS 解析由代理解析，这里代理直接应答，
    /// 因此即使离线也能断言"请求确实经过了代理"。
    #[tokio::test]
    async fn request_traverses_configured_proxy() {
        let _g = proxy::global_lock();
        let (proxy_addr, seen) = spawn_fake_proxy().await;
        proxy::set(
            proxy::normalize(&ProxySettings {
                enabled: true,
                url: proxy_addr,
                bypass: String::new(),
            })
            .unwrap(),
        );

        let client = build_client().unwrap();
        let resp = execute(&client, req("http://example.com/proxied")).await.unwrap();
        assert_eq!(resp.status, 200);
        match resp.body {
            crate::http::model::ResponseBody::Text { text, .. } => assert_eq!(text, "via-proxy"),
            other => panic!("期望文本响应，实际 {other:?}"),
        }

        let line = seen.lock().unwrap().clone().expect("代理没有收到任何请求");
        assert!(
            line.starts_with("GET http://example.com/proxied"),
            "代理应收到绝对形式请求行，实际：{line}"
        );

        proxy::set(ProxySettings::default());
    }

    /// 本机地址始终直连：代理开着也不该被代理接管（否则本地 Mock / Ollama 会连不上）
    #[tokio::test]
    async fn loopback_stays_direct_even_with_proxy_enabled() {
        let _g = proxy::global_lock();
        let origin = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin_addr = origin.local_addr().unwrap();
        let hits = Arc::new(Mutex::new(0usize));
        let hits_out = hits.clone();
        tokio::spawn(async move {
            while let Ok((mut sock, _)) = origin.accept().await {
                *hits_out.lock().unwrap() += 1;
                let mut buf = vec![0u8; 1024];
                let _ = sock.read(&mut buf).await;
                let _ = sock
                    .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
                    .await;
                let _ = sock.shutdown().await;
            }
        });

        let (proxy_addr, proxy_seen) = spawn_fake_proxy().await;
        proxy::set(
            proxy::normalize(&ProxySettings {
                enabled: true,
                url: proxy_addr,
                bypass: String::new(),
            })
            .unwrap(),
        );

        let client = build_client().unwrap();
        let resp = execute(&client, req(&format!("http://127.0.0.1:{}/local", origin_addr.port())))
            .await
            .unwrap();
        assert_eq!(resp.status, 204);
        assert_eq!(*hits.lock().unwrap(), 1, "本机目标应被直连");
        assert!(
            proxy_seen.lock().unwrap().is_none(),
            "本机目标不应经过代理"
        );

        proxy::set(ProxySettings::default());
    }

    /// 直连名单里的域名不走代理
    #[tokio::test]
    async fn bypassed_host_skips_proxy() {
        let _g = proxy::global_lock();
        let (proxy_addr, proxy_seen) = spawn_fake_proxy().await;
        proxy::set(
            proxy::normalize(&ProxySettings {
                enabled: true,
                url: proxy_addr,
                bypass: "example.com".into(),
            })
            .unwrap(),
        );

        let client = build_client().unwrap();
        // 绕过代理后要真去连 example.com，联网/断网都不关心，只断言代理没被碰到；
        // 超时收紧，避免 DNS 卡住拖慢用例
        let target = HttpRequest {
            timeout_ms: Some(3_000),
            ..req("http://example.com/bypassed")
        };
        let _ = execute(&client, target).await;
        assert!(
            proxy_seen.lock().unwrap().is_none(),
            "直连名单内的主机不应经过代理"
        );

        proxy::set(ProxySettings::default());
    }
}
