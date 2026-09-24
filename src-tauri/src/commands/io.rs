use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::db::repos::request;
use crate::error::AppResult;

/// 读取文本文件（Postman 导入用）
#[tauri::command]
pub fn read_text_file(path: String) -> AppResult<String> {
    Ok(std::fs::read_to_string(&path)?)
}

/// 写入文本文件（Postman 导出用）
#[tauri::command]
pub fn write_text_file(path: String, content: String) -> AppResult<()> {
    std::fs::write(&path, content)?;
    Ok(())
}

/// 写入二进制文件（响应体保存用，前端传 base64）
#[tauri::command]
pub fn write_binary_file(path: String, base64_data: String) -> AppResult<()> {
    use base64::Engine as _;
    let bytes = base64::engine::general_purpose::STANDARD.decode(base64_data.as_bytes())?;
    std::fs::write(&path, bytes)?;
    Ok(())
}

/// 递归列出某集合（含子文件夹）下所有请求（导出用）
#[tauri::command]
pub fn list_requests_recursive(
    db: State<'_, crate::db::SharedDb>,
    collection_id: String,
) -> AppResult<Vec<request::RequestRow>> {
    let db = db.lock().expect("db mutex poisoned");
    request::list_recursive(&db.conn, &collection_id)
}
