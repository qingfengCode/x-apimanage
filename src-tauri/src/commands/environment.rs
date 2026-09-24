use std::sync::Mutex;
use tauri::State;

use crate::db::Db;
use crate::db::repos::environment::{self, Environment, EnvironmentInput};
use crate::error::AppResult;

#[tauri::command]
pub fn list_environments(db: State<'_, crate::db::SharedDb>) -> AppResult<Vec<Environment>> {
    let db = db.lock().expect("db mutex poisoned");
    environment::list_all(&db.conn)
}

#[tauri::command]
pub fn save_environment(
    db: State<'_, crate::db::SharedDb>,
    input: EnvironmentInput,
) -> AppResult<Environment> {
    let db = db.lock().expect("db mutex poisoned");
    let env = environment::upsert(&db.conn, &input)?;
    Ok(env)
}

#[tauri::command]
pub fn set_active_environment(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    environment::set_active(&db.conn, &id)
}

#[tauri::command]
pub fn delete_environment(db: State<'_, crate::db::SharedDb>, id: String) -> AppResult<()> {
    let db = db.lock().expect("db mutex poisoned");
    environment::delete(&db.conn, &id)
}
