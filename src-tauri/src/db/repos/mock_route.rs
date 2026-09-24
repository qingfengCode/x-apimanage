use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockRoute {
    pub id: String,
    pub method: String,
    pub path: String,
    pub status: i64,
    /// JSON: [{key,value}]
    pub response_headers: String,
    pub response_body: String,
    pub delay_ms: i64,
    pub enabled: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MockRouteInput {
    pub id: Option<String>,
    pub method: Option<String>,
    pub path: String,
    pub status: Option<i64>,
    pub response_headers: Option<String>,
    pub response_body: Option<String>,
    pub delay_ms: Option<i64>,
    pub enabled: Option<bool>,
}

const COLUMNS: &str = "id, method, path, status, response_headers, response_body, delay_ms, enabled, created_at, updated_at";

fn read_row(row: &rusqlite::Row) -> rusqlite::Result<MockRoute> {
    Ok(MockRoute {
        id: row.get(0)?,
        method: row.get(1)?,
        path: row.get(2)?,
        status: row.get(3)?,
        response_headers: row.get(4)?,
        response_body: row.get(5)?,
        delay_ms: row.get(6)?,
        enabled: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
    })
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<MockRoute>> {
    let sql = format!("SELECT {COLUMNS} FROM mock_routes ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], read_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 启用的路由（供 mock server 读取，已按 method/path 索引）
pub fn list_enabled(conn: &Connection) -> AppResult<Vec<MockRoute>> {
    let sql = format!("SELECT {COLUMNS} FROM mock_routes WHERE enabled = 1 ORDER BY created_at DESC");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([], read_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn upsert(conn: &Connection, input: &MockRouteInput) -> AppResult<MockRoute> {
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let method = input.method.clone().unwrap_or_else(|| "GET".to_string());
    let status = input.status.unwrap_or(200);
    let response_headers = input
        .response_headers
        .clone()
        .unwrap_or_else(|| "[]".to_string());
    let response_body = input.response_body.clone().unwrap_or_default();
    let delay_ms = input.delay_ms.unwrap_or(0);
    let enabled_b = input.enabled.unwrap_or(true);
    let enabled_i: i64 = if enabled_b { 1 } else { 0 };
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO mock_routes (id, method, path, status, response_headers, response_body, delay_ms, enabled, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
         ON CONFLICT(id) DO UPDATE SET
            method = excluded.method,
            path = excluded.path,
            status = excluded.status,
            response_headers = excluded.response_headers,
            response_body = excluded.response_body,
            delay_ms = excluded.delay_ms,
            enabled = excluded.enabled,
            updated_at = excluded.updated_at",
        params![
            id,
            method,
            input.path,
            status,
            response_headers,
            response_body,
            delay_ms,
            enabled_i,
            now,
        ],
    )?;

    let sql = format!("SELECT {COLUMNS} FROM mock_routes WHERE id = ?1");
    conn.query_row(&sql, params![id], read_row).map_err(Into::into)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM mock_routes WHERE id = ?1", params![id])?;
    Ok(())
}
