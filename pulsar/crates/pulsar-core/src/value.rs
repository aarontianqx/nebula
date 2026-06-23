//! 工具的输入/输出值，以及参数类型。
//!
//! `ToolValue` 统一封装工具的输入与输出，便于 Pipeline 串联与 Smart Detection。
//! 当前 scaffold 阶段聚焦文本类工具；`Bytes` 已预留给二进制/图片类工具，
//! 流式 (`Stream`) 处理见 `architecture.md`，留待大文件阶段引入。

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 工具的输入 / 输出值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data", rename_all = "snake_case")]
pub enum ToolValue {
    /// 纯文本（绝大多数工具的输入/输出）。
    Text(String),
    /// 原始字节（图片、文件、二进制编解码）。
    Bytes(Vec<u8>),
}

impl ToolValue {
    pub fn text(s: impl Into<String>) -> Self {
        ToolValue::Text(s.into())
    }

    /// 取文本视图；若是字节则按 UTF-8 lossy 解析。
    pub fn as_text(&self) -> std::borrow::Cow<'_, str> {
        match self {
            ToolValue::Text(s) => std::borrow::Cow::Borrowed(s),
            ToolValue::Bytes(b) => String::from_utf8_lossy(b),
        }
    }

    /// 值的 IO 类型，用于 Pipeline 兼容性校验。
    pub fn kind(&self) -> IoKind {
        match self {
            ToolValue::Text(_) => IoKind::Text,
            ToolValue::Bytes(_) => IoKind::Bytes,
        }
    }
}

/// 工具输入/输出的类型标记（供 Pipeline 构建期校验、UI 提示）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IoKind {
    Text,
    Bytes,
}

/// 单个参数值（UI 表单字段 / CLI flag 都映射到这里）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ParamValue {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl ParamValue {
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ParamValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ParamValue::Int(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            ParamValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// 工具参数集合：键 → 值。
pub type ToolParams = BTreeMap<String, ParamValue>;
