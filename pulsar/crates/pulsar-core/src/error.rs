//! 工具执行的类型化错误。

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ToolError {
    /// 输入内容不符合工具预期（如非法 JSON、非法 Base64）。
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// 缺少必需参数或参数类型不对。
    #[error("invalid parameter '{name}': {reason}")]
    InvalidParam { name: String, reason: String },

    /// 输入值类型与工具不匹配（如把 Bytes 喂给只接受 Text 的工具）。
    #[error("unsupported input kind for this tool")]
    UnsupportedKind,
}

pub type ToolResult = Result<crate::value::ToolValue, ToolError>;
