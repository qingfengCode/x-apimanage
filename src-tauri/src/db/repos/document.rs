use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// 文档（AI 生成或手写的 API 文档 / 产品文档，Markdown）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    /// api | product
    pub doc_type: String,
    pub title: String,
    pub content: String,
    /// 关联的请求 id（可选）
    pub request_id: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct DocumentInput {
    pub id: Option<String>,
    pub doc_type: String,
    pub title: String,
    pub content: String,
    pub request_id: Option<String>,
}

const COLUMNS: &str = "id, doc_type, title, content, request_id, created_at, updated_at";

fn read_row(row: &rusqlite::Row) -> rusqlite::Result<Document> {
    Ok(Document {
        id: row.get(0)?,
        doc_type: row.get(1)?,
        title: row.get(2)?,
        content: row.get(3)?,
        request_id: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<Document>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM documents ORDER BY updated_at DESC"
    ))?;
    let rows = stmt.query_map([], read_row)?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Document> {
    conn.query_row(
        &format!("SELECT {COLUMNS} FROM documents WHERE id = ?1"),
        [id],
        read_row,
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => {
            AppError::Other(format!("文档不存在: {id}"))
        }
        other => AppError::Db(other),
    })
}

pub fn upsert(conn: &Connection, input: &DocumentInput) -> AppResult<Document> {
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO documents (id, doc_type, title, content, request_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
         ON CONFLICT(id) DO UPDATE SET
           doc_type = excluded.doc_type,
           title = excluded.title,
           content = excluded.content,
           request_id = excluded.request_id,
           updated_at = excluded.updated_at",
        rusqlite::params![
            id,
            input.doc_type,
            input.title,
            input.content,
            input.request_id,
            now
        ],
    )?;
    get(conn, &id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM documents WHERE id = ?1", [id])?;
    Ok(())
}
