use std::collections::HashMap;
use std::sync::{Mutex as StdMutex, OnceLock};

use serde::Deserialize;
use serde_json::{json, Value};
use tauri::ipc::Channel;
use tauri::State;
use tokio_util::sync::CancellationToken;

use crate::ai::client::{self, AiConfig, ChatMessage};
use crate::ai::tools;
use crate::db::repos::ai_setting;
use crate::db::repos::ai_session::{self, AiSession, AiSessionInput, AiSessionMeta};
use crate::db::SharedDb;
use crate::error::{AppError, AppResult};

/// 推送给前端的事件（流式增量 / 工具执行 / 完成）
#[derive(Clone, serde::Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AiEvent {
    Status { text: String },
    /// 流式文本增量（打字机）
    Delta { text: String },
    Tool { name: String, ok: bool, result: String },
    Done { content: String },
    Error { message: String },
}

#[derive(Deserialize)]
#[serde(tag = "role", rename_all = "lowercase")]
pub enum ChatMsgIn {
    User { content: String },
    Assistant { content: String },
}

/// 在途 AI 生成的取消令牌注册表（key = 前端下发的 requestId）
static AI_CANCELS: OnceLock<StdMutex<HashMap<String, CancellationToken>>> = OnceLock::new();

fn cancel_registry() -> &'static StdMutex<HashMap<String, CancellationToken>> {
    AI_CANCELS.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Drop 时自动把 request_id 从注册表移除，避免泄漏
struct AiCancelGuard(String);
impl Drop for AiCancelGuard {
    fn drop(&mut self) {
        cancel_registry().lock().unwrap().remove(&self.0);
    }
}

/// 前端点"停止生成"时调用：取消对应在途请求（真实中止后端流，而非仅忽略事件）
#[tauri::command]
pub fn cancel_ai_chat(request_id: String) -> AppResult<()> {
    if let Some(token) = cancel_registry().lock().unwrap().remove(&request_id) {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
pub fn get_ai_settings(db: State<'_, SharedDb>) -> AppResult<ai_setting::AiSettings> {
    let db = db.lock().expect("db mutex poisoned");
    ai_setting::get(&db.conn)
}

#[tauri::command]
pub fn save_ai_settings(
    db: State<'_, SharedDb>,
    settings: ai_setting::AiSettings,
) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    ai_setting::save(&db.conn, &settings)
}

/// 测试 AI 连接：发一条极小的补全请求
#[tauri::command]
pub async fn test_ai_connection(
    db: State<'_, SharedDb>,
    http: State<'_, reqwest::Client>,
) -> AppResult<String> {
    let cfg = {
        let db = db.lock().expect("db mutex poisoned");
        let s = ai_setting::get(&db.conn)?;
        AiConfig {
            base_url: s.base_url,
            api_key: s.api_key,
            model: s.model,
            system_prompt_extra: String::new(),
        }
    };
    if cfg.base_url.is_empty() || cfg.model.is_empty() {
        return Err(AppError::Other("请先填写 Base URL 和模型名".into()));
    }
    let completion = client::chat_completion(
        &http,
        &cfg,
        &[ChatMessage::User("回复 OK 两个字母即可".into())],
        false,
    )
    .await?;
    let (content, _) = client::extract_message(&completion)?;
    Ok(content)
}

/// AI 对话主入口：带工具调用循环。
/// 返回 { content, messages } —— messages 为完整 OpenAI 格式历史（含工具调用），
/// 前端保存后作为下次请求的上下文。
#[tauri::command]
pub async fn ai_chat(
    db: State<'_, SharedDb>,
    http: State<'_, reqwest::Client>,
    messages: Vec<ChatMsgIn>,
    context: Option<String>,
    on_event: Channel<AiEvent>,
    request_id: Option<String>,
) -> AppResult<Value> {
    let cfg = {
        let db = db.lock().expect("db mutex poisoned");
        let s = ai_setting::get(&db.conn)?;
        AiConfig {
            base_url: s.base_url,
            api_key: s.api_key,
            model: s.model,
            system_prompt_extra: s.system_prompt,
        }
    };
    if cfg.base_url.is_empty() || cfg.model.is_empty() {
        return Err(AppError::Other(
            "AI 未配置：请点击设置按钮填写 Base URL / API Key / 模型".into(),
        ));
    }

    // 取消令牌：前端"停止生成"时通过 cancel_ai_chat(request_id) 触发；
    // _cancel_guard 在函数退出时自动从注册表移除
    let cancel_token = CancellationToken::new();
    let _cancel_guard = request_id.as_ref().map(|id| {
        cancel_registry()
            .lock()
            .unwrap()
            .insert(id.clone(), cancel_token.clone());
        AiCancelGuard(id.clone())
    });

    // 组装完整消息序列：system + 历史用户输入（附上下文）
    let mut history: Vec<ChatMessage> = vec![ChatMessage::System(client::build_system_prompt(
        &cfg.system_prompt_extra,
    ))];    for m in &messages {
        match m {
            ChatMsgIn::User { content } => history.push(ChatMessage::User(content.clone())),
            ChatMsgIn::Assistant { content } => {
                history.push(ChatMessage::Assistant {
                    content: content.clone(),
                    tool_calls: None,
                })
            }
        }
    }
    if let Some(ctx) = context.as_deref().filter(|c| !c.trim().is_empty()) {
        let last = history.last_mut();
        if let Some(ChatMessage::User(c)) = last {
            *c = format!("{c}\n\n[当前请求上下文]\n{ctx}");
        }
    }

    let _ = on_event.send(AiEvent::Status { text: "正在思考…".into() });

    // 工具循环（最多 8 轮，防止死循环）。每轮流式输出（打字机）。
    let mut round = 0;
    loop {
        if cancel_token.is_cancelled() {
            return Err(AppError::Other("AI 请求已取消".into()));
        }
        round += 1;
        if round > 8 {
            return Err(AppError::Other("工具调用轮数超限（8 轮）".into()));
        }

        let emit_delta = |t: &str| {
            let _ = on_event.send(AiEvent::Delta { text: t.to_string() });
        };
        let (content, tool_calls) = client::chat_completion_stream(
            &http,
            &cfg,
            &history,
            true,
            Some(&cancel_token),
            &emit_delta,
        )
        .await?;

        let Some(tool_calls) = tool_calls else {
            // 无工具调用 → 最终回复（Done 携带完整内容，前端以其为准）
            let _ = on_event.send(AiEvent::Done { content: content.clone() });
            // 输出完整历史（前端下次传入；跳过 system）
            let out: Vec<Value> = history
                .iter()
                .skip(1)
                .map(|m| m.to_json())
                .collect();
            return Ok(json!({ "content": content, "messages": out }));
        };

        // 记录 assistant 的工具调用消息
        history.push(ChatMessage::Assistant {
            content: content.clone(),
            tool_calls: Some(tool_calls.clone()),
        });

        let calls = tool_calls.as_array().cloned().unwrap_or_default();
        for call in &calls {
            if cancel_token.is_cancelled() {
                return Err(AppError::Other("AI 请求已取消".into()));
            }
            let id = call.get("id").and_then(|x| x.as_str()).unwrap_or_default().to_string();
            let func = call.get("function").cloned().unwrap_or(json!({}));
            let name = func.get("name").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let args_str = func.get("arguments").and_then(|x| x.as_str()).unwrap_or("{}").to_string();
            let args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));

            let _ = on_event.send(AiEvent::Status {
                text: format!("执行工具 {name} …"),
            });

            let result = tools::execute(&name, &args, &db, &http).await;
            let (result_json, ok) = match &result {
                Ok(v) => (v.clone(), true),
                Err(e) => (json!({ "error": e.to_string() }), false),
            };
            let summary = serde_json::to_string(&result_json)
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect::<String>();
            let _ = on_event.send(AiEvent::Tool {
                name: name.clone(),
                ok,
                result: summary,
            });

            history.push(ChatMessage::Tool {
                tool_call_id: id,
                content: serde_json::to_string(&result_json)?,
            });
        }

        let _ = on_event.send(AiEvent::Status { text: "正在生成回复…".into() });
    }
}

// ---- AI 会话管理 ----

#[tauri::command]
pub fn list_ai_sessions(db: State<'_, SharedDb>) -> AppResult<Vec<AiSessionMeta>> {
    let db = db.lock().expect("db mutex poisoned");
    ai_session::list_meta(&db.conn)
}

#[tauri::command]
pub fn get_ai_session(db: State<'_, SharedDb>, id: String) -> AppResult<AiSession> {
    let db = db.lock().expect("db mutex poisoned");
    ai_session::get(&db.conn, &id)
}

#[tauri::command]
pub fn save_ai_session(db: State<'_, SharedDb>, input: AiSessionInput) -> AppResult<AiSessionMeta> {
    let db = db.lock().expect("db mutex poisoned");
    ai_session::upsert(&db.conn, &input)
}

#[tauri::command]
pub fn delete_ai_session(db: State<'_, SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    ai_session::delete(&db.conn, &id)
}
