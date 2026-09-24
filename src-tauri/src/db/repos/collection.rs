use serde::{Deserialize, Serialize};

use rusqlite::{params, Connection};

use crate::error::AppResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub parent_id: Option<String>,
    /// collection | folder
    pub kind: String,
    pub description: Option<String>,
    pub sort_order: i64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionInput {
    pub id: Option<String>,
    pub name: String,
    pub parent_id: Option<String>,
    pub kind: Option<String>,
    pub description: Option<String>,
    pub sort_order: Option<i64>,
}

pub fn list_all(conn: &Connection) -> AppResult<Vec<Collection>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, parent_id, kind, description, sort_order, created_at, updated_at
         FROM collections ORDER BY sort_order, name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(Collection {
            id: row.get(0)?,
            name: row.get(1)?,
            parent_id: row.get(2)?,
            kind: row.get(3)?,
            description: row.get(4)?,
            sort_order: row.get(5)?,
            created_at: row.get(6)?,
            updated_at: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

pub fn upsert(conn: &Connection, input: &CollectionInput) -> AppResult<Collection> {
    let id = input.id.clone().unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let kind = input.kind.clone().unwrap_or_else(|| "collection".to_string());
    let sort_order = input.sort_order.unwrap_or(0);
    let now = chrono::Utc::now().timestamp_millis();

    conn.execute(
        "INSERT INTO collections (id, name, parent_id, kind, description, sort_order, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            parent_id = excluded.parent_id,
            kind = excluded.kind,
            description = excluded.description,
            sort_order = excluded.sort_order,
            updated_at = excluded.updated_at",
        params![
            id,
            input.name,
            input.parent_id,
            kind,
            input.description,
            sort_order,
            now,
        ],
    )?;

    Ok(Collection {
        id,
        name: input.name.clone(),
        parent_id: input.parent_id.clone(),
        kind,
        description: input.description.clone(),
        sort_order,
        created_at: now,
        updated_at: now,
    })
}

pub fn delete(conn: &Connection, id: &str) -> AppResult<()> {
    conn.execute("DELETE FROM collections WHERE id = ?1", params![id])?;
    Ok(())
}

/// 移动节点到新父节点下
pub fn move_node(conn: &Connection, id: &str, new_parent_id: Option<&str>, sort_order: Option<i64>) -> AppResult<()> {
    let now = chrono::Utc::now().timestamp_millis();
    match (new_parent_id, sort_order) {
        (Some(p), Some(s)) => {
            conn.execute(
                "UPDATE collections SET parent_id = ?1, sort_order = ?2, updated_at = ?3 WHERE id = ?4",
                params![p, s, now, id],
            )?;
        }
        (Some(p), None) => {
            conn.execute(
                "UPDATE collections SET parent_id = ?1, updated_at = ?2 WHERE id = ?3",
                params![p, now, id],
            )?;
        }
        (None, Some(s)) => {
            conn.execute(
                "UPDATE collections SET parent_id = NULL, sort_order = ?1, updated_at = ?2 WHERE id = ?3",
                params![s, now, id],
            )?;
        }
        (None, None) => {
            conn.execute(
                "UPDATE collections SET parent_id = NULL, updated_at = ?1 WHERE id = ?2",
                params![now, id],
            )?;
        }
    }
    Ok(())
}
