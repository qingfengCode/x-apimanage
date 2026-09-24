// 与 Rust 后端共享的类型定义

export interface KeyValue {
  key: string;
  value: string;
  enabled: boolean;
}

export type HttpMethod = "GET" | "POST" | "PUT" | "DELETE" | "PATCH" | "HEAD" | "OPTIONS";

export const HTTP_METHODS: HttpMethod[] = [
  "GET",
  "POST",
  "PUT",
  "DELETE",
  "PATCH",
  "HEAD",
  "OPTIONS",
];

export type BodyMode = "raw" | "form" | "multipart";

// ---- 认证配置（Auth Tab，随请求持久化到 requests.auth 列） ----

export interface BearerAuth {
  type: "bearer";
  /** 完整 token（发送时拼 "Bearer <token>"，支持 {{变量}}） */
  token: string;
  /** 自定义 scheme，默认 Bearer（如 OAuth 用的 MAC、Token 等） */
  prefix: string;
}

export interface BasicAuth {
  type: "basic";
  username: string;
  password: string;
}

export interface ApiKeyAuth {
  type: "apikey";
  /** 注入位置：请求头或 query 参数 */
  addTo: "header" | "query";
  /** header 名（addTo=header 时）或参数名（addTo=query 时） */
  key: string;
  value: string;
}

export type AuthConfig = BearerAuth | BasicAuth | ApiKeyAuth;

export interface RawBody {
  mode: "raw";
  raw: string;
  mimeType: string;
}

export interface FormBody {
  mode: "form";
  items: KeyValue[];
}

export interface MultipartBody {
  mode: "multipart";
  items: KeyValue[];
}

export type RequestBody = RawBody | FormBody | MultipartBody;

/** 发送给 Rust 后端的请求结构 */
export interface HttpRequestPayload {
  method: string;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  body: RequestBody | null;
  timeoutMs?: number;
  followRedirects?: boolean;
}

/** HTTP 响应 */
export interface TextResponseBody {
  kind: "text";
  text: string;
  mime: string;
}

export interface BinaryResponseBody {
  kind: "binary";
  base64: string;
  mime: string;
}

export type ResponseBody = TextResponseBody | BinaryResponseBody;

export interface HttpResponsePayload {
  status: number;
  statusText: string;
  headers: [string, string][];
  body: ResponseBody;
  timeMs: number;
  size: number;
  url: string;
}

// ---- 数据库模型 ----

export type CollectionKind = "collection" | "folder";

export interface Collection {
  id: string;
  name: string;
  parentId: string | null;
  kind: CollectionKind;
  description: string | null;
  sortOrder: number;
  createdAt: number;
  updatedAt: number;
}

export interface RequestItem {
  id: string;
  collectionId: string;
  name: string;
  method: string;
  url: string | null;
  params: string | null; // JSON 字符串
  headers: string | null; // JSON 字符串
  body: string | null; // JSON 字符串
  auth: string | null;
  preScript: string | null;
  testScript: string | null;
  sortOrder: number;
  /** 请求级超时（毫秒），null = 默认 30s */
  timeoutMs: number | null;
  createdAt: number;
  updatedAt: number;
}

export interface Environment {
  id: string;
  name: string;
  variables: string; // JSON 字符串: KeyValue[]
  isActive: boolean;
  createdAt: number;
  updatedAt: number;
}

export interface HistoryItem {
  id: string;
  method: string | null;
  url: string | null;
  status: number | null;
  timeMs: number | null;
  size: number | null;
  createdAt: number;
  /** 完整请求快照 JSON（用于从历史恢复） */
  requestSnapshot?: string | null;
  /** 是否存有响应快照（列表查询只带存在性标记，本体在恢复时单条拉取） */
  hasResponse?: boolean;
}

/** 历史记录里存的请求快照结构 */
export interface RequestSnapshot {
  name: string;
  method: string;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  body: RequestBody | null;
  auth?: AuthConfig | null;
  preScript?: string;
  testScript?: string;
  /** 请求级超时（毫秒） */
  timeoutMs?: number;
}

/** Mock 路由 */
export interface MockRoute {
  id: string;
  method: string;
  path: string;
  status: number;
  responseHeaders: string; // JSON: [{key,value}]
  responseBody: string;
  delayMs: number;
  enabled: boolean;
  createdAt: number;
  updatedAt: number;
}

/** Mock 服务最近请求日志（未命中的请求也记录，status=404） */
export interface MockLogEntry {
  id: string;
  /** 请求到达时间（毫秒） */
  ts: number;
  method: string;
  path: string;
  status: number;
  matchedPattern: string | null;
  routeId: string | null;
  delayMs: number;
}

/** 测试脚本断言结果 */
export interface TestResult {
  name: string;
  passed: boolean;
  error?: string;
}

/** Pre/Post 脚本执行的结果汇总 */
export interface ScriptRunResult {
  /** Pre-script 执行后写入的变量（覆盖环境变量用于本次请求） */
  variables: Record<string, string>;
  /** Pre-script 执行错误（若有） */
  preError?: string;
  /** Post-script (test) 的断言结果 */
  tests: TestResult[];
  /** Post-script 执行错误（若有） */
  testError?: string;
  /** console.log 收集的输出 */
  logs: string[];
}

/** 应用内的请求草稿（Tab 里编辑的） */
export interface RequestDraft {
  id: string; // 与 RequestItem.id 对应（未保存时为临时 id）
  name: string;
  collectionId: string | null;
  method: HttpMethod;
  url: string;
  params: KeyValue[];
  headers: KeyValue[];
  body: RequestBody | null;
  /** 认证配置（null = 不使用），发送时自动注入 */
  auth: AuthConfig | null;
  preScript: string;
  testScript: string;
  /** 请求级超时（毫秒） */
  timeoutMs: number;
  dirty: boolean; // 是否有未保存改动
}

/** AI / MCP 设置（对应 Rust ai_settings 表） */
export interface AiSettings {
  baseUrl: string;
  apiKey: string;
  model: string;
  systemPrompt: string;
  mcpEnabled: boolean;
  mcpPort: number;
  /** MCP 访问密钥（非空 = 启用 Bearer 鉴权 + 局域网开放） */
  mcpToken: string;
}

// ---- 应用自更新 ----

/** 更新清单（自建服务器上的 update.json） */
export interface UpdateManifest {
  version: string;
  notes: string;
  url: string;
  sha256?: string | null;
}

/** 下载进度事件 payload */
export interface UpdateProgressEvent {
  received: number;
  total: number;
  percent: number;
}

/** 更新弹窗展示的应用信息 */
export interface UpdateInfo {
  currentVersion: string;
  manifestUrl: string;
  dataDir: string;
}

/** MCP 服务信息（mcp_server_info 命令返回） */
export interface McpServerInfo {
  running: boolean;
  url: string | null;
  port: number;
  token: string;
  lanIp: string;
}

/** AI 聊天事件（Rust Channel 推送） */
export type AiChatEvent =
  | { type: "status"; text: string }
  | { type: "delta"; text: string }
  | { type: "tool"; name: string; ok: boolean; result: string }
  | { type: "done"; content: string }
  | { type: "error"; message: string };

/** AI 会话（列表元数据） */
export interface AiSessionMeta {
  id: string;
  title: string;
  createdAt: number;
  updatedAt: number;
}

/** 聊天界面渲染用的消息条目 */
export interface AiChatItem {
  kind: "user" | "assistant" | "tool" | "status" | "error";
  text: string;
  toolName?: string;
  ok?: boolean;
  /** 流式输出中（打字机阶段，Done 后清除） */
  streaming?: boolean;
}

/** 文档（AI 生成或手写，Markdown） */
export interface DocumentItem {
  id: string;
  docType: "api" | "product" | string;
  title: string;
  content: string;
  requestId: string | null;
  createdAt: number;
  updatedAt: number;
}
