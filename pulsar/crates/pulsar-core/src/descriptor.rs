//! 工具的静态元数据。
//!
//! UI 表单、CLI 子命令、Pipeline 兼容校验、Smart Detection 候选——
//! 全部由 `ToolDescriptor` 派生。新增工具只需实现 `Tool` 并注册其描述符。

use crate::detect::Detector;
use crate::value::IoKind;
use serde::Serialize;

/// 工具分类（与 specs 的信息架构一致）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Converters,
    Encoders,
    Formatters,
    Generators,
    Testers,
    Text,
    Graphic,
    Reference,
}

impl Category {
    /// 用于 id 前缀与 UI 分组的稳定标识。
    pub fn slug(&self) -> &'static str {
        match self {
            Category::Converters => "converters",
            Category::Encoders => "encoders",
            Category::Formatters => "formatters",
            Category::Generators => "generators",
            Category::Testers => "testers",
            Category::Text => "text",
            Category::Graphic => "graphic",
            Category::Reference => "reference",
        }
    }

    /// UI 展示名。
    pub fn label(&self) -> &'static str {
        match self {
            Category::Converters => "Converters",
            Category::Encoders => "Encoders / Decoders",
            Category::Formatters => "Formatters",
            Category::Generators => "Generators",
            Category::Testers => "Testers",
            Category::Text => "Text",
            Category::Graphic => "Graphic",
            Category::Reference => "Reference",
        }
    }
}

/// 参数类型（决定 UI 控件与 CLI flag 解析方式）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    Bool,
    Int,
    /// 单行字符串。
    Str,
    /// 多选其一（取值来自 `ParamSpec::options`）。
    Enum,
}

/// 单个参数的声明。
#[derive(Debug, Clone, Serialize)]
pub struct ParamSpec {
    pub key: &'static str,
    pub label: &'static str,
    pub kind: ParamKind,
    /// 默认值（字符串形式，前端按 kind 解释）。
    pub default: &'static str,
    /// 当 kind == Enum 时的可选项。
    pub options: &'static [&'static str],
}

impl ParamSpec {
    pub const fn boolean(key: &'static str, label: &'static str, default: bool) -> Self {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Bool,
            default: if default { "true" } else { "false" },
            options: &[],
        }
    }

    pub const fn int(key: &'static str, label: &'static str, default: &'static str) -> Self {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Int,
            default,
            options: &[],
        }
    }

    pub const fn string(key: &'static str, label: &'static str, default: &'static str) -> Self {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Str,
            default,
            options: &[],
        }
    }

    pub const fn enumerated(
        key: &'static str,
        label: &'static str,
        default: &'static str,
        options: &'static [&'static str],
    ) -> Self {
        ParamSpec {
            key,
            label,
            kind: ParamKind::Enum,
            default,
            options,
        }
    }
}

/// 工具的静态描述符。
#[derive(Debug, Clone, Serialize)]
pub struct ToolDescriptor {
    /// 形如 "encoders.base64"，约定为 `<category>.<tool>`。
    pub id: String,
    pub category: Category,
    pub name: &'static str,
    pub description: &'static str,
    /// 搜索关键词（Command Palette）。
    pub keywords: &'static [&'static str],
    pub params: &'static [ParamSpec],
    pub input_kind: IoKind,
    pub output_kind: IoKind,
    /// 是否可参与 Pipeline。
    pub pipeable: bool,
    /// Smart Detection 规则（仅服务端使用，不下发前端）。
    #[serde(skip)]
    pub detectors: &'static [Detector],
}
