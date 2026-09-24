use serde_json::{json, Value};

use crate::error::{AppError, AppResult};

use super::tools;

/// 一轮对话消息（OpenAI 格式子集）
#[derive(Debug, Clone)]
pub enum ChatMessage {
    System(String),
    User(String),
    Assistant {
        content: String,
        /// 原样回传的 tool_calls（含 id/function）
        tool_calls: Option<Value>,
    },
    Tool {
        tool_call_id: String,
        content: String,
    },
}

impl ChatMessage {
    pub fn to_json(&self) -> Value {
        match self {
            ChatMessage::System(c) => json!({ "role": "system", "content": c }),
            ChatMessage::User(c) => json!({ "role": "user", "content": c }),
            ChatMessage::Assistant { content, tool_calls } => {
                let mut m = json!({ "role": "assistant", "content": content });
                if let Some(tc) = tool_calls {
                    m["tool_calls"] = tc.clone();
                }
                m
            }
            ChatMessage::Tool { tool_call_id, content } => json!({
                "role": "tool", "tool_call_id": tool_call_id, "content": content
            }),
        }
    }
}

pub struct AiConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub system_prompt_extra: String,
}

/// 单次 chat completion（非流式，支持 tools）
pub async fn chat_completion(
    http: &reqwest::Client,
    cfg: &AiConfig,
    messages: &[ChatMessage],
    with_tools: bool,
) -> AppResult<Value> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");

    let mut body = json!({
        "model": cfg.model,
        "messages": messages.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
        "stream": false,
    });
    if with_tools {
        body["tools"] = json!(tools::tool_defs()
            .into_iter()
            .map(|t| json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            }))
            .collect::<Vec<_>>());
        body["tool_choice"] = json!("auto");
    }

    let mut req = http.post(&url).json(&body).timeout(std::time::Duration::from_secs(180));
    if !cfg.api_key.is_empty() {
        req = req.bearer_auth(&cfg.api_key);
    }

    let resp = req.send().await?;
    let status = resp.status();
    let text = resp.text().await?;
    if !status.is_success() {
        // 截断超长错误信息
        let msg: String = text.chars().take(600).collect();
        return Err(AppError::Http(format!("AI 服务返回 {status}: {msg}")));
    }
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| AppError::Http(format!("AI 服务响应解析失败: {e}")))?;
    Ok(v)
}

/// 从 completion 响应中提取 (content, tool_calls)
pub fn extract_message(completion: &Value) -> AppResult<(String, Option<Value>)> {
    let msg = completion
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .ok_or_else(|| AppError::Http("AI 响应缺少 choices[0].message".into()))?;
    let content = msg
        .get("content")
        .and_then(|c| c.as_str())
        .unwrap_or("")
        .to_string();
    let tool_calls = msg
        .get("tool_calls")
        .filter(|tc| tc.as_array().map(|a| !a.is_empty()).unwrap_or(false))
        .cloned();
    Ok((content, tool_calls))
}

/// 聚合流式 tool_calls 分片（按 index 拼 arguments/name/id）
#[derive(Default)]
struct ToolCallAgg {
    id: String,
    name: String,
    args: String,
}

/// 流式 chat completion（SSE）。
/// 每个 content delta 通过 on_delta 回调实时上报；
/// 返回聚合后的 (完整 content, tool_calls)。
/// cancel 非 None 时，每收到一个 chunk 都会检查取消令牌（支持前端"停止生成"）。
pub async fn chat_completion_stream(
    http: &reqwest::Client,
    cfg: &AiConfig,
    messages: &[ChatMessage],
    with_tools: bool,
    cancel: Option<&tokio_util::sync::CancellationToken>,
    on_delta: &(dyn Fn(&str) + Send + Sync),
) -> AppResult<(String, Option<Value>)> {
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/chat/completions");

    let mut body = json!({
        "model": cfg.model,
        "messages": messages.iter().map(|m| m.to_json()).collect::<Vec<_>>(),
        "stream": true,
    });
    if with_tools {
        body["tools"] = json!(tools::tool_defs()
            .into_iter()
            .map(|t| json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.input_schema,
                }
            }))
            .collect::<Vec<_>>());
        body["tool_choice"] = json!("auto");
    }

    let mut req = http
        .post(&url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(300));
    if !cfg.api_key.is_empty() {
        req = req.bearer_auth(&cfg.api_key);
    }

    let resp = req.send().await?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let msg: String = text.chars().take(600).collect();
        return Err(AppError::Http(format!("AI 服务返回 {status}: {msg}")));
    }

    use futures_util::StreamExt;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    let mut content = String::new();
    let mut agg: Vec<ToolCallAgg> = Vec::new();

    while let Some(chunk) = stream.next().await {
        if let Some(c) = cancel {
            if c.is_cancelled() {
                return Err(AppError::Other("AI 请求已取消".into()));
            }
        }
        let bytes = chunk?;
        buf.push_str(&String::from_utf8_lossy(&bytes));
        // 逐行解析 SSE（以 "data: " 开头）
        while let Some(pos) = buf.find('\n') {
            let line: String = buf[..pos].trim_end_matches('\r').to_string();
            buf.drain(..=pos);
            let Some(data) = line.strip_prefix("data: ").map(str::trim) else {
                continue;
            };
            if data == "[DONE]" {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(data) else {
                continue; // 容忍不完整/非 JSON 行
            };
            let Some(delta) = v.pointer("/choices/0/delta") else {
                continue;
            };
            if let Some(c) = delta.get("content").and_then(|x| x.as_str()) {
                if !c.is_empty() {
                    content.push_str(c);
                    on_delta(c);
                }
            }
            if let Some(tcs) = delta.get("tool_calls").and_then(|x| x.as_array()) {
                for tc in tcs {
                    let idx = tc.get("index").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
                    while agg.len() <= idx {
                        agg.push(ToolCallAgg::default());
                    }
                    if let Some(id) = tc.get("id").and_then(|x| x.as_str()) {
                        if !id.is_empty() {
                            agg[idx].id = id.to_string();
                        }
                    }
                    if let Some(f) = tc.get("function") {
                        if let Some(n) = f.get("name").and_then(|x| x.as_str()) {
                            if !n.is_empty() {
                                agg[idx].name = n.to_string();
                            }
                        }
                        if let Some(a) = f.get("arguments").and_then(|x| x.as_str()) {
                            agg[idx].args.push_str(a);
                        }
                    }
                }
            }
        }
    }

    let tool_calls = if agg.is_empty() {
        None
    } else {
        Some(json!(agg
            .into_iter()
            .enumerate()
            .map(|(i, a)| json!({
                "id": if a.id.is_empty() { format!("call_{i}") } else { a.id },
                "type": "function",
                "function": { "name": a.name, "arguments": a.args },
            }))
            .collect::<Vec<_>>()))
    };

    Ok((content, tool_calls))
}

/// 默认系统提示词 + 用户自定义追加
pub fn build_system_prompt(extra: &str) -> String {
    if extra.trim().is_empty() {
        tools::DEFAULT_SYSTEM_PROMPT.to_string()
    } else {
        format!("{}\n\n用户补充指令：\n{}", tools::DEFAULT_SYSTEM_PROMPT, extra)
    }
}
