mod ai;
mod commands;
mod db;
mod error;
mod http;
mod mock;
mod mcp_server;
mod updater;

use std::sync::{Arc, Mutex};
use tauri::Manager;

use commands::mock::MockState;
use db::Db;
use mcp_server::McpState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // 初始化数据库
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to resolve app_data_dir");
            std::fs::create_dir_all(&app_data_dir).ok();
            let db_path = app_data_dir.join("x-apimanage.db");
            let db = Db::open(&db_path).expect("failed to open db");
            db.run_migrations().expect("failed to run migrations");

            // 共享的 HTTP 客户端（带 cookie store）
            let http_client = http::engine::build_client();

            // Mock server / MCP server 运行时状态
            let mock_state = MockState::default();
            let mcp_state = McpState::default();

            let shared_db: Arc<Mutex<Db>> = Arc::new(Mutex::new(db));
            app.manage(shared_db.clone());
            app.manage(http_client.clone());
            app.manage(mock_state);
            app.manage(mcp_state);

            // MCP 按设置自动启动（读取设置失败不阻断应用）
            let settings = {
                let conn = shared_db.lock().expect("db mutex poisoned");
                db::repos::ai_setting::get(&conn.conn)
            };
            if let Ok(s) = settings {
                if s.mcp_enabled && (1..=65535).contains(&s.mcp_port) {
                    let port = s.mcp_port as u16;
                    let app_handle = app.handle().clone();
                    tauri::async_runtime::spawn(async move {
                        match mcp_server::start(port, shared_db, http_client).await {
                            Ok(server) => {
                                println!("[mcp] listening on http://{}/mcp", server.addr);
                                // 登记进 McpState：否则状态查询永远“未运行”，
                                // 且 UI 关闭 MCP 时无法取消这个孤儿实例
                                let state = app_handle.state::<McpState>();
                                *state.addr.lock().expect("mcp state poisoned") = Some(server.addr);
                                *state.running.lock().expect("mcp state poisoned") = Some(server);
                            }
                            Err(e) => println!("[mcp] auto-start failed: {e}"),
                        }
                    });
                }
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // http
            commands::http::send_http_request,
            commands::http::cancel_http_request,
            // io
            commands::io::read_text_file,
            commands::io::write_text_file,
            commands::io::write_binary_file,
            commands::io::list_requests_recursive,
            // collection
            commands::collection::list_collections,
            commands::collection::create_collection,
            commands::collection::update_collection,
            commands::collection::delete_collection,
            commands::collection::move_node,
            // request
            commands::request::list_requests,
            commands::request::get_request,
            commands::request::save_request,
            commands::request::delete_request,
            commands::request::move_request,
            // environment
            commands::environment::list_environments,
            commands::environment::save_environment,
            commands::environment::set_active_environment,
            commands::environment::delete_environment,
            // history
            commands::history::list_history,
            commands::history::get_history,
            commands::history::save_history,
            commands::history::clear_history,
            // mock
            commands::mock::list_mock_routes,
            commands::mock::save_mock_route,
            commands::mock::delete_mock_route,
            commands::mock::start_mock_server,
            commands::mock::stop_mock_server,
            commands::mock::mock_server_status,
            commands::mock::refresh_mock_routes,
            commands::mock::list_mock_logs,
            commands::mock::clear_mock_logs,
            // update（应用自更新）
            commands::update::update_get_info,
            commands::update::update_get_manifest_url,
            commands::update::update_set_manifest_url,
            commands::update::update_check,
            commands::update::update_download,
            commands::update::update_install_and_exit,
            // document（AI 文档）
            commands::document::list_documents,
            commands::document::get_document,
            commands::document::save_document,
            commands::document::delete_document,
            // ai
            commands::ai::get_ai_settings,
            commands::ai::save_ai_settings,
            commands::ai::test_ai_connection,
            commands::ai::ai_chat,
            commands::ai::cancel_ai_chat,
            commands::ai::list_ai_sessions,
            commands::ai::get_ai_session,
            commands::ai::save_ai_session,
            commands::ai::delete_ai_session,
            // mcp
            commands::mcp::mcp_server_status,
            commands::mcp::set_mcp_server,
            commands::mcp::generate_mcp_token,
            commands::mcp::revoke_mcp_token,
            commands::mcp::mcp_server_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
