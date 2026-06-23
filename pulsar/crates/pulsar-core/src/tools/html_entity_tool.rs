//! HTML 实体转义 / 反转义（`encoders.html_entity`）。
//!
//! - `encode`：把 `& < > " '` 转成命名实体，安全地嵌入 HTML。
//! - `decode`：把命名实体与数字实体（`&#NN;` / `&#xHH;`）还原为字符。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
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

pub struct HtmlEntityTool;

impl Tool for HtmlEntityTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.html_entity".to_string(),
            category: Category::Encoders,
            name: "HTML 实体",
            description: "在字符与 HTML 实体之间转义/反转义（含数字实体）。",
            keywords: &["html", "entity", "实体", "转义", "escape"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let mode = params
            .get(MODE)
            .and_then(|v| v.as_str())
            .unwrap_or("encode");
        let text = input.as_text();
        let out = match mode {
            "encode" => encode(&text),
            "decode" => decode(&text),
            other => {
                return Err(ToolError::InvalidParam {
                    name: MODE.to_string(),
                    reason: format!("未知模式 '{other}'"),
                })
            }
        };
        Ok(ToolValue::text(out))
    }
}

fn encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

fn decode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            if let Some(semi) = input[i + 1..].find(';') {
                let entity = &input[i + 1..i + 1 + semi];
                if let Some(ch) = decode_entity(entity) {
                    out.push(ch);
                    i += semi + 2; // 跳过 `&...;`
                    continue;
                }
            }
        }
        // 非实体：原样推进一个 UTF-8 字符。
        let ch = input[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// 解析单个实体体（不含 `&` 与 `;`）。支持命名与数字实体。
fn decode_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        "#39" => Some('\''),
        "nbsp" => Some('\u{00A0}'),
        _ => {
            let code = entity.strip_prefix('#')?;
            let n = if let Some(hex) = code.strip_prefix(['x', 'X']) {
                u32::from_str_radix(hex, 16).ok()?
            } else {
                code.parse::<u32>().ok()?
            };
            char::from_u32(n)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, mode: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        HtmlEntityTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn encodes_special_chars() {
        assert_eq!(
            run(r#"<a href="x">&'"#, "encode"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
    }

    #[test]
    fn decodes_named_entities() {
        assert_eq!(run("&lt;a&gt;&amp;", "decode"), "<a>&");
    }

    #[test]
    fn decodes_numeric_entities() {
        assert_eq!(run("&#65;&#x42;", "decode"), "AB");
    }

    #[test]
    fn roundtrip() {
        let original = r#"if (a < b && c > "d") {}"#;
        let encoded = run(original, "encode");
        assert_eq!(run(&encoded, "decode"), original);
    }

    #[test]
    fn leaves_plain_text_untouched() {
        assert_eq!(run("hello world", "encode"), "hello world");
        assert_eq!(run("a & b not entity", "decode"), "a & b not entity");
    }
}
