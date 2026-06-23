//! 正则测试器（`testers.regex`）。
//!
//! 用给定 pattern 在输入文本中查找匹配，列出每个匹配及其捕获分组。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use regex::Regex;
use std::sync::OnceLock;

const PATTERN: &str = "pattern";
const IGNORE_CASE: &str = "ignore_case";
const MULTILINE: &str = "multiline";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::string(PATTERN, "正则表达式", ""),
    ParamSpec::boolean(IGNORE_CASE, "忽略大小写 (i)", false),
    ParamSpec::boolean(MULTILINE, "多行 (m)", false),
];

pub struct RegexTool;

impl Tool for RegexTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "testers.regex".to_string(),
            category: Category::Testers,
            name: "正则测试器",
            description: "用正则在输入文本中查找匹配，列出匹配项与捕获分组。",
            keywords: &["regex", "regexp", "正则", "match", "pattern"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let pattern = params
            .get(PATTERN)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if pattern.is_empty() {
            return Err(ToolError::InvalidParam {
                name: PATTERN.to_string(),
                reason: "请提供正则表达式".into(),
            });
        }

        let ignore_case = params
            .get(IGNORE_CASE)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let multiline = params
            .get(MULTILINE)
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let re = regex::RegexBuilder::new(&pattern)
            .case_insensitive(ignore_case)
            .multi_line(multiline)
            .build()
            .map_err(|e| ToolError::InvalidParam {
                name: PATTERN.to_string(),
                reason: format!("非法正则: {e}"),
            })?;

        let text = input.as_text();
        Ok(ToolValue::text(format_matches(&re, &text)))
    }
}

fn format_matches(re: &Regex, text: &str) -> String {
    let mut lines = Vec::new();
    let mut count = 0;

    for caps in re.captures_iter(text) {
        count += 1;
        let whole = caps.get(0).map(|m| m.as_str()).unwrap_or("");
        lines.push(format!("#{count}: {whole:?}"));
        for i in 1..caps.len() {
            if let Some(g) = caps.get(i) {
                lines.push(format!("    group {i}: {:?}", g.as_str()));
            } else {
                lines.push(format!("    group {i}: <none>"));
            }
        }
    }

    if count == 0 {
        return "无匹配".to_string();
    }
    format!("共 {count} 处匹配\n\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(pattern: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(PATTERN.to_string(), ParamValue::Str(pattern.to_string()));
        p
    }

    #[test]
    fn finds_matches_with_groups() {
        let out = RegexTool
            .run(ToolValue::text("a1 b2 c3"), &params(r"([a-z])(\d)"))
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("共 3 处匹配"));
        assert!(text.contains("group 1: \"a\""));
        assert!(text.contains("group 2: \"1\""));
    }

    #[test]
    fn reports_no_match() {
        let out = RegexTool
            .run(ToolValue::text("hello"), &params(r"\d+"))
            .unwrap();
        assert_eq!(out.as_text(), "无匹配");
    }

    #[test]
    fn rejects_empty_pattern() {
        let err = RegexTool
            .run(ToolValue::text("x"), &params(""))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }

    #[test]
    fn rejects_invalid_pattern() {
        let err = RegexTool
            .run(ToolValue::text("x"), &params("("))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }
}
