//! 大小写 / 命名转换（`text.case`）。
//!
//! 把输入按词拆分（识别 camelCase / snake_case / kebab-case / 空格分隔），
//! 再以目标风格重组：camelCase / PascalCase / snake_case / kebab-case /
//! CONSTANT_CASE / "Title Case"。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const TARGET: &str = "target";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    TARGET,
    "目标风格",
    "camel",
    &["camel", "pascal", "snake", "kebab", "constant", "title"],
)];

pub struct CaseTool;

impl Tool for CaseTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "text.case".to_string(),
            category: Category::Text,
            name: "大小写/命名转换",
            description:
                "在 camelCase / PascalCase / snake_case / kebab-case / CONSTANT / Title 之间转换。",
            keywords: &[
                "case",
                "camel",
                "snake",
                "kebab",
                "pascal",
                "命名",
                "大小写",
            ],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let target = params
            .get(TARGET)
            .and_then(|v| v.as_str())
            .unwrap_or("camel");
        let words = split_words(&input.as_text());

        let out = match target {
            "camel" => to_camel(&words, false),
            "pascal" => to_camel(&words, true),
            "snake" => join_lower(&words, "_"),
            "kebab" => join_lower(&words, "-"),
            "constant" => words
                .iter()
                .map(|w| w.to_uppercase())
                .collect::<Vec<_>>()
                .join("_"),
            "title" => words
                .iter()
                .map(|w| capitalize(w))
                .collect::<Vec<_>>()
                .join(" "),
            other => {
                return Err(ToolError::InvalidParam {
                    name: TARGET.to_string(),
                    reason: format!("未知风格 '{other}'"),
                })
            }
        };
        Ok(ToolValue::text(out))
    }
}

/// 拆词：在分隔符、大小写边界、字母↔数字边界处切分；输出小写词。
fn split_words(input: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut cur = String::new();

    let flush = |cur: &mut String, words: &mut Vec<String>| {
        if !cur.is_empty() {
            words.push(std::mem::take(cur).to_lowercase());
        }
    };

    let chars: Vec<char> = input.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '_' || c == '-' || c.is_whitespace() {
            flush(&mut cur, &mut words);
            continue;
        }
        if i > 0 {
            let prev = chars[i - 1];
            let boundary = (c.is_uppercase() && prev.is_lowercase())
                || (c.is_uppercase()
                    && prev.is_uppercase()
                    && chars.get(i + 1).is_some_and(|n| n.is_lowercase()))
                || (c.is_ascii_digit() != prev.is_ascii_digit());
            if boundary {
                flush(&mut cur, &mut words);
            }
        }
        cur.push(c);
    }
    flush(&mut cur, &mut words);
    words
}

fn to_camel(words: &[String], pascal: bool) -> String {
    words
        .iter()
        .enumerate()
        .map(|(i, w)| {
            if i == 0 && !pascal {
                w.clone()
            } else {
                capitalize(w)
            }
        })
        .collect()
}

fn join_lower(words: &[String], sep: &str) -> String {
    words.join(sep).to_lowercase().replace(' ', sep)
}

fn capitalize(w: &str) -> String {
    let mut chars = w.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn convert(input: &str, target: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(TARGET.to_string(), ParamValue::Str(target.to_string()));
        CaseTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn from_snake_to_others() {
        assert_eq!(convert("hello_world_foo", "camel"), "helloWorldFoo");
        assert_eq!(convert("hello_world_foo", "pascal"), "HelloWorldFoo");
        assert_eq!(convert("hello_world_foo", "kebab"), "hello-world-foo");
        assert_eq!(convert("hello_world_foo", "constant"), "HELLO_WORLD_FOO");
        assert_eq!(convert("hello_world_foo", "title"), "Hello World Foo");
    }

    #[test]
    fn from_camel_to_snake() {
        assert_eq!(convert("helloWorldFoo", "snake"), "hello_world_foo");
    }

    #[test]
    fn from_pascal_to_kebab() {
        assert_eq!(convert("HelloWorld", "kebab"), "hello-world");
    }

    #[test]
    fn handles_digits_and_spaces() {
        assert_eq!(convert("user ID 2", "snake"), "user_id_2");
    }
}
