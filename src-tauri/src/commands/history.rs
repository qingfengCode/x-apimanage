use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::db::repos::history::{self, HistoryInput, HistoryRow};
use crate::error::AppResult;

#[tauri::command]
pub fn list_history(db: State<'_, crate::db::SharedDb>, limit: Option<u32>) -> AppResult<Vec<HistoryRow>> {
    let db = db.lock().expect("db mutex poisoned");
    history::list_all(&db.conn, limit.unwrap_or(500))
}

/// 单条历史（含响应快照本体，历史恢复时取当时的响应）
#[tauri::command]
pub fn get_history(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<Option<HistoryRow>> {
    let db = db.lock().expect("db mutex poisoned");
    history::get(&db.conn, &id)
}

#[tauri::command]
pub fn save_history(db: State<'_, crate::db::SharedDb>, input: HistoryInput) -> AppResult<String> {
    let db = db.lock().expect("db mutex poisoned");
    history::insert(&db.conn, &input)
}

#[tauri::command]
pub fn clear_history(db: State<'_, crate::db::SharedDb>) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    history::clear(&db.conn)
}
