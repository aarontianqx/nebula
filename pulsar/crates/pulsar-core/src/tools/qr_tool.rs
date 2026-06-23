//! 二维码生成（`generators.qr`）。
//!
//! 把输入文本编码为二维码，输出 SVG（可嵌入网页/矢量打印）或 ASCII 预览（终端可读）。
//! 纯文本输出，不引入位图依赖。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use qrcode::render::svg;
use qrcode::QrCode;
use std::sync::OnceLock;

const FORMAT: &str = "format";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    FORMAT,
    "输出",
    "svg",
    &["svg", "ascii"],
)];

pub struct QrTool;

impl Tool for QrTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.qr".to_string(),
            category: Category::Generators,
            name: "二维码生成",
            description: "把文本/URL 编码为二维码，输出 SVG 或 ASCII 预览。",
            keywords: &["qr", "qrcode", "二维码", "svg"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let text = input.as_text();
        let data = text.trim_end_matches('\n');
        if data.is_empty() {
            return Err(ToolError::InvalidInput("请输入要编码的内容".to_string()));
        }
        let format = params.get(FORMAT).and_then(|v| v.as_str()).unwrap_or("svg");

        let code = QrCode::new(data.as_bytes())
            .map_err(|e| ToolError::InvalidInput(format!("二维码编码失败: {e}")))?;

        let out = match format {
            "svg" => code
                .render::<svg::Color>()
                .min_dimensions(200, 200)
                .dark_color(svg::Color("#000000"))
                .light_color(svg::Color("#ffffff"))
                .build(),
            "ascii" => code
                .render::<char>()
                .quiet_zone(true)
                .module_dimensions(2, 1)
                .dark_color('█')
                .light_color(' ')
                .build(),
            other => {
                return Err(ToolError::InvalidParam {
                    name: FORMAT.to_string(),
                    reason: format!("未知输出 '{other}'"),
                })
            }
        };
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, format: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(FORMAT.to_string(), ParamValue::Str(format.to_string()));
        QrTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn svg_output_is_svg() {
        let out = run("https://example.com", "svg");
        assert!(out.contains("<svg"));
        assert!(out.contains("</svg>"));
    }

    #[test]
    fn ascii_output_has_modules() {
        let out = run("hello", "ascii");
        assert!(out.contains('█'));
        assert!(out.lines().count() > 5);
    }

    #[test]
    fn empty_input_errors() {
        let err = QrTool
            .run(ToolValue::text(""), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
