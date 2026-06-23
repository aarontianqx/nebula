//! JSON ↔ YAML 互转工具（`converters.json_yaml`）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const DIRECTION: &str = "direction";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    DIRECTION,
    "方向",
    "json_to_yaml",
    &["json_to_yaml", "yaml_to_json"],
)];

static DETECTORS: &[Detector] = &[Detector::new(Rule::JsonParse, 55)];

pub struct JsonYamlTool;

impl Tool for JsonYamlTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.json_yaml".to_string(),
            category: Category::Converters,
            name: "JSON ↔ YAML",
            description: "在 JSON 与 YAML 之间互相转换。",
            keywords: &["json", "yaml", "convert", "转换"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let direction = params
            .get(DIRECTION)
            .and_then(|v| v.as_str())
            .unwrap_or("json_to_yaml");
        let text = input.as_text();
        let trimmed = text.trim();

        match direction {
            "json_to_yaml" => {
                let value: serde_json::Value = serde_json::from_str(trimmed)
                    .map_err(|e| ToolError::InvalidInput(format!("非法 JSON: {e}")))?;
                let yaml = serde_yaml::to_string(&value)
                    .map_err(|e| ToolError::InvalidInput(format!("YAML 序列化失败: {e}")))?;
                Ok(ToolValue::text(yaml))
            }
            "yaml_to_json" => {
                let value: serde_json::Value = serde_yaml::from_str(trimmed)
                    .map_err(|e| ToolError::InvalidInput(format!("非法 YAML: {e}")))?;
                let json = serde_json::to_string_pretty(&value)
                    .map_err(|e| ToolError::InvalidInput(format!("JSON 序列化失败: {e}")))?;
                Ok(ToolValue::text(json))
            }
            other => Err(ToolError::InvalidParam {
                name: DIRECTION.to_string(),
                reason: format!("未知方向 '{other}'"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(dir: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(DIRECTION.to_string(), ParamValue::Str(dir.to_string()));
        p
    }

    #[test]
    fn json_to_yaml() {
        let out = JsonYamlTool
            .run(
                ToolValue::text(r#"{"a":1,"b":["x","y"]}"#),
                &params("json_to_yaml"),
            )
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("a: 1"));
        assert!(text.contains("- x"));
    }

    #[test]
    fn yaml_to_json() {
        let out = JsonYamlTool
            .run(ToolValue::text("a: 1\nb: 2\n"), &params("yaml_to_json"))
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("\"a\": 1"));
        assert!(text.contains("\"b\": 2"));
    }

    #[test]
    fn round_trips() {
        let original = r#"{"name":"pulsar","tags":["dev","tool"]}"#;
        let yaml = JsonYamlTool
            .run(ToolValue::text(original), &params("json_to_yaml"))
            .unwrap();
        let json = JsonYamlTool.run(yaml, &params("yaml_to_json")).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json.as_text()).unwrap();
        assert_eq!(v["name"], "pulsar");
        assert_eq!(v["tags"][1], "tool");
    }

    #[test]
    fn rejects_invalid_json() {
        let err = JsonYamlTool
            .run(ToolValue::text("{bad}"), &params("json_to_yaml"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
