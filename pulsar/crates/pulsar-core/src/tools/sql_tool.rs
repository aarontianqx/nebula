//! SQL 格式化（`formatters.sql`）。
//!
//! 基于 `sqlformat` crate 重排空白与缩进，可选把保留关键字转大写。
//! 适合把压缩成一行的查询整理成可读形式。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::ToolResult;
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use sqlformat::{FormatOptions, Indent, QueryParams};
use std::sync::OnceLock;

const INDENT: &str = "indent";
const UPPERCASE: &str = "uppercase";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::enumerated(INDENT, "缩进", "2", &["2", "4", "tab"]),
    ParamSpec::boolean(UPPERCASE, "关键字大写", true),
];

pub struct SqlTool;

impl Tool for SqlTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "formatters.sql".to_string(),
            category: Category::Formatters,
            name: "SQL 格式化",
            description: "重排 SQL 缩进与空白，可选关键字大写，提升可读性。",
            keywords: &["sql", "format", "格式化", "美化", "query"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let indent = match params.get(INDENT).and_then(|v| v.as_str()).unwrap_or("2") {
            "4" => Indent::Spaces(4),
            "tab" => Indent::Tabs,
            _ => Indent::Spaces(2),
        };
        let uppercase = params
            .get(UPPERCASE)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let options = FormatOptions {
            indent,
            uppercase: Some(uppercase),
            ..Default::default()
        };
        let out = sqlformat::format(&input.as_text(), &QueryParams::None, &options);
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn format_sql(input: &str, uppercase: bool) -> String {
        let mut p = ToolParams::new();
        p.insert(UPPERCASE.to_string(), ParamValue::Bool(uppercase));
        SqlTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn breaks_oneliner_into_multiple_lines() {
        let out = format_sql("select id, name from users where age > 18", true);
        assert!(out.lines().count() > 1);
        assert!(out.contains("SELECT"));
        assert!(out.contains("FROM"));
        assert!(out.contains("WHERE"));
    }

    #[test]
    fn lowercase_keywords_when_disabled() {
        let out = format_sql("SELECT a FROM t", false);
        assert!(out.contains("select"));
    }

    #[test]
    fn empty_input_is_ok() {
        let out = SqlTool
            .run(ToolValue::text(""), &ToolParams::new())
            .unwrap();
        assert_eq!(out.as_text().trim(), "");
    }
}
