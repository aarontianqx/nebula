//! TOML ↔ JSON / YAML 互转（`converters.toml`）。
//!
//! 以 TOML 为中心做四向转换：`toml2json` / `json2toml` / `toml2yaml` / `yaml2toml`。
//! 经由 `serde_json::Value` 作为中间表示桥接三种格式。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const MODE: &str = "mode";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    MODE,
    "方向",
    "toml2json",
    &["toml2json", "json2toml", "toml2yaml", "yaml2toml"],
)];

pub struct TomlTool;

impl Tool for TomlTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.toml".to_string(),
            category: Category::Converters,
            name: "TOML 互转",
            description: "TOML 与 JSON / YAML 互转（以 TOML 为中心的四向）。",
            keywords: &["toml", "json", "yaml", "config", "转换"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let mode = params
            .get(MODE)
            .and_then(|v| v.as_str())
            .unwrap_or("toml2json");
        let text = input.as_text();
        let src = text.trim();

        // 统一先解析成 serde_json::Value 作为中介。
        let value: serde_json::Value = match mode {
            "toml2json" | "toml2yaml" => toml::from_str(src)
                .map_err(|e| ToolError::InvalidInput(format!("TOML 解析失败: {e}")))?,
            "json2toml" => serde_json::from_str(src)
                .map_err(|e| ToolError::InvalidInput(format!("JSON 解析失败: {e}")))?,
            "yaml2toml" => serde_yaml::from_str(src)
                .map_err(|e| ToolError::InvalidInput(format!("YAML 解析失败: {e}")))?,
            other => {
                return Err(ToolError::InvalidParam {
                    name: MODE.to_string(),
                    reason: format!("未知方向 '{other}'"),
                })
            }
        };

        let out = match mode {
            "toml2json" => serde_json::to_string_pretty(&value)
                .map_err(|e| ToolError::InvalidInput(format!("JSON 序列化失败: {e}")))?,
            "toml2yaml" => serde_yaml::to_string(&value)
                .map_err(|e| ToolError::InvalidInput(format!("YAML 序列化失败: {e}")))?,
            "json2toml" | "yaml2toml" => toml::to_string_pretty(&value)
                .map_err(|e| ToolError::InvalidInput(format!("TOML 序列化失败: {e}")))?,
            _ => unreachable!(),
        };
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, mode: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        TomlTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn toml_to_json() {
        let out = run("name = \"pulsar\"\nport = 8080", "toml2json");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["name"], "pulsar");
        assert_eq!(v["port"], 8080);
    }

    #[test]
    fn json_to_toml() {
        let out = run(r#"{"name":"pulsar","port":8080}"#, "json2toml");
        assert!(out.contains("name = \"pulsar\""));
        assert!(out.contains("port = 8080"));
    }

    #[test]
    fn toml_json_roundtrip() {
        let original = "title = \"x\"\n\n[server]\nhost = \"localhost\"\n";
        let json = run(original, "toml2json");
        let back = run(&json, "json2toml");
        assert!(back.contains("title = \"x\""));
        assert!(back.contains("[server]"));
        assert!(back.contains("host = \"localhost\""));
    }

    #[test]
    fn yaml_to_toml() {
        let out = run("name: pulsar\nport: 8080", "yaml2toml");
        assert!(out.contains("name = \"pulsar\""));
    }

    #[test]
    fn invalid_toml_errors() {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("toml2json".to_string()));
        let err = TomlTool.run(ToolValue::text("= = ="), &p).unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
