//! 行处理：去重 / 排序 / 去空白（`text.dedup_sort`）。
//!
//! 按行对文本做组合处理。操作顺序固定为：trim → 去空行 → 去重 → 排序，
//! 各步均可通过参数开关，便于把杂乱列表整理成干净集合。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::ToolResult;
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::collections::HashSet;
use std::sync::OnceLock;

const TRIM: &str = "trim";
const REMOVE_BLANK: &str = "remove_blank";
const DEDUP: &str = "dedup";
const SORT: &str = "sort";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::boolean(TRIM, "去首尾空白", true),
    ParamSpec::boolean(REMOVE_BLANK, "去空行", true),
    ParamSpec::boolean(DEDUP, "去重", true),
    ParamSpec::enumerated(SORT, "排序", "none", &["none", "asc", "desc"]),
];

pub struct DedupSortTool;

impl Tool for DedupSortTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "text.dedup_sort".to_string(),
            category: Category::Text,
            name: "去重/排序/去空白",
            description: "按行 trim、去空行、去重、排序，整理杂乱列表。",
            keywords: &["dedup", "去重", "sort", "排序", "unique", "trim", "行"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let flag =
            |k: &str, default: bool| params.get(k).and_then(|v| v.as_bool()).unwrap_or(default);
        let trim = flag(TRIM, true);
        let remove_blank = flag(REMOVE_BLANK, true);
        let dedup = flag(DEDUP, true);
        let sort = params.get(SORT).and_then(|v| v.as_str()).unwrap_or("none");

        let text = input.as_text();
        let mut lines: Vec<String> = text
            .lines()
            .map(|l| {
                if trim {
                    l.trim().to_string()
                } else {
                    l.to_string()
                }
            })
            .collect();

        if remove_blank {
            lines.retain(|l| !l.is_empty());
        }
        if dedup {
            let mut seen = HashSet::new();
            lines.retain(|l| seen.insert(l.clone()));
        }
        match sort {
            "asc" => lines.sort(),
            "desc" => {
                lines.sort();
                lines.reverse();
            }
            _ => {}
        }

        Ok(ToolValue::text(lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, trim: bool, remove_blank: bool, dedup: bool, sort: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(TRIM.to_string(), ParamValue::Bool(trim));
        p.insert(REMOVE_BLANK.to_string(), ParamValue::Bool(remove_blank));
        p.insert(DEDUP.to_string(), ParamValue::Bool(dedup));
        p.insert(SORT.to_string(), ParamValue::Str(sort.to_string()));
        DedupSortTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn dedups_preserving_first_seen_order() {
        assert_eq!(run("b\na\nb\nc\na", false, false, true, "none"), "b\na\nc");
    }

    #[test]
    fn sorts_ascending_and_descending() {
        assert_eq!(run("c\na\nb", false, false, false, "asc"), "a\nb\nc");
        assert_eq!(run("a\nc\nb", false, false, false, "desc"), "c\nb\na");
    }

    #[test]
    fn trims_and_removes_blanks() {
        assert_eq!(run("  a  \n\n  b  \n", true, true, false, "none"), "a\nb");
    }

    #[test]
    fn combined_pipeline() {
        // trim + 去空行 + 去重 + 升序。
        assert_eq!(run("  b \na\n\n b\nc ", true, true, true, "asc"), "a\nb\nc");
    }

    #[test]
    fn all_disabled_is_identity_by_line() {
        assert_eq!(run("b\na\nb", false, false, false, "none"), "b\na\nb");
    }
}
