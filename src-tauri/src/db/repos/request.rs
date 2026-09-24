use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};

use crate::error::AppResult;

const COLUMNS: &str = "id, collection_id, name, method, url, params, headers, body, auth, pre_script, test_script, sort_order, timeout_ms, created_at, updated_at";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestRow {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub method: String,
    pub url: Option<String>,
    pub params: Option<String>,
    pub headers: Option<String>,
    pub body: Option<String>,
    pub auth: Option<String>,
    #[serde(default)]
    pub pre_script: Option<String>,
    #[serde(default)]
    pub test_script: Option<String>,
    pub sort_order: i64,
    /// 请求级超时（毫秒）；NULL = 引擎默认 30s
    #[serde(default)]
    pub timeout_ms: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 保存请求的输入（前端传入）
/// params/headers/body/auth/pre_script/test_script 均为字符串
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct RequestInput {
    pub id: Option<String>,
    pub collection_id: String,
    pub name: String,
    pub method: String,
    pub url: Option<String>,
    pub params: Option<String>,
    pub headers: Option<String>,
    pub body: Option<String>,
    pub auth: Option<String>,
    pub pre_script: Option<String>,
    pub test_script: Option<String>,
    pub sort_order: Option<i64>,
    /// 请求级超时（毫秒），None = 保持/使用默认
    pub timeout_ms: Option<i64>,
}

fn read_row(row: &rusqlite::Row) -> rusqlite::Result<RequestRow> {
    Ok(RequestRow {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        name: row.get(2)?,
        method: row.get(3)?,
        url: row.get(4)?,
        params: row.get(5)?,
        headers: row.get(6)?,
        body: row.get(7)?,
        auth: row.get(8)?,
        pre_script: row.get(9)?,
        test_script: row.get(10)?,
        sort_order: row.get(11)?,
        timeout_ms: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

pub fn list_by_collection(conn: &Connection, collection_id: &str) -> AppResult<Vec<RequestRow>> {
    let sql = format!(
        "SELECT {COLUMNS} FROM requests WHERE collection_id = ?1 ORDER BY sort_order, name"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![collection_id], read_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<RequestRow> {
    let sql = format!("SELECT {COLUMNS} FROM requests WHERE id = ?1");
    conn.query_row(&sql, params![id], read_row).map_err(Into::into)
}

pub fn upsert(conn: &Connection, input: &RequestInput) -> AppResult<RequestRow> {
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let sort_order = input.sort_order.unwrap_or(0);
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO requests (id, collection_id, name, method, url, params, headers, body, auth, pre_script, test_script, sort_order, timeout_ms, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?14)
         ON CONFLICT(id) DO UPDATE SET
            collection_id = excluded.collection_id,
            name = excluded.name,
            method = excluded.method,
            url = excluded.url,
            params = excluded.params,
            headers = excluded.headers,
            body = excluded.body,
            auth = excluded.auth,
            pre_script = excluded.pre_script,
            test_script = excluded.test_script,
            sort_order = excluded.sort_order,
            timeout_ms = excluded.timeout_ms,
            updated_at = excluded.updated_at",
        params![
            id,
            input.collection_id,
            input.name,
            input.method,
            input.url,
            input.params,
            input.headers,
            input.body,
            input.auth,
            input.pre_script,
            input.test_script,
            sort_order,
            input.timeout_ms,
            now,
        ],
    )?;

    get(conn, &id)
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM requests WHERE id = ?1", params![id])?;
    Ok(())
}

/// 把请求移动到另一个集合/文件夹下（拖拽移动）
pub fn change_collection(conn: &Connection, id: &str, collection_id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    conn.execute(
        "UPDATE requests SET collection_id = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, collection_id, now],
    )?;
    Ok(())
}

/// 递归列出某集合（含所有子文件夹）下的所有请求。
/// 通过递归 CTE 收集所有后代 collection id，再 join 请求。
pub fn list_recursive(conn: &Connection, collection_id: &str) -> AppResult<Vec<RequestRow>> {
    let sql = format!(
        r#"
        WITH RECURSIVE descendants(id) AS (
            SELECT id FROM collections WHERE id = ?1
            UNION ALL
            SELECT c.id FROM collections c JOIN descendants d ON c.parent_id = d.id
        )
        SELECT {COLUMNS} FROM requests WHERE collection_id IN (SELECT id FROM descendants)
        ORDER BY sort_order, name
        "#
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![collection_id], read_row)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}
