//! 强密码生成 + 熵分析（`generators.password`）。
//!
//! 按所选字符集（小写/大写/数字/符号）与长度生成随机密码（基于 `nanoid` 的 OS 随机源），
//! 并给出香农熵估算（`length × log2(charset)`）与强度评级。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const LENGTH: &str = "length";
const LOWER: &str = "lowercase";
const UPPER: &str = "uppercase";
const DIGITS: &str = "digits";
const SYMBOLS: &str = "symbols";

const LOWER_SET: &str = "abcdefghijklmnopqrstuvwxyz";
const UPPER_SET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGIT_SET: &str = "0123456789";
const SYMBOL_SET: &str = "!@#$%^&*()-_=+[]{};:,.<>?";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::int(LENGTH, "长度", "16"),
    ParamSpec::boolean(LOWER, "小写字母", true),
    ParamSpec::boolean(UPPER, "大写字母", true),
    ParamSpec::boolean(DIGITS, "数字", true),
    ParamSpec::boolean(SYMBOLS, "符号", true),
];

pub struct PasswordTool;

impl Tool for PasswordTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.password".to_string(),
            category: Category::Generators,
            name: "密码生成",
            description: "按字符集与长度生成随机密码，并给出熵与强度评级。",
            keywords: &["password", "密码", "随机", "熵", "entropy", "generate"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, _input: ToolValue, params: &ToolParams) -> ToolResult {
        let length = params
            .get(LENGTH)
            .and_then(|v| v.as_int())
            .unwrap_or(16)
            .clamp(4, 256) as usize;

        let flag =
            |k: &str, default: bool| params.get(k).and_then(|v| v.as_bool()).unwrap_or(default);
        let mut alphabet: Vec<char> = Vec::new();
        if flag(LOWER, true) {
            alphabet.extend(LOWER_SET.chars());
        }
        if flag(UPPER, true) {
            alphabet.extend(UPPER_SET.chars());
        }
        if flag(DIGITS, true) {
            alphabet.extend(DIGIT_SET.chars());
        }
        if flag(SYMBOLS, true) {
            alphabet.extend(SYMBOL_SET.chars());
        }
        if alphabet.is_empty() {
            return Err(ToolError::InvalidParam {
                name: "charset".to_string(),
                reason: "至少要启用一种字符集".to_string(),
            });
        }

        let password = nanoid::nanoid!(length, &alphabet);
        let entropy = length as f64 * (alphabet.len() as f64).log2();
        let out = format!(
            "{password}\n\n长度: {length}\n字符集大小: {}\n熵: {entropy:.1} bits ({})",
            alphabet.len(),
            strength_label(entropy),
        );
        Ok(ToolValue::text(out))
    }
}

/// 依据香农熵的常见经验阈值给出强度标签。
fn strength_label(entropy: f64) -> &'static str {
    if entropy < 40.0 {
        "弱"
    } else if entropy < 60.0 {
        "中等"
    } else if entropy < 80.0 {
        "强"
    } else {
        "很强"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn gen(length: i64, lower: bool, upper: bool, digits: bool, symbols: bool) -> String {
        let mut p = ToolParams::new();
        p.insert(LENGTH.to_string(), ParamValue::Int(length));
        p.insert(LOWER.to_string(), ParamValue::Bool(lower));
        p.insert(UPPER.to_string(), ParamValue::Bool(upper));
        p.insert(DIGITS.to_string(), ParamValue::Bool(digits));
        p.insert(SYMBOLS.to_string(), ParamValue::Bool(symbols));
        PasswordTool
            .run(ToolValue::text(""), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn generates_requested_length() {
        let out = gen(20, true, true, true, true);
        let pw = out.lines().next().unwrap();
        assert_eq!(pw.chars().count(), 20);
    }

    #[test]
    fn respects_charset_only_digits() {
        let out = gen(32, false, false, true, false);
        let pw = out.lines().next().unwrap();
        assert!(pw.chars().all(|c| c.is_ascii_digit()));
    }

    #[test]
    fn reports_entropy_and_strength() {
        let out = gen(16, true, true, true, true);
        assert!(out.contains("熵:"));
        assert!(out.contains("bits"));
    }

    #[test]
    fn empty_charset_errors() {
        let mut p = ToolParams::new();
        p.insert(LOWER.to_string(), ParamValue::Bool(false));
        p.insert(UPPER.to_string(), ParamValue::Bool(false));
        p.insert(DIGITS.to_string(), ParamValue::Bool(false));
        p.insert(SYMBOLS.to_string(), ParamValue::Bool(false));
        let err = PasswordTool.run(ToolValue::text(""), &p).unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }
}
