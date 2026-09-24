use serde_json::Value;
use tauri::State;

use crate::db::repos::ai_setting;
use crate::db::SharedDb;
use crate::error::{AppError, AppResult};
use crate::mcp_server::{self, McpState};

#[tauri::command]
pub fn mcp_server_status(state: State<'_, McpState>) -> AppResult<Option<String>> {
    let running = state.running.lock().expect("mcp state poisoned");
    Ok(running.as_ref().map(|s| format!("http://{}", s.addr)))
}

/// 启动/停止 MCP 服务。port 为 None 时使用设置里的端口。
#[tauri::command]
pub async fn set_mcp_server(
    db: State<'_, SharedDb>,
    http: State<'_, reqwest::Client>,
    state: State<'_, McpState>,
    enabled: bool,
    port: Option<u64>,
) -> AppResult<Option<String>> {
    // 先停掉运行中的实例
    {
        let mut running = state.running.lock().expect("mcp state poisoned");
        if let Some(server) = running.take() {
            server.cancel.cancel();
        }
        *state.addr.lock().expect("mcp state poisoned") = None;
    }

    if !enabled {
        // 持久化关闭
        let db = db.lock().expect("db mutex poisoned");
        let mut s = ai_setting::get(&db.conn)?;
        s.mcp_enabled = false;
        if let Some(p) = port {
            s.mcp_port = p as i64;
        }
        ai_setting::save(&db.conn, &s)?;
        return Ok(None);
    }

    // 读取/落库端口配置
    let port_num = {
        let db = db.lock().expect("db mutex poisoned");
        let mut s = ai_setting::get(&db.conn)?;
        if let Some(p) = port {
            if p == 0 || p > 65535 {
                return Err(AppError::Other(format!("端口必须在 1-65535 之间，收到 {p}")));
            }
            s.mcp_port = p as i64;
        }
        // 历史脏数据自愈：越界端口不再静默截断（70000 曾经会被截成 4464）
        if s.mcp_port < 1 || s.mcp_port > 65535 {
            s.mcp_port = 8765;
        }
        s.mcp_enabled = true;
        let p = s.mcp_port as u16;
        ai_setting::save(&db.conn, &s)?;
        p
    };

    let db: SharedDb = db.inner().clone();
    let http = http.inner().clone();
    let server = mcp_server::start(port_num, db, http)
        .await
        .map_err(AppError::Other)?;

    let url = format!("http://{}/mcp", server.addr);
    *state.addr.lock().expect("mcp state poisoned") = Some(server.addr);
    *state.running.lock().expect("mcp state poisoned") = Some(server);
    Ok(Some(url))
}

/// 生成新的 MCP 访问密钥（32 位 hex），保存并按需重启服务（切换绑定模式）
#[tauri::command]
pub async fn generate_mcp_token(
    db: State<'_, SharedDb>,
    http: State<'_, reqwest::Client>,
    state: State<'_, McpState>,
) -> AppResult<String> {
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
    .chars()
    .take(32)
    .collect::<String>();
    apply_token(&db, &http, &state, token.clone()).await?;
    Ok(token)
}

/// 吊销访问密钥（恢复仅本机回环、无需鉴权）
#[tauri::command]
pub async fn revoke_mcp_token(
    db: State<'_, SharedDb>,
    http: State<'_, reqwest::Client>,
    state: State<'_, McpState>,
) -> AppResult<()> {
    apply_token(&db, &http, &state, String::new()).await
}

/// 落库密钥；服务运行中则重启以应用绑定地址变化
async fn apply_token(
    db: &State<'_, SharedDb>,
    http: &State<'_, reqwest::Client>,
    state: &State<'_, McpState>,
    token: String,
) -> AppResult<()> {
    let was_running = {
        let running = state.running.lock().expect("mcp state poisoned");
        running.is_some()
    };
    if was_running {
        // 先停（set_mcp_server 内部会重新读取设置并重启）
        let mut running = state.running.lock().expect("mcp state poisoned");
        if let Some(server) = running.take() {
            server.cancel.cancel();
        }
        *state.addr.lock().expect("mcp state poisoned") = None;
    }
    {
        let conn = db.lock().expect("db mutex poisoned");
        let mut s = ai_setting::get(&conn.conn)?;
        s.mcp_token = token;
        ai_setting::save(&conn.conn, &s)?;
    }
    if was_running {
        let (db_clone, http_clone) = (db.inner().clone(), http.inner().clone());
        let port = {
            let conn = db.lock().expect("db mutex poisoned");
            ai_setting::get(&conn.conn)?.mcp_port as u16
        };
        let server = mcp_server::start(port, db_clone, http_clone)
            .await
            .map_err(AppError::Other)?;
        *state.addr.lock().expect("mcp state poisoned") = Some(server.addr);
        *state.running.lock().expect("mcp state poisoned") = Some(server);
    }
    Ok(())
}

/// MCP 服务完整信息（状态/地址/密钥），供面板展示与生成客户端配置
#[tauri::command]
pub fn mcp_server_info(db: State<'_, SharedDb>, state: State<'_, McpState>) -> AppResult<Value> {
    let (token, port) = {
        let conn = db.lock().expect("db mutex poisoned");
        let s = ai_setting::get(&conn.conn)?;
        (s.mcp_token, s.mcp_port)
    };
    let running = {
        let running = state.running.lock().expect("mcp state poisoned");
        running.is_some()
    };
    let url = {
        let addr = state.addr.lock().expect("mcp state poisoned");
        addr.as_ref().map(|a| format!("http://{}/mcp", a))
    };
    let lan = mcp_server::lan_ip().unwrap_or_else(|| "127.0.0.1".into());
    Ok(serde_json::json!({
        "running": running,
        "url": url,
        "port": port,
        "token": token,
        "lanIp": lan,
    }))
}
