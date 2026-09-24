use std::net::SocketAddr;
use std::sync::Arc;

use tauri::State;
use tokio::sync::Mutex;

use crate::db::Db;
use crate::db::repos::mock_route::{self, MockRoute, MockRouteInput};
use crate::error::{AppError, AppResult};
use crate::mock::server::{self, MockLogEntry, MockServer, SharedLogs, SharedRoutes};

/// Mock server 运行时状态
#[derive(Default)]
pub struct MockState {
    /// 运行中的 server 句柄（None 表示未启动）
    pub running: tokio::sync::Mutex<Option<MockServer>>,
    /// 运行时路由表（server 读取）
    pub routes: SharedRoutes,
    /// 当前监听地址
    pub addr: tokio::sync::Mutex<Option<SocketAddr>>,
    /// 最近请求日志（server 写入，命令读取）
    pub logs: SharedLogs,
}

#[tauri::command]
pub fn list_mock_routes(db: State<'_, crate::db::SharedDb>) -> AppResult<Vec<MockRoute>> {
    let db = db.lock().expect("db mutex poisoned");
    mock_route::list_all(&db.conn)
}

#[tauri::command]
pub fn save_mock_route(
    db: State<'_, crate::db::SharedDb>,
    input: MockRouteInput,
) -> AppResult<MockRoute> {
    let row = {
        let db = db.lock().expect("db mutex poisoned");
        mock_route::upsert(&db.conn, &input)?
    };
    Ok(row)
}

#[tauri::command]
pub fn delete_mock_route(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    mock_route::delete(&db.conn, &id)
}

/// 启动 mock server。会先从 DB 加载所有 enabled 路由到内存。
#[tauri::command]
pub async fn start_mock_server(
    db: State<'_, crate::db::SharedDb>,
    mock_state: State<'_, MockState>,
    port: Option<u16>,
) -> AppResult<String> {
    // 检查+启动+登记全程持有同一把锁：并发双击不会同时通过检查导致
    // 泄漏无法停止的重复实例
    let mut running = mock_state.running.lock().await;
    if running.is_some() {
        let addr = mock_state.addr.lock().await;
        if let Some(a) = addr.as_ref() {
            let url = format!("http://{}", a);
            // 已运行时若用户明确要求换端口，提示先停止，避免“以为换端口成功”
            if let Some(p) = port {
                if p != 0 && p != a.port() {
                    return Err(AppError::Other(format!(
                        "Mock 服务已在 {url} 运行，请先停止再更换端口"
                    )));
                }
            }
            return Ok(url);
        }
    }

    // 从 DB 加载路由
    let routes = {
        let db = db.lock().expect("db mutex poisoned");
        mock_route::list_enabled(&db.conn)?
    };
    // 重置共享路由
    {
        let mut r = mock_state.routes.lock().await;
        r.clear();
        r.extend(routes);
    }

    let port = port.unwrap_or(0); // 0 = 自动分配
    let (addr, cancel, handle) =
        server::start(port, mock_state.routes.clone(), mock_state.logs.clone()).await?;
    *running = Some(MockServer {
        addr,
        cancel,
        handle,
    });
    *mock_state.addr.lock().await = Some(addr);
    Ok(format!("http://{}", addr))
}

/// 最近请求日志（新→旧，最多 200 条）
#[tauri::command]
pub fn list_mock_logs(mock_state: State<'_, MockState>) -> AppResult<Vec<MockLogEntry>> {
    let logs = mock_state
        .logs
        .lock()
        .map_err(|e| AppError::Other(format!("logs mutex poisoned: {e}")))?;
    let mut out = logs.clone();
    out.reverse();
    Ok(out)
}

/// 清空请求日志
#[tauri::command]
pub fn clear_mock_logs(mock_state: State<'_, MockState>) -> AppResult<()> {
    mock_state
        .logs
        .lock()
        .map_err(|e| AppError::Other(format!("logs mutex poisoned: {e}")))?
        .clear();
    Ok(())
}

/// 停止 mock server
#[tauri::command]
pub async fn stop_mock_server(mock_state: State<'_, MockState>) -> AppResult<()> {
    let server_opt = mock_state.running.lock().await.take();
    if let Some(srv) = server_opt {
        srv.cancel.cancel();
        // 等待任务结束（忽略错误）
        let _ = srv.handle.await;
    }
    *mock_state.addr.lock().await = None;
    Ok(())
}

/// 查询状态与地址
#[tauri::command]
pub async fn mock_server_status(mock_state: State<'_, MockState>) -> AppResult<Option<String>> {
    let running = mock_state.running.lock().await;
    if running.is_some() {
        let addr = mock_state.addr.lock().await;
        Ok(addr.as_ref().map(|a| format!("http://{}", a)))
    } else {
        Ok(None)
    }
}

/// 刷新运行时路由表（编辑路由后调用，无需重启 server）
#[tauri::command]
pub async fn refresh_mock_routes(
    db: State<'_, crate::db::SharedDb>,
    mock_state: State<'_, MockState>,
) -> AppResult<()> {
    let routes = {
        let db = db.lock().expect("db mutex poisoned");
        mock_route::list_enabled(&db.conn)?
    };
    let mut r = mock_state.routes.lock().await;
    r.clear();
    r.extend(routes);
    Ok(())
}
