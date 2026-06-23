//! 文本 Diff（`testers.diff`）。
//!
//! 输入两段文本，以单独一行 `=====`（至少 3 个等号）分隔，输出按行的差异。
//! 选择这种分隔方式是因为 scaffold 阶段参数仅支持单行；多输入面板待后续 UI 增强。

use crate::descriptor::{Category, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use similar::{ChangeTag, TextDiff};
use std::sync::OnceLock;

pub struct DiffTool;

impl Tool for DiffTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "testers.diff".to_string(),
            category: Category::Testers,
            name: "文本 Diff",
            description: "比较两段文本（用单独一行 ===== 分隔），按行显示差异。",
            keywords: &["diff", "compare", "difference", "对比", "差异"],
            params: &[],
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, _params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let (left, right) = split_inputs(&text).ok_or_else(|| {
            ToolError::InvalidInput("请用单独一行 `=====` 分隔需要比较的两段文本".into())
        })?;

        let diff = TextDiff::from_lines(left, right);
        let mut out = String::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            out.push_str(sign);
            out.push(' ');
            out.push_str(change.value());
            if !change.value().ends_with('\n') {
                out.push('\n');
            }
        }
        if out.is_empty() {
            out.push_str("（两段文本相同）");
        }
        Ok(ToolValue::text(out))
    }
}

/// 在第一处仅由 `=` 组成（>=3 个）的行处切分。
fn split_inputs(text: &str) -> Option<(&str, &str)> {
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.len() >= 3 && trimmed.chars().all(|c| c == '=') {
            let left = &text[..offset];
            let right = &text[offset + line.len()..];
            return Some((left, right));
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shows_line_changes() {
        let input = "line1\nline2\nline3\n=====\nline1\nline2 changed\nline3\n";
        let out = DiffTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("- line2\n"));
        assert!(text.contains("+ line2 changed\n"));
        assert!(text.contains("  line1\n"));
    }

    #[test]
    fn identical_inputs() {
        let input = "same\n=====\nsame\n";
        let out = DiffTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap();
        assert!(out.as_text().contains("  same"));
    }

    #[test]
    fn missing_separator_errors() {
        let err = DiffTool
            .run(ToolValue::text("no separator here"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
