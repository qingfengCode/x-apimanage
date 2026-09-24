/// 初始化 schema
pub const MIGRATION_001: &str = r#"
CREATE TABLE IF NOT EXISTS collections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id TEXT,
    kind TEXT NOT NULL DEFAULT 'collection',
    description TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER,
    FOREIGN KEY (parent_id) REFERENCES collections(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS requests (
    id TEXT PRIMARY KEY,
    collection_id TEXT NOT NULL,
    name TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'GET',
    url TEXT,
    params TEXT,
    headers TEXT,
    body TEXT,
    auth TEXT,
    sort_order INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER,
    FOREIGN KEY (collection_id) REFERENCES collections(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS environments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    variables TEXT NOT NULL DEFAULT '[]',
    is_active INTEGER DEFAULT 0,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE TABLE IF NOT EXISTS history (
    id TEXT PRIMARY KEY,
    method TEXT,
    url TEXT,
    status INTEGER,
    time_ms INTEGER,
    size INTEGER,
    created_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_requests_collection ON requests(collection_id);
CREATE INDEX IF NOT EXISTS idx_collections_parent ON collections(parent_id);
CREATE INDEX IF NOT EXISTS idx_history_created ON history(created_at DESC);
"#;

/// v2 增量迁移：请求脚本 + Mock 路由表 + 历史快照
/// 注意：SQLite 不支持 ADD COLUMN IF NOT EXISTS，这里用pragma 防重复执行。
/// 我们在 Db::run_migrations 里用 PRAGMA user_version 记录已应用的版本。
pub const MIGRATION_002: &str = r#"
ALTER TABLE requests ADD COLUMN pre_script TEXT;
ALTER TABLE requests ADD COLUMN test_script TEXT;

CREATE TABLE IF NOT EXISTS mock_routes (
    id TEXT PRIMARY KEY,
    method TEXT NOT NULL DEFAULT 'GET',
    path TEXT NOT NULL,
    status INTEGER NOT NULL DEFAULT 200,
    response_headers TEXT NOT NULL DEFAULT '[]',
    response_body TEXT NOT NULL DEFAULT '',
    delay_ms INTEGER DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_mock_path ON mock_routes(method, path);

ALTER TABLE history ADD COLUMN request_snapshot TEXT;
"#;



/// v3 增量迁移：文档表（AI 生成的 API 文档/产品文档）+ AI/MCP 设置表。
/// 语句均幂等（IF NOT EXISTS / OR IGNORE），随启动执行。
pub const MIGRATION_003: &str = r#"
CREATE TABLE IF NOT EXISTS documents (
    id TEXT PRIMARY KEY,
    doc_type TEXT NOT NULL DEFAULT 'api',
    title TEXT NOT NULL,
    content TEXT NOT NULL DEFAULT '',
    request_id TEXT,
    created_at INTEGER,
    updated_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_documents_type ON documents(doc_type);

CREATE TABLE IF NOT EXISTS ai_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    base_url TEXT NOT NULL DEFAULT '',
    api_key TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    system_prompt TEXT NOT NULL DEFAULT '',
    mcp_enabled INTEGER NOT NULL DEFAULT 0,
    mcp_port INTEGER NOT NULL DEFAULT 8765
);

INSERT OR IGNORE INTO ai_settings (id) VALUES (1);
"#;

/// v4 增量迁移：AI 多会话（每个会话一行，items 为 UI 条目 JSON，history 为模型上下文 JSON）
pub const MIGRATION_004: &str = r#"
CREATE TABLE IF NOT EXISTS ai_sessions (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL DEFAULT '新会话',
    items TEXT NOT NULL DEFAULT '[]',
    history TEXT NOT NULL DEFAULT '[]',
    created_at INTEGER,
    updated_at INTEGER
);

CREATE INDEX IF NOT EXISTS idx_ai_sessions_updated ON ai_sessions(updated_at);
"#;

/// v5 增量迁移：移除历史版本写入的示例/演示数据（模拟数据）。
/// 不再往新库写入任何种子；这里只清理旧库中“仍与出厂内容完全一致”的演示行，
/// 用户改动过（改名/改 URL/在 Demo Collection 里加了真实请求等）的行视为真实数据保留。
pub const MIGRATION_005: &str = r#"
DELETE FROM requests
WHERE id = 'req-demo-get'
  AND name = 'Get IP' AND method = 'GET' AND url = '{{baseUrl}}/ip'
  AND params = '[]' AND headers = '[]' AND body IS NULL;

DELETE FROM requests
WHERE id = 'req-demo-post'
  AND name = 'Post JSON' AND method = 'POST' AND url = '{{baseUrl}}/post'
  AND params = '[]'
  AND headers = '[{"key":"Content-Type","value":"application/json","enabled":true}]'
  AND body = '{"mode":"raw","raw":"{\n  \"hello\": \"world\"\n}","mimeType":"application/json"}';

DELETE FROM collections
WHERE id = 'col-demo'
  AND name = 'Demo Collection' AND kind = 'collection'
  AND parent_id IS NULL AND description = '示例集合'
  AND NOT EXISTS (SELECT 1 FROM requests WHERE collection_id = 'col-demo')
  AND NOT EXISTS (SELECT 1 FROM collections WHERE parent_id = 'col-demo');

DELETE FROM environments
WHERE id = 'env-default'
  AND name = 'My Workspace'
  AND variables = '[{"key":"baseUrl","value":"https://httpbin.org","enabled":true}]';
"#;

/// v6 增量迁移：请求级超时配置（毫秒；NULL = 使用引擎默认 30s）。
/// 由 user_version 保证只跑一次（ALTER TABLE 无 IF NOT EXISTS）。
pub const MIGRATION_006: &str = r#"
ALTER TABLE requests ADD COLUMN timeout_ms INTEGER;
"#;

/// v8 增量迁移：历史记录的响应快照（从历史恢复时直接查看当时的响应体）。
/// 由 user_version 保证只跑一次（ALTER TABLE 无 IF NOT EXISTS）。
pub const MIGRATION_008: &str = r#"
ALTER TABLE history ADD COLUMN response_snapshot TEXT;
"#;

/// v7 增量迁移：MCP 访问密钥（设置密钥后 MCP 服务要求 Bearer 鉴权并开放局域网）
/// 防御性重建表定义：极旧/异常库可能缺 ai_settings（正常 v3+ 库已存在，IF NOT EXISTS 无副作用）
pub const MIGRATION_007: &str = r#"
CREATE TABLE IF NOT EXISTS ai_settings (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    base_url TEXT NOT NULL DEFAULT '',
    api_key TEXT NOT NULL DEFAULT '',
    model TEXT NOT NULL DEFAULT '',
    system_prompt TEXT NOT NULL DEFAULT '',
    mcp_enabled INTEGER NOT NULL DEFAULT 0,
    mcp_port INTEGER NOT NULL DEFAULT 8765
);

INSERT OR IGNORE INTO ai_settings (id) VALUES (1);

ALTER TABLE ai_settings ADD COLUMN mcp_token TEXT NOT NULL DEFAULT '';
"#;
