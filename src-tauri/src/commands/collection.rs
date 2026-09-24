use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::db::repos::collection::{self, Collection, CollectionInput};
use crate::error::AppResult;

#[tauri::command]
pub fn list_collections(db: State<'_, crate::db::SharedDb>) -> AppResult<Vec<Collection>> {
    let db = db.lock().expect("db mutex poisoned");
    collection::list_all(&db.conn)
}

#[tauri::command]
pub fn create_collection(db: State<'_, crate::db::SharedDb>, input: CollectionInput) -> AppResult<Collection> {
    let db = db.lock().expect("db mutex poisoned");
    collection::upsert(&db.conn, &input)
}

#[tauri::command]
pub fn update_collection(db: State<'_, crate::db::SharedDb>, input: CollectionInput) -> AppResult<Collection> {
    let db = db.lock().expect("db mutex poisoned");
    collection::upsert(&db.conn, &input)
}

#[tauri::command]
pub fn delete_collection(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    collection::delete(&db.conn, &id)
}

#[tauri::command]
pub fn move_node(
    db: State<'_, crate::db::SharedDb>,
    id: String,
    new_parent_id: Option<String>,
    sort_order: Option<i64>,
) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    collection::move_node(&db.conn, &id, new_parent_id.as_deref(), sort_order)
}
