//! Unicode 转义 / 反转义（`encoders.unicode`）。
//!
//! - `encode`：把非 ASCII 字符转成 `\uXXXX`（BMP 外用代理对），ASCII 原样保留。
//! - `decode`：把 `\uXXXX` 与 `\u{XXXXX}` 还原为字符，正确合并代理对。

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

pub struct UnicodeTool;

impl Tool for UnicodeTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "encoders.unicode".to_string(),
            category: Category::Encoders,
            name: "Unicode 转义",
            description: "在字符与 \\uXXXX 转义序列之间互转（支持 BMP 外字符）。",
            keywords: &["unicode", "转义", "escape", "\\u", "码点"],
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
            "decode" => decode(&text)?,
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
    let mut out = String::new();
    for c in input.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            let cp = c as u32;
            if cp <= 0xFFFF {
                out.push_str(&format!("\\u{cp:04x}"));
            } else {
                // BMP 外：拆成 UTF-16 代理对。
                let v = cp - 0x10000;
                let high = 0xD800 + (v >> 10);
                let low = 0xDC00 + (v & 0x3FF);
                out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
            }
        }
    }
    out
}

fn decode(input: &str) -> Result<String, ToolError> {
    // 先收集所有 \uXXXX 为 u16 单元（穿插字面字符），再按 UTF-16 解码以合并代理对。
    enum Unit {
        Char(char),
        U16(u16),
    }
    let mut units: Vec<Unit> = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() && (bytes[i + 1] == b'u') {
            // 形式一：\u{XXXXX}
            if i + 2 < bytes.len() && bytes[i + 2] == b'{' {
                if let Some(close) = input[i + 3..].find('}') {
                    let hex = &input[i + 3..i + 3 + close];
                    let n = u32::from_str_radix(hex, 16)
                        .map_err(|_| ToolError::InvalidInput(format!("无效码点: \\u{{{hex}}}")))?;
                    let ch = char::from_u32(n)
                        .ok_or_else(|| ToolError::InvalidInput(format!("非法码点: {n:#x}")))?;
                    units.push(Unit::Char(ch));
                    i += 3 + close + 1;
                    continue;
                }
            }
            // 形式二：\uXXXX（4 位十六进制）
            if i + 6 <= bytes.len() {
                let hex = &input[i + 2..i + 6];
                if let Ok(n) = u16::from_str_radix(hex, 16) {
                    units.push(Unit::U16(n));
                    i += 6;
                    continue;
                }
            }
        }
        let ch = input[i..].chars().next().unwrap();
        units.push(Unit::Char(ch));
        i += ch.len_utf8();
    }

    // 把连续的 U16 段按 UTF-16 解码，字面字符直接拼接。
    let mut out = String::new();
    let mut buf: Vec<u16> = Vec::new();
    let flush = |buf: &mut Vec<u16>, out: &mut String| -> Result<(), ToolError> {
        if !buf.is_empty() {
            let decoded = String::from_utf16(buf)
                .map_err(|e| ToolError::InvalidInput(format!("UTF-16 解码失败: {e}")))?;
            out.push_str(&decoded);
            buf.clear();
        }
        Ok(())
    };
    for unit in units {
        match unit {
            Unit::U16(u) => buf.push(u),
            Unit::Char(c) => {
                flush(&mut buf, &mut out)?;
                out.push(c);
            }
        }
    }
    flush(&mut buf, &mut out)?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, mode: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        UnicodeTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn encodes_non_ascii() {
        assert_eq!(run("中A", "encode"), "\\u4e2dA");
    }

    #[test]
    fn decodes_bmp() {
        assert_eq!(run("\\u4e2dA", "decode"), "中A");
    }

    #[test]
    fn roundtrip_emoji_with_surrogate_pair() {
        // U+1F600 在 BMP 外，需走代理对。
        let encoded = run("😀", "encode");
        assert_eq!(encoded, "\\ud83d\\ude00");
        assert_eq!(run(&encoded, "decode"), "😀");
    }

    #[test]
    fn decodes_braced_form() {
        assert_eq!(run("\\u{1f600}", "decode"), "😀");
    }

    #[test]
    fn ascii_passthrough() {
        assert_eq!(run("hello", "encode"), "hello");
    }
}
