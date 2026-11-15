use std::fmt;

use serde::Serialize;

/// Tauri 命令统一返回的稳定错误结构。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// 稳定的机器可识别错误码。
    pub code: String,
    /// 面向用户的中文错误说明。
    pub message: String,
    /// 可选 Git、网络或模型技术细节。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Rust 后端操作的共享结果类型。
pub type AppResult<T> = Result<T, AppError>;

impl AppError {
    /// 创建带稳定错误码和可选技术细节的错误。
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// 追加技术上下文，同时保持公开错误码不变。
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        let details = details.into();
        if !details.trim().is_empty() {
            self.details = Some(details);
        }
        self
    }

    /// 将文件系统错误转换为带操作上下文的稳定错误。
    pub fn from_io(
        code: impl Into<String>,
        message: impl Into<String>,
        error: &std::io::Error,
    ) -> Self {
        Self::new(code, message).with_details(error.to_string())
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}
