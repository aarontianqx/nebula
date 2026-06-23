//! Hex ↔ 文本工具（`encoders.hex`）。
//!
//! 文本 → 十六进制字节串，或十六进制 → UTF-8 文本。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const MODE: &str = "mode";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    MODE,
    "模式",
    "encode",
    &["encode", "decode"],
)];

// 偶数位的十六进制串（至少 8 位）；置信度偏低（与 base64 数字部分重叠）。
static DETECTORS: &[Detector] = &[Detector::new(Rule::Regex(r"^([0-9a-fA-F]{2}){4,}$"), 35)];

pub struct HexTool;

impl Tool for HexTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.hex".to_string(),
            category: Category::Encoders,
            name: "Hex 编解码",
            description: "在文本与其十六进制字节表示之间转换。",
            keywords: &["hex", "hexadecimal", "bytes", "十六进制"],
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
            "encode" => Ok(ToolValue::text(hex::encode(text.as_bytes()))),
            "decode" => {
                // 容忍空格与换行。
                let cleaned: String = text.chars().filter(|c| !c.is_whitespace()).collect();
                let bytes = hex::decode(&cleaned)
                    .map_err(|e| ToolError::InvalidInput(format!("非法 Hex: {e}")))?;
                let decoded = String::from_utf8(bytes)
                    .map_err(|e| ToolError::InvalidInput(format!("解码结果非 UTF-8: {e}")))?;
                Ok(ToolValue::text(decoded))
            }
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知模式 '{other}'"),
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
    fn encodes() {
        let out = HexTool
            .run(ToolValue::text("hi"), &params("encode"))
            .unwrap();
        assert_eq!(out.as_text(), "6869");
    }

    #[test]
    fn decodes_with_whitespace() {
        let out = HexTool
            .run(ToolValue::text("68 69"), &params("decode"))
            .unwrap();
        assert_eq!(out.as_text(), "hi");
    }

    #[test]
    fn rejects_invalid_hex() {
        let err = HexTool
            .run(ToolValue::text("zz"), &params("decode"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
