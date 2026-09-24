use rusqlite::Connection;

use crate::error::AppResult;
use crate::http::proxy::ProxySettings;

/// 应用级设置（单行表 id=1）。目前只有出站 HTTP 代理，后续可继续加列。
#[derive(Debug, Clone, Default)]
pub struct AppSettings {
    pub proxy: ProxySettings,
}

pub fn get(conn: &Connection) -> AppResult<AppSettings> {
    let row = conn.query_row(
        "SELECT proxy_enabled, proxy_url, proxy_bypass FROM app_settings WHERE id = 1",
        [],
        |r| {
            Ok(AppSettings {
                proxy: ProxySettings {
                    enabled: r.get::<_, i64>(0)? != 0,
                    url: r.get(1)?,
                    bypass: r.get(2)?,
                },
            })
        },
    );
    match row {
        Ok(s) => Ok(s),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppSettings::default()),
        Err(e) => Err(e.into()),
    }
}

pub fn save(conn: &Connection, s: &AppSettings) -> AppResult<()> {
    conn.execute(
        "INSERT INTO app_settings (id, proxy_enabled, proxy_url, proxy_bypass)
         VALUES (1, ?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET
           proxy_enabled = excluded.proxy_enabled,
           proxy_url = excluded.proxy_url,
           proxy_bypass = excluded.proxy_bypass",
        rusqlite::params![
            s.proxy.enabled as i64,
            s.proxy.url,
            s.proxy.bypass
        ],
    )?;
    Ok(())
}
