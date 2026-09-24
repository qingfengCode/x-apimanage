use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::db::repos::request::{self, RequestInput, RequestRow};
use crate::error::AppResult;

#[tauri::command]
pub fn list_requests(db: State<'_, crate::db::SharedDb>, collection_id: String) -> AppResult<Vec<RequestRow>> {
    let db = db.lock().expect("db mutex poisoned");
    request::list_by_collection(&db.conn, &collection_id)
}

#[tauri::command]
pub fn get_request(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<RequestRow> {
    let db = db.lock().expect("db mutex poisoned");
    request::get(&db.conn, &id)
}

#[tauri::command]
pub fn save_request(db: State<'_, crate::db::SharedDb>, input: RequestInput) -> AppResult<RequestRow> {
    let db = db.lock().expect("db mutex poisoned");
    request::upsert(&db.conn, &input)
}

#[tauri::command]
pub fn delete_request(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    request::delete(&db.conn, &id)
}

/// 把请求移动到指定集合/文件夹（拖拽移动）
#[tauri::command]
pub fn move_request(
    db: State<'_, crate::db::SharedDb>,
    id: String,
    collection_id: String,
) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    request::change_collection(&db.conn, &id, &collection_id)
}
