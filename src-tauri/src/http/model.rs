use serde::{Deserialize, Serialize};

/// 前端发起的 HTTP 请求结构（与 TS 类型对应）
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    /// [{ key, value, enabled }]
    #[serde(default)]
    pub params: Vec<KeyValue>,
    /// [{ key, value, enabled }]
    #[serde(default)]
    pub headers: Vec<KeyValue>,
    #[serde(default)]
    pub body: Option<RequestBody>,
    /// 超时（毫秒），默认 30s
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    /// 是否跟随重定向，默认 true
    #[serde(default)]
    pub follow_redirects: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "lowercase")]
pub enum RequestBody {
    /// 原始文本 { mode: "raw", raw: "...", mimeType: "application/json" }
    Raw {
        raw: String,
        /// 容器级 rename_all 只作用于变体名，字段需单独指定 camelCase
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    /// 表单 { mode: "form", items: [...] }
    Form { items: Vec<KeyValue> },
    /// multipart 表单 { mode: "multipart", items: [...] }
    Multipart { items: Vec<KeyValue> },
}

/// 返回给前端的 HTTP 响应
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: ResponseBody,
    pub time_ms: u64,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ResponseBody {
    /// 文本响应 { kind: "text", text: "...", mime: "application/json" }
    Text { text: String, mime: String },
    /// 二进制响应 { kind: "binary", base64: "...", mime: "image/png" }
    Binary { base64: String, mime: String },
}
