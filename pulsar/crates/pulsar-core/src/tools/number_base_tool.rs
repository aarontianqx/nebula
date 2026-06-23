//! 进制转换工具（`converters.number_base`）。
//!
//! 输入一个整数（支持 0x / 0o / 0b 前缀或显式来源进制），输出 2/8/10/16 进制表示。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const FROM: &str = "from";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    FROM,
    "来源进制",
    "auto",
    &["auto", "2", "8", "10", "16"],
)];

// 0x / 0o / 0b 前缀数字几乎一定是要做进制转换。
static DETECTORS: &[Detector] = &[Detector::new(
    Rule::Regex(r"^\s*0[xXoObB][0-9a-fA-F]+\s*$"),
    70,
)];

pub struct NumberBaseTool;

impl Tool for NumberBaseTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.number_base".to_string(),
            category: Category::Converters,
            name: "进制转换",
            description: "在 2/8/10/16 进制之间转换整数（auto 识别 0x/0o/0b 前缀）。",
            keywords: &[
                "base",
                "hex",
                "binary",
                "octal",
                "进制",
                "二进制",
                "十六进制",
            ],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let from = params.get(FROM).and_then(|v| v.as_str()).unwrap_or("auto");
        let text = input.as_text();
        let trimmed = text.trim();

        let value = parse_input(trimmed, from)?;

        let out = format!(
            "decimal (10): {value}\nhex (16):     0x{value:X}\noctal (8):    0o{value:o}\nbinary (2):   0b{value:b}",
        );
        Ok(ToolValue::text(out))
    }
}

fn parse_input(input: &str, from: &str) -> Result<i128, ToolError> {
    let radix = match from {
        "auto" => {
            return parse_auto(input);
        }
        "2" => 2,
        "8" => 8,
        "10" => 10,
        "16" => 16,
        other => {
            return Err(ToolError::InvalidParam {
                name: FROM.to_string(),
                reason: format!("不支持的进制 '{other}'"),
            })
        }
    };
    i128::from_str_radix(input, radix)
        .map_err(|_| ToolError::InvalidInput(format!("'{input}' 不是合法的 {from} 进制整数")))
}

fn parse_auto(input: &str) -> Result<i128, ToolError> {
    let (body, radix) = if let Some(rest) = input
        .strip_prefix("0x")
        .or_else(|| input.strip_prefix("0X"))
    {
        (rest, 16)
    } else if let Some(rest) = input
        .strip_prefix("0o")
        .or_else(|| input.strip_prefix("0O"))
    {
        (rest, 8)
    } else if let Some(rest) = input
        .strip_prefix("0b")
        .or_else(|| input.strip_prefix("0B"))
    {
        (rest, 2)
    } else {
        (input, 10)
    };
    i128::from_str_radix(body, radix)
        .map_err(|_| ToolError::InvalidInput(format!("无法识别的整数: {input}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(from: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(FROM.to_string(), ParamValue::Str(from.to_string()));
        p
    }

    #[test]
    fn auto_detects_hex() {
        let out = NumberBaseTool
            .run(ToolValue::text("0xFF"), &params("auto"))
            .unwrap();
        assert!(out.as_text().contains("decimal (10): 255"));
        assert!(out.as_text().contains("binary (2):   0b11111111"));
    }

    #[test]
    fn decimal_default() {
        let out = NumberBaseTool
            .run(ToolValue::text("16"), &params("auto"))
            .unwrap();
        assert!(out.as_text().contains("hex (16):     0x10"));
    }

    #[test]
    fn explicit_binary_source() {
        let out = NumberBaseTool
            .run(ToolValue::text("1010"), &params("2"))
            .unwrap();
        assert!(out.as_text().contains("decimal (10): 10"));
    }

    #[test]
    fn rejects_invalid() {
        let err = NumberBaseTool
            .run(ToolValue::text("xyz"), &params("auto"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
