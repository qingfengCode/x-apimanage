use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AppError {
    #[error("数据库错误: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("序列化错误: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("HTTP 请求错误: {0}")]
    Http(String),
    #[error("无效的 URL: {0}")]
    InvalidUrl(String),
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),
    #[error("Base64 解码错误: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("应用更新错误: {0}")]
    Update(String),
    /// 用户主动终止在途请求（非错误，前端按取消态展示）
    #[error("请求已终止")]
    Cancelled,
    #[error("{0}")]
    Other(String),
}

impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self {
        AppError::Http(e.to_string())
    }
}

// 把 AppError 转成字符串返回给前端（Tauri 命令的 Err 类型需可序列化）
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
