//! JSON 格式化 / 压缩 / 校验工具（`formatters.json`）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const MINIFY: &str = "minify";
const INDENT: &str = "indent";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::boolean(MINIFY, "压缩（单行）", false),
    ParamSpec::int(INDENT, "缩进空格数", "2"),
];

static DETECTORS: &[Detector] = &[Detector::new(Rule::JsonParse, 80)];

pub struct JsonFormatTool;

impl Tool for JsonFormatTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "formatters.json".to_string(),
            category: Category::Formatters,
            name: "JSON Formatter",
            description: "格式化、压缩并校验 JSON。非法 JSON 会报错。",
            keywords: &["json", "format", "prettify", "minify", "格式化", "压缩"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let value: serde_json::Value = serde_json::from_str(text.trim())
            .map_err(|e| ToolError::InvalidInput(format!("非法 JSON: {e}")))?;

        let minify = params
            .get(MINIFY)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if minify {
            let out = serde_json::to_string(&value)
                .map_err(|e| ToolError::InvalidInput(e.to_string()))?;
            return Ok(ToolValue::text(out));
        }

        let indent = params
            .get(INDENT)
            .and_then(|v| v.as_int())
            .unwrap_or(2)
            .clamp(0, 8) as usize;

        let pretty = pretty_with_indent(&value, indent)?;
        Ok(ToolValue::text(pretty))
    }
}

/// 以指定空格数缩进美化 JSON。
fn pretty_with_indent(value: &serde_json::Value, indent: usize) -> Result<String, ToolError> {
    let indent_bytes = vec![b' '; indent];
    let formatter = serde_json::ser::PrettyFormatter::with_indent(&indent_bytes);
    let mut buf = Vec::new();
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, formatter);
    use serde::Serialize as _;
    value
        .serialize(&mut ser)
        .map_err(|e| ToolError::InvalidInput(e.to_string()))?;
    String::from_utf8(buf).map_err(|e| ToolError::InvalidInput(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    #[test]
    fn pretty_formats_with_default_indent() {
        let out = JsonFormatTool
            .run(ToolValue::text(r#"{"a":1,"b":[2,3]}"#), &ToolParams::new())
            .unwrap();
        assert_eq!(
            out.as_text(),
            "{\n  \"a\": 1,\n  \"b\": [\n    2,\n    3\n  ]\n}"
        );
    }

    #[test]
    fn minifies() {
        let mut p = ToolParams::new();
        p.insert(MINIFY.to_string(), ParamValue::Bool(true));
        let out = JsonFormatTool
            .run(ToolValue::text("{\n  \"a\": 1\n}"), &p)
            .unwrap();
        assert_eq!(out.as_text(), r#"{"a":1}"#);
    }

    #[test]
    fn respects_custom_indent() {
        let mut p = ToolParams::new();
        p.insert(INDENT.to_string(), ParamValue::Int(4));
        let out = JsonFormatTool
            .run(ToolValue::text(r#"{"a":1}"#), &p)
            .unwrap();
        assert_eq!(out.as_text(), "{\n    \"a\": 1\n}");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = JsonFormatTool
            .run(ToolValue::text("{not json}"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
