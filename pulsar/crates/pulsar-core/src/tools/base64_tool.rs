//! Base64 文本编解码工具（`encoders.base64`）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use std::sync::OnceLock;

const MODE: &str = "mode";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    MODE,
    "模式",
    "encode",
    &["encode", "decode"],
)];

// base64 字符集、长度 >= 8、可带尾随 =；置信度偏低（易与其它重叠）。
static DETECTORS: &[Detector] = &[Detector::new(Rule::Regex(r"^[A-Za-z0-9+/]{8,}={0,2}$"), 40)];

pub struct Base64Tool;

impl Tool for Base64Tool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.base64".to_string(),
            category: Category::Encoders,
            name: "Base64",
            description: "在文本与其 Base64 表示之间编解码。",
            keywords: &["base64", "encode", "decode", "编码", "解码"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let mode = params
            .get(MODE)
            .and_then(|v| v.as_str())
            .unwrap_or("encode");
        let text = input.as_text();

        match mode {
            "encode" => Ok(ToolValue::text(STANDARD.encode(text.as_bytes()))),
            "decode" => {
                let bytes = STANDARD
                    .decode(text.trim().as_bytes())
                    .map_err(|e| ToolError::InvalidInput(format!("非法 Base64: {e}")))?;
                let decoded = String::from_utf8(bytes)
                    .map_err(|e| ToolError::InvalidInput(format!("解码结果非 UTF-8 文本: {e}")))?;
                Ok(ToolValue::text(decoded))
            }
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知模式 '{other}'，应为 encode 或 decode"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(mode: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        p
    }

    #[test]
    fn encodes_text() {
        let out = Base64Tool
            .run(ToolValue::text("hello"), &params("encode"))
            .unwrap();
        assert_eq!(out.as_text(), "aGVsbG8=");
    }

    #[test]
    fn decodes_text() {
        let out = Base64Tool
            .run(ToolValue::text("aGVsbG8="), &params("decode"))
            .unwrap();
        assert_eq!(out.as_text(), "hello");
    }

    #[test]
    fn rejects_invalid_base64() {
        let err = Base64Tool
            .run(ToolValue::text("not!base64"), &params("decode"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[test]
    fn round_trips() {
        let original = "Pulsar ⭐ 脉冲星";
        let encoded = Base64Tool
            .run(ToolValue::text(original), &params("encode"))
            .unwrap();
        let decoded = Base64Tool.run(encoded, &params("decode")).unwrap();
        assert_eq!(decoded.as_text(), original);
    }
}
