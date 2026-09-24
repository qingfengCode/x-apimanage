use std::collections::HashMap;
use std::sync::{Mutex as StdMutex, OnceLock};

use tauri::State;
use tokio_util::sync::CancellationToken;

use crate::db::Db;
use crate::error::{AppError, AppResult};
use crate::http::engine::{self, HttpClients};
use crate::http::model::{HttpRequest, HttpResponse};

/// 在途 HTTP 请求的取消令牌注册表（key = 前端下发的 requestId）
static HTTP_CANCELS: OnceLock<StdMutex<HashMap<String, CancellationToken>>> = OnceLock::new();

fn cancel_registry() -> &'static StdMutex<HashMap<String, CancellationToken>> {
    HTTP_CANCELS.get_or_init(|| StdMutex::new(HashMap::new()))
}

/// Drop 时自动把 request_id 从注册表移除，避免泄漏
struct HttpCancelGuard(String);
impl Drop for HttpCancelGuard {
    fn drop(&mut self) {
        cancel_registry().lock().unwrap().remove(&self.0);
    }
}

#[tauri::command]
pub async fn send_http_request(
    clients: State<'_, HttpClients>,
    _db: State<'_, crate::db::SharedDb>,
    req: HttpRequest,
    request_id: Option<String>,
) -> AppResult<HttpResponse> {
    // 取出当前客户端（代理设置变更后是新的实例），后续请求不再读 State
    let client = clients.get();
    // 不带 requestId 的调用方（旧 Runner 等）不支持终止，行为不变
    let Some(request_id) = request_id else {
        return engine::execute(&client, req).await;
    };
    let token = CancellationToken::new();
    cancel_registry()
        .lock()
        .unwrap()
        .insert(request_id.clone(), token.clone());
    let _guard = HttpCancelGuard(request_id);
    // 令牌触发时放弃 execute future，reqwest 请求随 drop 真实中止
    tokio::select! {
        res = engine::execute(&client, req) => res,
        _ = token.cancelled() => Err(AppError::Cancelled),
    }
}

/// 前端点"终止"时调用：真实中止在途的后端请求
#[tauri::command]
pub fn cancel_http_request(request_id: String) -> AppResult<()> {
    if let Some(token) = cancel_registry().lock().unwrap().remove(&request_id) {
        token.cancel();
    }
    Ok(())
}
