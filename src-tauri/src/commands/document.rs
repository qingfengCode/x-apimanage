use tauri::State;

use crate::db::repos::document::{self, Document, DocumentInput};
use crate::db::SharedDb;
use crate::error::AppResult;

#[tauri::command]
pub fn list_documents(db: State<'_, SharedDb>) -> AppResult<Vec<Document>> {
    let db = db.lock().expect("db mutex poisoned");
    document::list_all(&db.conn)
}

#[tauri::command]
pub fn save_document(db: State<'_, SharedDb>, input: DocumentInput) -> AppResult<Document> {
    let db = db.lock().expect("db mutex poisoned");
    document::upsert(&db.conn, &input)
}

#[tauri::command]
pub fn get_document(db: State<'_, SharedDb>, id: String) -> AppResult<Document> {
    let db = db.lock().expect("db mutex poisoned");
    document::get(&db.conn, &id)
}

#[tauri::command]
pub fn delete_document(db: State<'_, SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    document::delete(&db.conn, &id)
}
