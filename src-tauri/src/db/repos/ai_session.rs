use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};

/// AI 会话（items 为 UI 条目 JSON，history 为传给模型的 [{role,content}] JSON）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSession {
    pub id: String,
    pub title: String,
    pub items: String,
    pub history: String,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 列表用元数据（不加载大字段）
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSessionMeta {
    pub id: String,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSessionInput {
    pub id: Option<String>,
    pub title: String,
    pub items: String,
    pub history: String,
}

pub fn list_meta(conn: &Connection) -> AppResult<Vec<AiSessionMeta>> {
    let mut stmt =
        conn.prepare("SELECT id, title, created_at, updated_at FROM ai_sessions ORDER BY updated_at DESC")?;
    let rows = stmt.query_map([], |r| {
        Ok(AiSessionMeta {
            id: r.get(0)?,
            title: r.get(1)?,
            created_at: r.get(2)?,
            updated_at: r.get(3)?,
        })
    })?;
    Ok(rows.collect::<Result<Vec<_>, _>>()?)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<AiSession> {
    conn.query_row(
        "SELECT id, title, items, history, created_at, updated_at FROM ai_sessions WHERE id = ?1",
        [id],
        |r| {
            Ok(AiSession {
                id: r.get(0)?,
                title: r.get(1)?,
                items: r.get(2)?,
                history: r.get(3)?,
                created_at: r.get(4)?,
                updated_at: r.get(5)?,
            })
        },
    )
    .map_err(|e| match e {
        rusqlite::Error::QueryReturnedNoRows => AppError::Other(format!("会话不存在: {id}")),
        other => AppError::Db(other),
    })
}

pub fn upsert(conn: &Connection, input: &AiSessionInput) -> AppResult<AiSessionMeta> {
    let id = input
        .id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO ai_sessions (id, title, items, history, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?5)
         ON CONFLICT(id) DO UPDATE SET
           title = excluded.title,
           items = excluded.items,
           history = excluded.history,
           updated_at = excluded.updated_at",
        rusqlite::params![id, input.title, input.items, input.history, now],
    )?;
    Ok(AiSessionMeta {
        id,
        title: input.title.clone(),
        created_at: now,
        updated_at: now,
    })
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM ai_sessions WHERE id = ?1", [id])?;
    Ok(())
}
