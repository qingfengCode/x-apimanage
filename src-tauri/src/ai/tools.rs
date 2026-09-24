use serde_json::{json, Value};

use crate::db::repos::{collection, document, request};
use crate::error::AppResult;
use crate::http::{engine as http_engine, model as http_model};

/// 工具描述（同时用于 OpenAI tools 与 MCP tools/list）
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub const DEFAULT_SYSTEM_PROMPT: &str = r#"你是 x-apimanage（一个类 Postman 的 API 调试平台）内置的 AI 助手。你可以：

1. **编写 API 调试配置**：用 create_request 工具创建请求（方法/URL/请求头/Body/参数），必要时用 send_request 实际发送并查看响应来调试迭代。
2. **编写 API 文档**：根据接口信息或实际响应，用 save_document 工具（doc_type="api"）生成结构清晰的 Markdown API 文档（含接口说明、参数表、请求/响应示例）。
3. **编写产品文档**：用 save_document 工具（doc_type="product"）生成需求说明、功能介绍等产品类 Markdown 文档。

使用规范：
- 用户给你 API 信息时，优先创建可执行的调试配置并实际发送验证，再基于真实响应写文档。
- 工具返回的结果中若包含 id，在总结里告知用户。
- 回答使用中文，简洁专业。"#;

pub fn tool_defs() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "list_collections",
            description: "列出应用中所有的集合（Collection）与文件夹",
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "list_requests",
            description: "列出集合下的所有请求（API 调试配置）。不传 collection_id 时列出全部请求",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "collection_id": { "type": "string", "description": "集合 id（可选）" }
                }
            }),
        },
        ToolDef {
            name: "create_request",
            description: "创建一条 API 调试配置（请求）。可指定方法、URL、请求头、查询参数、Body、测试脚本",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "请求名称" },
                    "method": { "type": "string", "description": "HTTP 方法，默认 GET" },
                    "url": { "type": "string", "description": "请求 URL" },
                    "collection_id": { "type": "string", "description": "目标集合 id（可选，不传则放入第一个集合）" },
                    "params": {
                        "type": "array", "description": "查询参数（可选）",
                        "items": { "type": "object", "properties": {
                            "key": { "type": "string" }, "value": { "type": "string" }
                        }, "required": ["key", "value"] }
                    },
                    "headers": {
                        "type": "array", "description": "请求头（可选）",
                        "items": { "type": "object", "properties": {
                            "key": { "type": "string" }, "value": { "type": "string" }
                        }, "required": ["key", "value"] }
                    },
                    "body": {
                        "type": "object", "description": "请求体（可选）。raw 文本传 {\"mode\":\"raw\",\"raw\":\"...\",\"mimeType\":\"application/json\"}；表单传 {\"mode\":\"form\",\"items\":[{\"key\":\"..\",\"value\":\"..\"}]}",
                        "properties": {
                            "mode": { "type": "string", "enum": ["raw", "form", "multipart"] },
                            "raw": { "type": "string" },
                            "mimeType": { "type": "string" },
                            "items": { "type": "array", "items": { "type": "object", "properties": { "key": {"type":"string"}, "value": {"type":"string"} } } }
                        }
                    },
                    "test_script": { "type": "string", "description": "Postman 风格测试脚本，可选（pm.test/pm.expect）" }
                },
                "required": ["name", "url"]
            }),
        },
        ToolDef {
            name: "send_request",
            description: "立即发送一个 HTTP 请求并返回响应（状态码/响应头/响应体，响应体超长会截断），用于实际调试接口",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "method": { "type": "string", "description": "HTTP 方法" },
                    "url": { "type": "string", "description": "请求 URL" },
                    "headers": {
                        "type": "array", "description": "请求头（可选）",
                        "items": { "type": "object", "properties": {
                            "key": { "type": "string" }, "value": { "type": "string" }
                        }, "required": ["key", "value"] }
                    },
                    "query_params": {
                        "type": "array", "description": "查询参数（可选，也可直接拼在 URL 上）",
                        "items": { "type": "object", "properties": {
                            "key": { "type": "string" }, "value": { "type": "string" }
                        }, "required": ["key", "value"] }
                    },
                    "body": {
                        "type": "object", "description": "请求体（可选），同 create_request 的 body",
                        "properties": {
                            "mode": { "type": "string", "enum": ["raw", "form", "multipart"] },
                            "raw": { "type": "string" },
                            "mimeType": { "type": "string" },
                            "items": { "type": "array", "items": { "type": "object", "properties": { "key": {"type":"string"}, "value": {"type":"string"} } } }
                        }
                    }
                },
                "required": ["url"]
            }),
        },
        ToolDef {
            name: "save_document",
            description: "保存一篇文档。doc_type=\"api\" 为 API 接口文档，\"product\" 为产品文档。content 为 Markdown",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "文档标题" },
                    "doc_type": { "type": "string", "enum": ["api", "product"] },
                    "content": { "type": "string", "description": "Markdown 正文" },
                    "document_id": { "type": "string", "description": "要更新的文档 id（可选，不传则新建）" },
                    "request_id": { "type": "string", "description": "关联的请求 id（可选）" }
                },
                "required": ["title", "doc_type", "content"]
            }),
        },
        ToolDef {
            name: "list_documents",
            description: "列出已保存的文档（API 文档 / 产品文档）",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "doc_type": { "type": "string", "enum": ["api", "product"], "description": "按类型过滤（可选）" }
                }
            }),
        },
    ]
}

/// 响应体回传给模型时的最大长度（字符）
const MAX_BODY_CHARS: usize = 8000;

fn truncate_body(text: &str) -> String {
    if text.chars().count() <= MAX_BODY_CHARS {
        text.to_string()
    } else {
        let cut: String = text.chars().take(MAX_BODY_CHARS).collect();
        format!("{cut}\n...[响应体过长，已截断，共 {} 字符]", text.chars().count())
    }
}

fn parse_kv_list(v: Option<&Value>) -> Vec<http_model::KeyValue> {
    v.and_then(|x| x.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| {
                    let key = item.get("key")?.as_str()?.to_string();
                    let value = item.get("value").and_then(|x| x.as_str()).unwrap_or("").to_string();
                    Some(http_model::KeyValue { key, value, enabled: true })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn parse_body(v: Option<&Value>) -> Option<http_model::RequestBody> {
    let b = v?;
    match b.get("mode")?.as_str()? {
        "raw" => Some(http_model::RequestBody::Raw {
            raw: b.get("raw").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            mime_type: b
                .get("mimeType")
                .and_then(|x| x.as_str())
                .unwrap_or("application/json")
                .to_string(),
        }),
        "form" => Some(http_model::RequestBody::Form {
            items: parse_kv_list(b.get("items")),
        }),
        "multipart" => Some(http_model::RequestBody::Multipart {
            items: parse_kv_list(b.get("items")),
        }),
        _ => None,
    }
}

/// 执行工具（AI 聊天与 MCP 共用）。
/// db 为共享数据库；http 用于真实发送请求。
/// 注意：std Mutex guard 不能跨 await，同步分支内短暂持锁。
pub async fn execute(
    name: &str,
    args: &Value,
    db: &crate::db::SharedDb,
    http: &reqwest::Client,
) -> AppResult<Value> {
    fn lock(db: &crate::db::SharedDb) -> AppResult<std::sync::MutexGuard<'_, crate::db::Db>> {
        db.lock().map_err(|_| crate::error::AppError::Other("数据库锁中毒".into()))
    }
    match name {
        "list_collections" => {
            let conn = lock(db)?;
            let cols = collection::list_all(&conn.conn)?;
            Ok(json!(cols
                .into_iter()
                .map(|c| json!({ "id": c.id, "name": c.name, "kind": c.kind, "parentId": c.parent_id }))
                .collect::<Vec<_>>()))
        }
        "list_requests" => {
            let cid = args.get("collection_id").and_then(|x| x.as_str());
            let rows = {
                let conn = lock(db)?;
                match cid {
                    Some(id) => request::list_by_collection(&conn.conn, id)?,
                    None => {
                        let mut all = Vec::new();
                        for c in collection::list_all(&conn.conn)? {
                            all.extend(request::list_by_collection(&conn.conn, &c.id)?);
                        }
                        all
                    }
                }
            };
            Ok(json!(rows
                .into_iter()
                .map(|r| json!({
                    "id": r.id, "collectionId": r.collection_id, "name": r.name,
                    "method": r.method, "url": r.url,
                }))
                .collect::<Vec<_>>()))
        }
        "create_request" => {
            let conn = lock(db)?;
            let name = args.get("name").and_then(|x| x.as_str()).unwrap_or("未命名请求");
            let url = args.get("url").and_then(|x| x.as_str()).unwrap_or("");
            // 目标集合：优先指定，否则第一个 kind=collection 的集合，否则新建
            let collection_id = match args.get("collection_id").and_then(|x| x.as_str()) {
                Some(id) => id.to_string(),
                None => {
                    let cols = collection::list_all(&conn.conn)?;
                    match cols.iter().find(|c| c.kind == "collection") {
                        Some(c) => c.id.clone(),
                        None => {
                            let c = collection::upsert(
                                &conn.conn,
                                &collection::CollectionInput {
                                    id: None,
                                    name: "AI 创建".into(),
                                    parent_id: None,
                                    kind: Some("collection".into()),
                                    description: None,
                                    sort_order: None,
                                },
                            )?;
                            c.id
                        }
                    }
                }
            };
            let params = parse_kv_list(args.get("params"));
            let headers = parse_kv_list(args.get("headers"));
            let body = parse_body(args.get("body"));
            let input = request::RequestInput {
                id: None,
                collection_id,
                name: name.to_string(),
                method: args
                    .get("method")
                    .and_then(|x| x.as_str())
                    .unwrap_or("GET")
                    .to_uppercase(),
                url: Some(url.to_string()),
                params: Some(serde_json::to_string(&params)?),
                headers: Some(serde_json::to_string(&headers)?),
                body: body.map(|b| serde_json::to_string(&b)).transpose()?,
                auth: None,
                pre_script: None,
                test_script: args.get("test_script").and_then(|x| x.as_str()).map(String::from),
                sort_order: None,
                timeout_ms: args.get("timeout_ms").and_then(|x| x.as_i64()),
            };
            let saved = request::upsert(&conn.conn, &input)?;
            Ok(json!({
                "id": saved.id, "collectionId": saved.collection_id,
                "name": saved.name, "method": saved.method, "url": saved.url,
                "hint": "已创建，可在左侧集合中查看"
            }))
        }
        "send_request" => {
            let url = args.get("url").and_then(|x| x.as_str()).unwrap_or_default().to_string();
            if url.is_empty() {
                return Err(crate::error::AppError::Other("url 不能为空".into()));
            }
            let req = http_model::HttpRequest {
                method: args
                    .get("method")
                    .and_then(|x| x.as_str())
                    .unwrap_or("GET")
                    .to_uppercase(),
                url,
                params: parse_kv_list(args.get("query_params")),
                headers: parse_kv_list(args.get("headers")),
                body: parse_body(args.get("body")),
                timeout_ms: None,
                follow_redirects: None,
            };

            let resp = http_engine::execute(http, req).await?;
            let body_text = match &resp.body {
                http_model::ResponseBody::Text { text, .. } => truncate_body(text),
                http_model::ResponseBody::Binary { mime, .. } => format!("[二进制响应 {mime}]"),
            };
            Ok(json!({
                "status": resp.status,
                "statusText": resp.status_text,
                "timeMs": resp.time_ms,
                "size": resp.size,
                "headers": resp.headers.iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>(),
                "body": body_text,
            }))
        }
        "save_document" => {
            let conn = lock(db)?;
            let title = args.get("title").and_then(|x| x.as_str()).unwrap_or("未命名文档");
            let doc_type = args.get("doc_type").and_then(|x| x.as_str()).unwrap_or("api");
            let content = args.get("content").and_then(|x| x.as_str()).unwrap_or_default();
            let input = document::DocumentInput {
                id: args.get("document_id").and_then(|x| x.as_str()).map(String::from),
                doc_type: doc_type.to_string(),
                title: title.to_string(),
                content: content.to_string(),
                request_id: args.get("request_id").and_then(|x| x.as_str()).map(String::from),
            };
            let saved = document::upsert(&conn.conn, &input)?;
            Ok(json!({
                "id": saved.id, "docType": saved.doc_type, "title": saved.title,
                "hint": "已保存，可在左侧 Docs 页签查看"
            }))
        }
        "list_documents" => {
            let filter = args.get("doc_type").and_then(|x| x.as_str());
            let conn = lock(db)?;
            let docs = document::list_all(&conn.conn)?;
            Ok(json!(docs
                .into_iter()
                .filter(|d| filter.map(|f| d.doc_type == f).unwrap_or(true))
                .map(|d| json!({
                    "id": d.id, "docType": d.doc_type, "title": d.title,
                    "updatedAt": d.updated_at, "contentChars": d.content.chars().count()
                }))
                .collect::<Vec<_>>()))
        }
        other => Err(crate::error::AppError::Other(format!("未知工具: {other}"))),
    }
}
