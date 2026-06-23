//! XML 格式化（`formatters.xml`）。
//!
//! 用 `quick-xml` 重新解析并以缩进写出，把压缩或杂乱的 XML 整理成可读形式。
//! 缩进宽度可选 2 / 4 空格。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use std::sync::OnceLock;

const INDENT: &str = "indent";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(INDENT, "缩进", "2", &["2", "4"])];

pub struct XmlFormatTool;

impl Tool for XmlFormatTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "formatters.xml".to_string(),
            category: Category::Formatters,
            name: "XML 格式化",
            description: "重新缩进 XML，把压缩或杂乱的标签整理成可读结构。",
            keywords: &["xml", "format", "格式化", "美化", "indent"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let indent: usize = match params.get(INDENT).and_then(|v| v.as_str()).unwrap_or("2") {
            "4" => 4,
            _ => 2,
        };
        let text = input.as_text();
        let src = text.trim();
        if src.is_empty() {
            return Ok(ToolValue::text(String::new()));
        }

        let mut reader = Reader::from_str(src);
        reader.config_mut().trim_text(true);
        let mut writer = Writer::new_with_indent(Vec::new(), b' ', indent);

        loop {
            match reader.read_event() {
                Ok(Event::Eof) => break,
                Ok(event) => writer
                    .write_event(event)
                    .map_err(|e| ToolError::InvalidInput(format!("写出 XML 失败: {e}")))?,
                Err(e) => {
                    return Err(ToolError::InvalidInput(format!(
                        "XML 解析失败 (位置 {}): {e}",
                        reader.error_position()
                    )))
                }
            }
        }

        let bytes = writer.into_inner();
        String::from_utf8(bytes)
            .map(|s| ToolValue::text(s.trim_end().to_string()))
            .map_err(|e| ToolError::InvalidInput(format!("XML 非 UTF-8: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format(input: &str) -> String {
        XmlFormatTool
            .run(ToolValue::text(input), &ToolParams::new())
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn indents_nested_elements() {
        let out = format("<root><child>text</child></root>");
        assert!(out.lines().count() >= 3);
        assert!(out.contains("<root>"));
        assert!(out.contains("  <child>text</child>"));
    }

    #[test]
    fn preserves_attributes() {
        let out = format(r#"<a x="1"><b y="2"/></a>"#);
        assert!(out.contains(r#"<a x="1">"#));
        assert!(out.contains(r#"y="2""#));
    }

    #[test]
    fn empty_input_is_ok() {
        assert_eq!(format(""), "");
    }

    #[test]
    fn invalid_xml_errors() {
        let err = XmlFormatTool
            .run(ToolValue::text("<a><b></a>"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
