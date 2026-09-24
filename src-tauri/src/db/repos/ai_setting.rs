use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;

/// AI / MCP 配置（单行表 id=1）
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSettings {
    /// OpenAI 兼容 API 地址，如 https://api.deepseek.com/v1
    pub base_url: String,
    pub api_key: String,
    /// 模型名，如 deepseek-chat / gpt-4o-mini
    pub model: String,
    /// 追加到默认系统提示词后的自定义指令
    pub system_prompt: String,
    pub mcp_enabled: bool,
    pub mcp_port: i64,
    /// MCP 访问密钥（非空时要求 Bearer 鉴权并绑定 0.0.0.0）
    pub mcp_token: String,
}

pub fn get(conn: &Connection) -> AppResult<AiSettings> {
    let row = conn.query_row(
        "SELECT base_url, api_key, model, system_prompt, mcp_enabled, mcp_port, mcp_token
         FROM ai_settings WHERE id = 1",
        [],
        |r| {
            Ok(AiSettings {
                base_url: r.get(0)?,
                api_key: r.get(1)?,
                model: r.get(2)?,
                system_prompt: r.get(3)?,
                mcp_enabled: r.get::<_, i64>(4)? != 0,
                mcp_port: r.get(5)?,
                mcp_token: r.get(6)?,
            })
        },
    );
    match row {
        Ok(s) => Ok(s),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AiSettings::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(conn: &Connection, s: &AiSettings) -> AppResult<()> {
    conn.execute(
        "INSERT INTO ai_settings (id, base_url, api_key, model, system_prompt, mcp_enabled, mcp_port, mcp_token)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(id) DO UPDATE SET
           base_url = excluded.base_url,
           api_key = excluded.api_key,
           model = excluded.model,
           system_prompt = excluded.system_prompt,
           mcp_enabled = excluded.mcp_enabled,
           mcp_port = excluded.mcp_port,
           mcp_token = excluded.mcp_token",
        rusqlite::params![
            s.base_url,
            s.api_key,
            s.model,
            s.system_prompt,
            s.mcp_enabled as i64,
            s.mcp_port,
            s.mcp_token
        ],
    )?;
    Ok(())
}
