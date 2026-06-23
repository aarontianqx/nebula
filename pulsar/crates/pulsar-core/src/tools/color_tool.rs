//! 颜色格式转换（`converters.color`）。
//!
//! 解析任意一种常见写法（`#RGB` / `#RRGGBB` / `rgb(r,g,b)` / `rgba(...)` /
//! `hsl(h,s%,l%)`），统一输出 HEX、RGB、HSL 三种表示，便于在设计稿与代码之间互换。

use crate::descriptor::{Category, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

static DETECTORS: &[Detector] = &[
    // #RGB / #RRGGBB
    Detector::new(Rule::Regex(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$"), 60),
    // rgb(...) / rgba(...) / hsl(...)
    Detector::new(Rule::Regex(r"(?i)^(rgba?|hsl)\s*\("), 55),
];

pub struct ColorTool;

impl Tool for ColorTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.color".to_string(),
            category: Category::Converters,
            name: "颜色格式转换",
            description: "在 HEX / RGB / HSL 之间互转，自动识别输入写法。",
            keywords: &["color", "颜色", "hex", "rgb", "hsl", "配色"],
            params: &[],
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, _params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let (r, g, b) = parse_color(text.trim())?;
        let (h, s, l) = rgb_to_hsl(r, g, b);
        let out = format!(
            "HEX: #{r:02X}{g:02X}{b:02X}\nRGB: rgb({r}, {g}, {b})\nHSL: hsl({h}, {s}%, {l}%)",
        );
        Ok(ToolValue::text(out))
    }
}

/// 把任意支持的写法解析成 `(r, g, b)`（各 0–255）。
fn parse_color(input: &str) -> Result<(u8, u8, u8), ToolError> {
    let s = input.trim();
    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }
    let lower = s.to_lowercase();
    if lower.starts_with("rgb") {
        return parse_rgb(s);
    }
    if lower.starts_with("hsl") {
        return parse_hsl(s);
    }
    // 容错：无 `#` 前缀的裸 hex。
    if s.chars().all(|c| c.is_ascii_hexdigit()) && (s.len() == 3 || s.len() == 6) {
        return parse_hex(s);
    }
    Err(ToolError::InvalidInput(format!(
        "无法识别的颜色写法: '{input}'（支持 #hex / rgb() / hsl()）"
    )))
}

fn parse_hex(hex: &str) -> Result<(u8, u8, u8), ToolError> {
    let expand = |h: &str| -> Option<(u8, u8, u8)> {
        match h.len() {
            3 => {
                let bytes: Vec<u8> = h.bytes().collect();
                let dup = |b: u8| {
                    let c = (b as char).to_digit(16)? as u8;
                    Some(c * 16 + c)
                };
                Some((dup(bytes[0])?, dup(bytes[1])?, dup(bytes[2])?))
            }
            6 => {
                let r = u8::from_str_radix(&h[0..2], 16).ok()?;
                let g = u8::from_str_radix(&h[2..4], 16).ok()?;
                let b = u8::from_str_radix(&h[4..6], 16).ok()?;
                Some((r, g, b))
            }
            _ => None,
        }
    };
    expand(hex).ok_or_else(|| {
        ToolError::InvalidInput(format!("无效 HEX 颜色: '#{hex}'（需 3 或 6 位十六进制）"))
    })
}

fn parse_rgb(s: &str) -> Result<(u8, u8, u8), ToolError> {
    let nums = extract_numbers(s);
    if nums.len() < 3 {
        return Err(ToolError::InvalidInput(format!(
            "RGB 需要至少 3 个分量: '{s}'"
        )));
    }
    let clamp = |v: f64| v.round().clamp(0.0, 255.0) as u8;
    Ok((clamp(nums[0]), clamp(nums[1]), clamp(nums[2])))
}

fn parse_hsl(s: &str) -> Result<(u8, u8, u8), ToolError> {
    let nums = extract_numbers(s);
    if nums.len() < 3 {
        return Err(ToolError::InvalidInput(format!("HSL 需要 3 个分量: '{s}'")));
    }
    let h = nums[0].rem_euclid(360.0);
    let sat = (nums[1] / 100.0).clamp(0.0, 1.0);
    let light = (nums[2] / 100.0).clamp(0.0, 1.0);
    Ok(hsl_to_rgb(h, sat, light))
}

/// 从字符串里提取所有数字（含小数、负号），用于解析 `rgb(...)` / `hsl(...)`。
fn extract_numbers(s: &str) -> Vec<f64> {
    let mut nums = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' {
            cur.push(c);
        } else if !cur.is_empty() {
            if let Ok(n) = cur.parse::<f64>() {
                nums.push(n);
            }
            cur.clear();
        }
    }
    if let Ok(n) = cur.parse::<f64>() {
        nums.push(n);
    }
    nums
}

/// RGB(0–255) → HSL，返回 `(h 度, s%, l%)`，分量四舍五入到整数。
fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (u16, u8, u8) {
    let rf = r as f64 / 255.0;
    let gf = g as f64 / 255.0;
    let bf = b as f64 / 255.0;
    let max = rf.max(gf).max(bf);
    let min = rf.min(gf).min(bf);
    let delta = max - min;

    let l = (max + min) / 2.0;
    let (mut h, s) = if delta.abs() < f64::EPSILON {
        (0.0, 0.0)
    } else {
        let s = delta / (1.0 - (2.0 * l - 1.0).abs());
        let h = if (max - rf).abs() < f64::EPSILON {
            ((gf - bf) / delta).rem_euclid(6.0)
        } else if (max - gf).abs() < f64::EPSILON {
            (bf - rf) / delta + 2.0
        } else {
            (rf - gf) / delta + 4.0
        };
        (h * 60.0, s)
    };
    if h < 0.0 {
        h += 360.0;
    }
    (
        h.round() as u16 % 360,
        (s * 100.0).round() as u8,
        (l * 100.0).round() as u8,
    )
}

/// HSL(h 度, s 0–1, l 0–1) → RGB(0–255)。
fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    if s.abs() < f64::EPSILON {
        let v = (l * 255.0).round() as u8;
        return (v, v, v);
    }
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = h / 60.0;
    let x = c * (1.0 - (h_prime.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h_prime as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (to(r1), to(g1), to(b1))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn convert(input: &str) -> String {
        ColorTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn hex6_to_all() {
        let out = convert("#1E90FF");
        assert!(out.contains("HEX: #1E90FF"));
        assert!(out.contains("RGB: rgb(30, 144, 255)"));
        assert!(out.contains("HSL: hsl(210, 100%, 56%)"));
    }

    #[test]
    fn hex3_is_expanded() {
        let out = convert("#0af");
        assert!(out.contains("HEX: #00AAFF"));
    }

    #[test]
    fn rgb_roundtrips_to_hex() {
        let out = convert("rgb(255, 0, 0)");
        assert!(out.contains("HEX: #FF0000"));
        assert!(out.contains("HSL: hsl(0, 100%, 50%)"));
    }

    #[test]
    fn hsl_parses_back() {
        let out = convert("hsl(120, 100%, 50%)");
        assert!(out.contains("HEX: #00FF00"));
    }

    #[test]
    fn bare_hex_without_hash() {
        let out = convert("ffffff");
        assert!(out.contains("HEX: #FFFFFF"));
        assert!(out.contains("HSL: hsl(0, 0%, 100%)"));
    }

    #[test]
    fn invalid_color_errors() {
        let err = ColorTool
            .run(ToolValue::text("not-a-color"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
