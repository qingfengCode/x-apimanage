use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryRow {
    pub id: String,
    pub method: Option<String>,
    pub url: Option<String>,
    pub status: Option<i64>,
    pub time_ms: Option<i64>,
    pub size: Option<i64>,
    pub created_at: i64,
    /// 完整请求快照（JSON 字符串），用于"从历史恢复"。
    /// 含 name/method/url/params/headers/body/auth/preScript/testScript。
    #[serde(default)]
    pub request_snapshot: Option<String>,
    /// 是否存有响应快照（列表查询不取快照本体，只取存在性标记）。
    #[serde(default)]
    pub has_response: bool,
    /// 响应快照（JSON 字符串），仅 get 单条查询时填充；列表查询恒为 None。
    #[serde(default)]
    pub response_snapshot: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryInput {
    pub method: String,
    pub url: String,
    pub status: Option<i64>,
    pub time_ms: Option<i64>,
    pub size: Option<i64>,
    /// 可选的完整请求快照 JSON
    #[serde(default)]
    pub request_snapshot: Option<String>,
    /// 可选的响应快照 JSON（状态/响应头/响应体），恢复历史时直接查看
    #[serde(default)]
    pub response_snapshot: Option<String>,
}

/// 列表查询列（不含 response_snapshot 本体，避免大字段随 500 条列表整体加载）
const COLUMNS: &str = "id, method, url, status, time_ms, size, created_at, request_snapshot, (response_snapshot IS NOT NULL)";

fn read_row(row: &rusqlite::Row) -> rusqlite::Result<HistoryRow> {
    Ok(HistoryRow {
        id: row.get(0)?,
        method: row.get(1)?,
        url: row.get(2)?,
        status: row.get(3)?,
        time_ms: row.get(4)?,
        size: row.get(5)?,
        created_at: row.get(6)?,
        request_snapshot: row.get(7)?,
        has_response: row.get::<_, i64>(8)? != 0,
        response_snapshot: None,
    })
}

pub fn list_all(conn: &Connection, limit: u32) -> AppResult<Vec<HistoryRow>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM history ORDER BY created_at DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![limit], read_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 单条查询（含响应快照本体，供历史恢复时取当时的响应）
pub fn get(conn: &Connection, id: &str) -> AppResult<Option<HistoryRow>> {
    let sql = format!(
        "SELECT id, method, url, status, time_ms, size, created_at, request_snapshot, 1, response_snapshot
         FROM history WHERE id = ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut rows = stmt.query_map(params![id], |row| {
        Ok(HistoryRow {
            id: row.get(0)?,
            method: row.get(1)?,
            url: row.get(2)?,
            status: row.get(3)?,
            time_ms: row.get(4)?,
            size: row.get(5)?,
            created_at: row.get(6)?,
            request_snapshot: row.get(7)?,
            has_response: true,
            response_snapshot: row.get(9)?,
        })
    })?;
    match rows.next() {
        Some(r) => Ok(Some(r?)),
        None => Ok(None),
    }
}

pub fn insert(conn: &Connection, input: &HistoryInput) -> AppResult<String> {
    let id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "INSERT INTO history (id, method, url, status, time_ms, size, created_at, request_snapshot, response_snapshot)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            input.method,
            input.url,
            input.status,
            input.time_ms,
            input.size,
            now,
            input.request_snapshot,
            input.response_snapshot,
        ],
    )?;
    // 落库端同样裁剪：界面只显示最近 500 条，DB 也只需保留近期记录，
    // 避免旧行无限堆积（尤其 request_snapshot/response_snapshot 可能较大）
    conn.execute(
        "DELETE FROM history WHERE id NOT IN (
            SELECT id FROM history ORDER BY created_at DESC, rowid DESC LIMIT ?1
        )",
        params![1000i64],
    )?;
    Ok(id)
}

pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM history", [])?;
    Ok(())
}
