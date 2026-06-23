//! Slug 生成（`text.slug`）。
//!
//! 把任意标题转成 URL 友好的 slug：小写、ASCII 字母数字保留，其余转连字符并合并，
//! 去除首尾连字符。非 ASCII 字符（如中文）按分隔处理。可选保留大小写、自定义分隔符。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::ToolResult;
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const SEPARATOR: &str = "separator";
const LOWERCASE: &str = "lowercase";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::enumerated(SEPARATOR, "分隔符", "-", &["-", "_"]),
    ParamSpec::boolean(LOWERCASE, "转小写", true),
];

pub struct SlugTool;

impl Tool for SlugTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "text.slug".to_string(),
            category: Category::Text,
            name: "Slug 生成",
            description: "把标题转成 URL 友好的 slug（小写、连字符分隔）。",
            keywords: &["slug", "url", "permalink", "短链", "别名"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let sep = params
            .get(SEPARATOR)
            .and_then(|v| v.as_str())
            .and_then(|s| s.chars().next())
            .unwrap_or('-');
        let lowercase = params
            .get(LOWERCASE)
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let text = input.as_text();
        let mut out = String::with_capacity(text.len());
        let mut prev_sep = true; // 置 true 以吃掉前导分隔符
        for c in text.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(if lowercase { c.to_ascii_lowercase() } else { c });
                prev_sep = false;
            } else if !prev_sep {
                // 任何非字母数字（空格、标点、非 ASCII）都折叠为单个分隔符。
                out.push(sep);
                prev_sep = true;
            }
        }
        // 去掉末尾分隔符。
        while out.ends_with(sep) {
            out.pop();
        }
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn slug(input: &str) -> String {
        SlugTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap()
            .as_text()
            .into_owned()
    }

    fn slug_with(input: &str, sep: &str, lowercase: bool) -> String {
        let mut p = ToolParams::new();
        p.insert(SEPARATOR.to_string(), ParamValue::Str(sep.to_string()));
        p.insert(LOWERCASE.to_string(), ParamValue::Bool(lowercase));
        SlugTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn basic_title_to_slug() {
        assert_eq!(slug("Hello World"), "hello-world");
    }

    #[test]
    fn collapses_punctuation_and_spaces() {
        assert_eq!(slug("  Foo --- Bar!!  Baz  "), "foo-bar-baz");
    }

    #[test]
    fn custom_separator_and_case() {
        assert_eq!(slug_with("Hello World", "_", false), "Hello_World");
    }

    #[test]
    fn non_ascii_acts_as_separator() {
        // 中文之间的英文段被分隔；纯标点/中文折叠成分隔符并被裁掉。
        assert_eq!(slug("a 中文 b"), "a-b");
    }

    #[test]
    fn leading_trailing_separators_trimmed() {
        assert_eq!(slug("--hello--"), "hello");
    }
}
