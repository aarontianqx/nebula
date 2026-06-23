//! URL 编解码工具（`encoders.url`）。
//!
//! 对文本做 percent-encoding 编码或解码。

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

// 含 %XX 转义序列，或是 http(s) URL。
static DETECTORS: &[Detector] = &[
    Detector::new(Rule::Regex(r"%[0-9a-fA-F]{2}"), 55),
    Detector::new(Rule::Regex(r"^\s*https?://"), 45),
];

pub struct UrlTool;

impl Tool for UrlTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.url".to_string(),
            category: Category::Encoders,
            name: "URL 编解码",
            description: "对文本进行 URL（percent-encoding）编码或解码。",
            keywords: &["url", "percent", "encode", "decode", "编码", "解码"],
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
            "encode" => Ok(ToolValue::text(urlencoding::encode(&text).into_owned())),
            "decode" => {
                let decoded = urlencoding::decode(&text)
                    .map_err(|e| ToolError::InvalidInput(format!("非法 URL 编码: {e}")))?;
                Ok(ToolValue::text(decoded.into_owned()))
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
        let out = UrlTool
            .run(ToolValue::text("a b&c=d"), &params("encode"))
            .unwrap();
        assert_eq!(out.as_text(), "a%20b%26c%3Dd");
    }

    #[test]
    fn decodes() {
        let out = UrlTool
            .run(ToolValue::text("a%20b%26c"), &params("decode"))
            .unwrap();
        assert_eq!(out.as_text(), "a b&c");
    }

    #[test]
    fn round_trips_unicode() {
        let original = "搜索 query?";
        let enc = UrlTool
            .run(ToolValue::text(original), &params("encode"))
            .unwrap();
        let dec = UrlTool.run(enc, &params("decode")).unwrap();
        assert_eq!(dec.as_text(), original);
    }
}
