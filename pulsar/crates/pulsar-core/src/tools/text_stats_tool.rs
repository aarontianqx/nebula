//! 文本统计（`text.stats`）。
//!
//! 统计字符数（含/不含空白）、单词数、行数、字节数（UTF-8），用于快速了解文本规模。

use crate::descriptor::{Category, ToolDescriptor};
use crate::error::ToolResult;
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

pub struct TextStatsTool;

impl Tool for TextStatsTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "text.stats".to_string(),
            category: Category::Text,
            name: "文本统计",
            description: "统计字符（含/不含空白）、单词、行数与字节数。",
            keywords: &["count", "统计", "字数", "字符", "行数", "word"],
            params: &[],
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, _params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let chars = text.chars().count();
        let chars_no_ws = text.chars().filter(|c| !c.is_whitespace()).count();
        let words = text.split_whitespace().count();
        let bytes = text.len();
        // 行数：非空文本至少 1 行；末尾换行不额外计行。
        let lines = if text.is_empty() {
            0
        } else {
            text.lines().count().max(1)
        };

        let out = format!(
            "字符数:       {chars}\n字符数(无空白): {chars_no_ws}\n单词数:       {words}\n行数:         {lines}\n字节数(UTF-8): {bytes}",
        );
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(input: &str) -> String {
        TextStatsTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn counts_basic_text() {
        let out = stats("hello world");
        assert!(out.contains("字符数:       11"));
        assert!(out.contains("单词数:       2"));
        assert!(out.contains("行数:         1"));
    }

    #[test]
    fn counts_multibyte_chars() {
        let out = stats("中文");
        assert!(out.contains("字符数:       2"));
        assert!(out.contains("字节数(UTF-8): 6"));
    }

    #[test]
    fn counts_lines() {
        let out = stats("a\nb\nc");
        assert!(out.contains("行数:         3"));
    }

    #[test]
    fn chars_without_whitespace() {
        let out = stats("a b c");
        assert!(out.contains("字符数(无空白): 3"));
    }

    #[test]
    fn empty_text_is_all_zero() {
        let out = stats("");
        assert!(out.contains("字符数:       0"));
        assert!(out.contains("行数:         0"));
    }
}
