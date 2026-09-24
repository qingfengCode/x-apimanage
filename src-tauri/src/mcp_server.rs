//! MCP (Model Context Protocol) 服务端 —— Streamable HTTP 无状态模式。
//!
//! 外部 MCP 客户端（Claude Code / Cursor 等）通过
//! `http://127.0.0.1:{port}/mcp` POST JSON-RPC 消息与本应用交互，
//! 可调用与内置 AI 助手相同的工具集（创建调试配置 / 发送请求 / 写文档…）。

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Mutex;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::ai::tools;
use crate::db::Db;

pub struct McpServer {
    pub addr: SocketAddr,
    pub cancel: CancellationToken,
}

pub struct McpState {
    pub running: Mutex<Option<McpServer>>,
    pub addr: Mutex<Option<SocketAddr>>,
}

impl Default for McpState {
    fn default() -> Self {
        Self {
            running: Mutex::new(None),
            addr: Mutex::new(None),
        }
    }
}

const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const SERVER_PROTOCOL_VERSION: &str = "2025-06-18";

/// 本机局域网 IP（UDP connect 不实际发包）
pub fn lan_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("8.8.8.8:80").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// 读取当前配置的 MCP 访问密钥（空 = 不鉴权）
fn current_token(db: &Arc<Mutex<Db>>) -> String {
    db.lock()
        .ok()
        .and_then(|conn| crate::db::repos::ai_setting::get(&conn.conn).ok())
        .map(|s| s.mcp_token)
        .unwrap_or_default()
}

/// 启动 MCP server（阻塞至绑定成功或失败）。
/// 设置了访问密钥 → 绑定 0.0.0.0（局域网可访问，凭密钥鉴权）；
/// 未设置 → 仅绑定 127.0.0.1（本机使用，无需鉴权）。
pub async fn start(
    port: u16,
    db: Arc<Mutex<Db>>,
    http: reqwest::Client,
) -> Result<McpServer, String> {
    let token = current_token(&db);
    let bind_host = if token.is_empty() { "127.0.0.1" } else { "0.0.0.0" };

    let app = Router::new()
        .route("/mcp", post(handle_post).get(handle_get))
        .with_state((db, http));

    let listener = tokio::net::TcpListener::bind((bind_host, port))
        .await
        .map_err(|e| format!("MCP 端口 {port} 绑定失败: {e}"))?;
    let addr = listener
        .local_addr()
        .map_err(|e| format!("获取监听地址失败: {e}"))?;

    let cancel = CancellationToken::new();
    let token = cancel.clone();
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move { token.cancelled().await })
            .await
            .ok();
    });

    Ok(McpServer { addr, cancel })
}

/// 401：MCP 客户端能识别的 JSON-RPC 错误
fn unauthorized_response() -> Response {
    (
        StatusCode::UNAUTHORIZED,
        [("Content-Type", "application/json")],
        Json(json!({
            "jsonrpc": "2.0",
            "id": null,
            "error": { "code": -32001, "message": "Unauthorized: missing or invalid Bearer token" }
        })),
    )
        .into_response()
}

/// 无状态模式不支持 GET（SSE 流）
async fn handle_get() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [("Allow", "POST")],
        "SSE not supported; use stateless POST",
    )
        .into_response()
}

async fn handle_post(
    State((db, http)): State<(Arc<Mutex<Db>>, reqwest::Client)>,
    headers: axum::http::HeaderMap,
    Json(rpc): Json<Value>,
) -> Response {
    // 访问密钥校验：配置了密钥时要求 Authorization: Bearer <token>
    let expected = current_token(&db);
    if !expected.is_empty() {
        let got = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .unwrap_or("");
        if got != expected {
            return unauthorized_response();
        }
    }

    let id = rpc.get("id").cloned();
    let method = rpc.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let params = rpc.get("params").cloned().unwrap_or(json!({}));

    // 通知（无 id）：返回 202 空体
    let Some(id) = id else {
        return (StatusCode::ACCEPTED, "").into_response();
    };

    let result = match method {
        "initialize" => initialize(&params),
        "ping" => Ok(json!({})),
        "tools/list" => tools_list(),
        "tools/call" => tools_call(&params, &db, &http).await,
        "resources/list" => resources_list(&db),
        "resources/templates/list" => resource_templates(),
        "resources/read" => resources_read(&params, &db),
        other => Err(json!({
            "code": -32601,
            "message": format!("Method not found: {other}")
        })),
    };

    let body = match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(error) => json!({ "jsonrpc": "2.0", "id": id, "error": error }),
    };
    (
        StatusCode::OK,
        [("Content-Type", "application/json")],
        Json(body),
    )
        .into_response()
}

fn initialize(params: &Value) -> Result<Value, Value> {
    let requested = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or(SERVER_PROTOCOL_VERSION);
    let version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
        requested
    } else {
        SERVER_PROTOCOL_VERSION
    };
    Ok(json!({
        "protocolVersion": version,
        "capabilities": { "tools": {}, "resources": {} },
        "serverInfo": { "name": "x-apimanage", "version": env!("CARGO_PKG_VERSION") }
    }))
}

fn tools_list() -> Result<Value, Value> {
    Ok(json!({
        "tools": tools::tool_defs()
            .into_iter()
            .map(|t| json!({
                "name": t.name,
                "description": t.description,
                "inputSchema": t.input_schema,
            }))
            .collect::<Vec<_>>()
    }))
}

async fn tools_call(
    params: &Value,
    db: &Arc<Mutex<Db>>,
    http: &reqwest::Client,
) -> Result<Value, Value> {
    let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
    let args = params.get("arguments").cloned().unwrap_or(json!({}));

    match tools::execute(name, &args, db, http).await {
        Ok(result) => {
            let text = serde_json::to_string_pretty(&result)
                .unwrap_or_else(|_| result.to_string());
            Ok(json!({ "content": [{ "type": "text", "text": text }], "isError": false }))
        }
        Err(e) => Ok(json!({
            "content": [{ "type": "text", "text": e.to_string() }],
            "isError": true
        })),
    }
}

// ---- MCP resources：把集合/请求/文档/历史暴露为可读资源 ----

use crate::db::repos::{collection, document, history, request};

/// 集合树（集合 + 每个集合下的请求）JSON
fn collections_tree(conn: &rusqlite::Connection) -> Result<Value, String> {
    let cols = collection::list_all(conn).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for c in &cols {
        let reqs = request::list_by_collection(conn, &c.id).map_err(|e| e.to_string())?;
        out.push(json!({
            "id": c.id, "name": c.name, "kind": c.kind, "parentId": c.parent_id,
            "requests": reqs.iter().map(|r| json!({
                "id": r.id, "name": r.name, "method": r.method, "url": r.url,
            })).collect::<Vec<_>>(),
        }));
    }
    Ok(json!(out))
}

fn resources_list(db: &Arc<Mutex<Db>>) -> Result<Value, Value> {
    let conn = db.lock().map_err(|_| rpc_err(-32603, "数据库锁中毒"))?;
    let mut resources = vec![
        json!({
            "uri": "xapimanage://collections",
            "name": "所有集合与请求",
            "description": "应用内全部集合、文件夹及请求的树形结构（JSON）",
            "mimeType": "application/json",
        }),
        json!({
            "uri": "xapimanage://history",
            "name": "最近请求历史",
            "description": "最近 100 条请求历史（方法/URL/状态码/耗时）",
            "mimeType": "application/json",
        }),
    ];
    // 每篇文档一个资源
    if let Ok(docs) = document::list_all(&conn.conn) {
        for d in docs {
            resources.push(json!({
                "uri": format!("xapimanage://documents/{}", d.id),
                "name": d.title,
                "description": if d.doc_type == "product" { "产品文档（Markdown）" } else { "API 文档（Markdown）" },
                "mimeType": "text/markdown",
            }));
        }
    }
    Ok(json!({ "resources": resources }))
}

fn resource_templates() -> Result<Value, Value> {
    Ok(json!({
        "resourceTemplates": [
            {
                "uriTemplate": "xapimanage://requests/{id}",
                "name": "请求详情",
                "description": "按 id 读取单个请求的完整调试配置（方法/URL/参数/请求头/Body/脚本，JSON）",
                "mimeType": "application/json",
            }
        ]
    }))
}

fn resources_read(params: &Value, db: &Arc<Mutex<Db>>) -> Result<Value, Value> {
    let uri = params
        .get("uri")
        .and_then(|u| u.as_str())
        .unwrap_or("")
        .to_string();

    let read_one = |uri: &str| -> Result<(String, String), String> {
        let conn = db.lock().map_err(|_| "数据库锁中毒".to_string())?;
        match uri {
            "xapimanage://collections" => Ok((
                serde_json::to_string_pretty(&collections_tree(&conn.conn)?).unwrap_or_default(),
                "application/json".into(),
            )),
            "xapimanage://history" => {
                let items = history::list_all(&conn.conn, 100).map_err(|e| e.to_string())?;
                Ok((
                    serde_json::to_string_pretty(&items).unwrap_or_default(),
                    "application/json".into(),
                ))
            }
            u if u.starts_with("xapimanage://documents/") => {
                let id = u.trim_start_matches("xapimanage://documents/");
                let doc = document::get(&conn.conn, id).map_err(|e| e.to_string())?;
                Ok((doc.content, "text/markdown".into()))
            }
            u if u.starts_with("xapimanage://requests/") => {
                let id = u.trim_start_matches("xapimanage://requests/");
                let r = request::get(&conn.conn, id).map_err(|e| e.to_string())?;
                Ok((
                    serde_json::to_string_pretty(&json!({
                        "id": r.id, "collectionId": r.collection_id, "name": r.name,
                        "method": r.method, "url": r.url,
                        "params": r.params, "headers": r.headers, "body": r.body,
                        "preScript": r.pre_script, "testScript": r.test_script,
                    }))
                    .unwrap_or_default(),
                    "application/json".into(),
                ))
            }
            other => Err(format!("未知资源: {other}")),
        }
    };

    match read_one(&uri) {
        Ok((text, mime)) => Ok(json!({
            "contents": [{ "uri": uri, "mimeType": mime, "text": text }]
        })),
        Err(e) => Err(rpc_err(-32602, &e)),
    }
}

fn rpc_err(code: i64, msg: &str) -> Value {
    json!({ "code": code, "message": msg })
}
