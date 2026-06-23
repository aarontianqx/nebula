//! JWT 解析工具（`encoders.jwt`）。
//!
//! 解码 JWT 的 header 与 payload（base64url），美化输出。
//! 注意：当前不做签名验签（验签留待后续安全工具阶段，见 specs）。

use crate::descriptor::{Category, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use std::sync::OnceLock;

static DETECTORS: &[Detector] = &[Detector::new(Rule::JwtShape, 95)];

pub struct JwtTool;

impl Tool for JwtTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.jwt".to_string(),
            category: Category::Encoders,
            name: "JWT 解析",
            description: "解码 JWT 的 header 与 payload（不验签）。",
            keywords: &["jwt", "token", "json web token", "decode"],
            params: &[],
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, _params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let token = text.trim();

        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() < 2 {
            return Err(ToolError::InvalidInput(
                "不是合法的 JWT（应至少包含 header.payload）".into(),
            ));
        }

        let header = decode_segment(parts[0], "header")?;
        let payload = decode_segment(parts[1], "payload")?;

        let out = format!("--- Header ---\n{header}\n\n--- Payload ---\n{payload}");
        Ok(ToolValue::text(out))
    }
}

fn decode_segment(segment: &str, which: &str) -> Result<String, ToolError> {
    let bytes = URL_SAFE_NO_PAD
        .decode(segment)
        .map_err(|e| ToolError::InvalidInput(format!("{which} 段 base64url 解码失败: {e}")))?;
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|e| ToolError::InvalidInput(format!("{which} 段不是合法 JSON: {e}")))?;
    serde_json::to_string_pretty(&value).map_err(|e| ToolError::InvalidInput(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // {"alg":"HS256","typ":"JWT"}.{"sub":"123","name":"Pulsar"}.<sig>
    const SAMPLE: &str =
        "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMiLCJuYW1lIjoiUHVsc2FyIn0.abc";

    #[test]
    fn decodes_header_and_payload() {
        let out = JwtTool
            .run(ToolValue::text(SAMPLE), &ToolParams::new())
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("\"alg\": \"HS256\""));
        assert!(text.contains("\"name\": \"Pulsar\""));
    }

    #[test]
    fn rejects_non_jwt() {
        let err = JwtTool
            .run(ToolValue::text("not-a-jwt"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
