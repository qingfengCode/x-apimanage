pub mod repos;
pub mod schema;

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::error::{AppError, AppResult};

/// 跨线程共享的数据库句柄（Tauri State 与 MCP server 共用）
pub type SharedDb = Arc<Mutex<Db>>;

/// 当前 schema 最高版本（新增迁移时同步更新；测试断言引用此常量避免过期）
pub const LATEST_VERSION: i64 = 8;

/// 数据库句柄
pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn open(path: &Path) -> AppResult<Self> {
        let conn = Connection::open(path)?;
        // 启用外键级联
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        Ok(Db { conn })
    }

    pub fn run_migrations(&self) -> AppResult<()> {
        // 基础 schema（幂等）
        self.conn.execute_batch(schema::MIGRATION_001)?;

        // 基于版本号的增量迁移
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap_or(0);

        if version < 2 {
            // ALTER TABLE 无 IF NOT EXISTS；用 user_version 保证只跑一次
            self.apply_migration_002()?;
            self.conn.execute_batch("PRAGMA user_version = 2")?;
        }

        if version < 3 {
            // v3：文档表 + AI/MCP 设置（语句幂等，仍登记版本号）
            self.conn.execute_batch(schema::MIGRATION_003)?;
            self.conn.execute_batch("PRAGMA user_version = 3")?;
        }

        if version < 4 {
            // v4：AI 多会话（语句幂等）
            self.conn.execute_batch(schema::MIGRATION_004)?;
            self.conn.execute_batch("PRAGMA user_version = 4")?;
        }

        if version < 5 {
            // v5：清理旧库遗留的出厂演示/示例数据（仅删除未改动过的原始行）
            self.conn.execute_batch(schema::MIGRATION_005)?;
            self.conn.execute_batch("PRAGMA user_version = 5")?;
        }

        if version < 6 {
            // v6：请求级超时配置（毫秒）
            self.conn.execute_batch(schema::MIGRATION_006)?;
            self.conn.execute_batch("PRAGMA user_version = 6")?;
        }

        if version < 7 {
            // v7：MCP 访问密钥列（ALTER 无 IF NOT EXISTS，重复执行容错）
            for stmt in schema::MIGRATION_007.split(';') {
                let trimmed = stmt.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Err(e) = self.conn.execute(trimmed, []) {
                    if !e.to_string().contains("duplicate column") {
                        return Err(AppError::Db(e));
                    }
                }
            }
            self.conn.execute_batch("PRAGMA user_version = 7")?;
        }

        if version < 8 {
            // v8：历史响应快照列（user_version 保证只跑一次；ALTER 容错与 v7 同款）
            for stmt in schema::MIGRATION_008.split(';') {
                let trimmed = stmt.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Err(e) = self.conn.execute(trimmed, []) {
                    if !e.to_string().contains("duplicate column") {
                        return Err(AppError::Db(e));
                    }
                }
            }
            self.conn.execute_batch("PRAGMA user_version = 8")?;
        }

        Ok(())
    }

    fn apply_migration_002(&self) -> AppResult<()> {
        // 逐条执行，对已存在的列/表做容错（幂等）
        for stmt in schema::MIGRATION_002.split(';') {
            let trimmed = stmt.trim();
            if trimmed.is_empty() {
                continue;
            }
            // ALTER TABLE 已存在列会报错，吞掉 "duplicate column" 错误
            if let Err(e) = self.conn.execute(trimmed, []) {
                let msg = e.to_string();
                let benign = msg.contains("duplicate column")
                    || msg.contains("already exists");
                if !benign {
                    return Err(AppError::Db(e));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// 与历史 SEED 写入内容完全一致的演示行
    fn seed_demo_rows(conn: &Connection) {
        conn.execute_batch(
            r#"
INSERT INTO environments (id, name, variables, is_active, created_at, updated_at) VALUES
  ('env-default', 'My Workspace',
   '[{"key":"baseUrl","value":"https://httpbin.org","enabled":true}]', 1, 0, 0),
  ('env-real', 'Prod',
   '[{"key":"host","value":"api.real.com","enabled":true}]', 0, 0, 0);

INSERT INTO collections (id, name, parent_id, kind, description, sort_order, created_at, updated_at) VALUES
  ('col-demo', 'Demo Collection', NULL, 'collection', '示例集合', 0, 0, 0);

INSERT INTO requests (id, collection_id, name, method, url, params, headers, body, auth, sort_order, created_at, updated_at) VALUES
  ('req-demo-get', 'col-demo', 'Get IP', 'GET', '{{baseUrl}}/ip', '[]', '[]', NULL, NULL, 0, 0, 0),
  ('req-demo-post', 'col-demo', 'Post JSON', 'POST', '{{baseUrl}}/post', '[]',
   '[{"key":"Content-Type","value":"application/json","enabled":true}]',
   '{"mode":"raw","raw":"{\n  \"hello\": \"world\"\n}","mimeType":"application/json"}',
   NULL, 1, 0, 0);
"#,
        )
        .unwrap();
    }

    fn count(conn: &Connection, sql: &str) -> i64 {
        conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    /// 原样演示数据：全部清理；用户自己的数据不受影响
    #[test]
    fn migration_005_removes_pristine_demo_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::MIGRATION_001).unwrap();
        seed_demo_rows(&conn);
        conn.execute_batch(schema::MIGRATION_005).unwrap();

        assert_eq!(count(&conn, "SELECT COUNT(*) FROM requests"), 0);
        assert_eq!(count(&conn, "SELECT COUNT(*) FROM collections"), 0);
        // env-default 被清，env-real 保留
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM environments"),
            1,
            "用户环境应保留"
        );
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM environments WHERE id = 'env-real'"),
            1
        );
    }

    /// 用户改动过的演示行 / Demo 集合里新增的真实请求：全部保留
    #[test]
    fn migration_005_keeps_user_modified_rows() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(schema::MIGRATION_001).unwrap();
        seed_demo_rows(&conn);
        // 用户改过 req-demo-get（改名 + 换 URL）
        conn.execute_batch(
            r#"
UPDATE requests SET name = '我的接口', url = 'https://api.real.com/ip' WHERE id = 'req-demo-get';
INSERT INTO requests (id, collection_id, name, method, url, params, headers, body, auth, sort_order, created_at, updated_at)
VALUES ('req-real', 'col-demo', '真实请求', 'GET', 'https://api.real.com/x', '[]', '[]', NULL, NULL, 2, 0, 0);
"#,
        )
        .unwrap();
        conn.execute_batch(schema::MIGRATION_005).unwrap();

        // 原样的 req-demo-post 被清，改动过的 req-demo-get 与真实请求保留
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM requests WHERE id = 'req-demo-get'"),
            1,
            "改动过的演示请求应保留"
        );
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM requests WHERE id = 'req-real'"),
            1
        );
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM requests WHERE id = 'req-demo-post'"),
            0
        );
        // 集合里还有真实请求 → col-demo 保留
        assert_eq!(
            count(&conn, "SELECT COUNT(*) FROM collections WHERE id = 'col-demo'"),
            1,
            "含真实请求的演示集合应保留"
        );
    }

    fn temp_db_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("xapimanage-test-{name}-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("test.db")
    }

    /// 全新空库：run_migrations 后只有空表，不写入任何演示/模拟数据
    #[test]
    fn run_migrations_on_fresh_db_creates_no_demo_data() {
        let path = temp_db_path("fresh");
        let db = Db::open(&path).unwrap();
        db.run_migrations().unwrap();
        let conn = &db.conn;

        assert_eq!(count(conn, "SELECT COUNT(*) FROM environments"), 0);
        assert_eq!(count(conn, "SELECT COUNT(*) FROM collections"), 0);
        assert_eq!(count(conn, "SELECT COUNT(*) FROM requests"), 0);
        // 迁移到最新版本后 requests 表含 timeout_ms 列
        let timeout_col: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('requests') WHERE name = 'timeout_ms'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(timeout_col, 1);
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, LATEST_VERSION);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// 模拟 v4 旧库升级：出厂演示行被清理、用户行保留、version 升到最新
    #[test]
    fn run_migrations_upgrades_v4_db_and_cleans_demo_rows() {
        let path = temp_db_path("v4-upgrade");
        {
            // 建出真实 v4 形态的旧库：v1-v4 迁移全部应用 + 旧 SEED 演示数据 + 用户数据
            let raw = Connection::open(&path).unwrap();
            raw.execute_batch(schema::MIGRATION_001).unwrap();
            raw.execute_batch(schema::MIGRATION_002).unwrap();
            raw.execute_batch(schema::MIGRATION_003).unwrap();
            raw.execute_batch(schema::MIGRATION_004).unwrap();
            seed_demo_rows(&raw);
            raw.execute_batch(
                "INSERT INTO requests (id, collection_id, name, method, url, params, headers, body, auth, sort_order, created_at, updated_at)
                 VALUES ('req-user', 'col-demo', '用户请求', 'GET', 'https://real.com/', '[]', '[]', NULL, NULL, 5, 0, 0)",
            )
            .unwrap();
            raw.execute_batch("PRAGMA user_version = 4").unwrap();
            drop(raw);
        }

        let db = Db::open(&path).unwrap();
        db.run_migrations().unwrap();
        let conn = &db.conn;

        // 演示集合因含用户请求而保留，但两个原样演示请求被清
        assert_eq!(
            count(conn, "SELECT COUNT(*) FROM requests WHERE id = 'req-demo-get'"),
            0
        );
        assert_eq!(
            count(conn, "SELECT COUNT(*) FROM requests WHERE id = 'req-user'"),
            1
        );
        assert_eq!(
            count(conn, "SELECT COUNT(*) FROM collections WHERE id = 'col-demo'"),
            1
        );
        // env-default 被清，env-real 保留
        assert_eq!(
            count(conn, "SELECT COUNT(*) FROM environments WHERE id = 'env-default'"),
            0
        );
        assert_eq!(
            count(conn, "SELECT COUNT(*) FROM environments WHERE id = 'env-real'"),
            1
        );
        let version: i64 = conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, LATEST_VERSION);
        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }

    /// 请求级超时的 upsert/读取回环（含显式清空为 NULL）
    #[test]
    fn request_upsert_roundtrips_timeout_ms() {
        use crate::db::repos::request::{self, RequestInput};

        let path = temp_db_path("timeout");
        let db = Db::open(&path).unwrap();
        db.run_migrations().unwrap();
        let conn = &db.conn;
        conn.execute(
            "INSERT INTO collections (id, name, parent_id, kind, created_at, updated_at)
             VALUES ('c1', 'C', NULL, 'collection', 0, 0)",
            [],
        )
        .unwrap();

        let input = RequestInput {
            id: Some("r1".into()),
            collection_id: "c1".into(),
            name: "X".into(),
            method: "GET".into(),
            url: Some("http://x".into()),
            params: None,
            headers: None,
            body: None,
            auth: None,
            pre_script: None,
            test_script: None,
            sort_order: Some(1),
            timeout_ms: Some(5000),
        };
        request::upsert(conn, &input).unwrap();
        assert_eq!(request::get(conn, "r1").unwrap().timeout_ms, Some(5000));

        // 再次保存把超时清空（如旧请求恢复默认）时，应落 NULL 而非残留旧值
        let mut reset = input;
        reset.timeout_ms = None;
        request::upsert(conn, &reset).unwrap();
        assert_eq!(request::get(conn, "r1").unwrap().timeout_ms, None);

        std::fs::remove_dir_all(path.parent().unwrap()).ok();
    }
}
