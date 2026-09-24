use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    /// JSON: [{key,value,enabled}]
    pub variables: String,
    pub is_active: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentInput {
    pub id: Option<String>,
    pub name: String,
    /// JSON: [{key,value,enabled}]
    pub variables: Option<String>,
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<Environment>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, variables, is_active, created_at, updated_at FROM environments ORDER BY name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Environment {
            id: row.get(0)?,
            name: row.get(1)?,
            variables: row.get(2)?,
            is_active: row.get::<_, i64>(3)? != 0,
            created_at: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn upsert(conn: &Connection, input: &EnvironmentInput) -> AppResult<Environment> {
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let variables = input.variables.clone().unwrap_or_else(|| "[]".to_string());
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO environments (id, name, variables, is_active, created_at, updated_at)
         VALUES (?1, ?2, ?3, 0, ?4, ?4)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            variables = excluded.variables,
            updated_at = excluded.updated_at",
        params![id, input.name, variables, now],
    )?;

    // 返回最新数据
    get(conn, &id)
}

pub fn get(conn: &Connection, id: &str) -> AppResult<Environment> {
    conn.query_row(
        "SELECT id, name, variables, is_active, created_at, updated_at FROM environments WHERE id = ?1",
        params![id],
        |row| {
            Ok(Environment {
                id: row.get(0)?,
                name: row.get(1)?,
                variables: row.get(2)?,
                is_active: row.get::<_, i64>(3)? != 0,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        },
    )
    .map_err(Into::into)
}

pub fn set_active(conn: &Connection, id: &str) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    let tx = conn.unchecked_transaction()?;
    tx.execute("UPDATE environments SET is_active = 0", [])?;
    let affected = tx.execute(
        "UPDATE environments SET is_active = 1, updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    // id 不存在时直接清空所有活动标志会静默丢失“当前环境”，必须报错并回滚
    if affected == 0 {
        return Err(AppError::Other(format!("环境 {id} 不存在，无法设为当前环境")));
    }
    tx.commit()?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM environments WHERE id = ?1", params![id])?;
    Ok(())
}
