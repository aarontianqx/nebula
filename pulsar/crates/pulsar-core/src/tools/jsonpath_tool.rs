//! JSONPath 查询（`testers.jsonpath`）。
//!
//! 用 JSONPath 表达式在输入 JSON 中查询节点，输出匹配结果（JSON 数组）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use serde_json_path::JsonPath;
use std::sync::OnceLock;

const PATH: &str = "path";

static PARAMS: &[ParamSpec] = &[ParamSpec::string(PATH, "JSONPath", "$")];

pub struct JsonPathTool;

impl Tool for JsonPathTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "testers.jsonpath".to_string(),
            category: Category::Testers,
            name: "JSONPath 查询",
            description: "用 JSONPath 表达式查询 JSON，返回匹配的节点。",
            keywords: &["jsonpath", "json", "query", "path", "查询"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let path_str = params.get(PATH).and_then(|v| v.as_str()).unwrap_or("$");

        let value: serde_json::Value = serde_json::from_str(input.as_text().trim())
            .map_err(|e| ToolError::InvalidInput(format!("非法 JSON: {e}")))?;

        let path = JsonPath::parse(path_str).map_err(|e| ToolError::InvalidParam {
            name: PATH.to_string(),
            reason: format!("非法 JSONPath: {e}"),
        })?;

        let nodes = path.query(&value).all();
        let result = serde_json::Value::Array(nodes.into_iter().cloned().collect());
        let out = serde_json::to_string_pretty(&result)
            .map_err(|e| ToolError::InvalidInput(e.to_string()))?;
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(path: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(PATH.to_string(), ParamValue::Str(path.to_string()));
        p
    }

    const DOC: &str = r#"{"users":[{"name":"a","age":30},{"name":"b","age":25}]}"#;

    #[test]
    fn selects_field_across_array() {
        let out = JsonPathTool
            .run(ToolValue::text(DOC), &params("$.users[*].name"))
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.as_text()).unwrap();
        assert_eq!(v, serde_json::json!(["a", "b"]));
    }

    #[test]
    fn filter_expression() {
        let out = JsonPathTool
            .run(ToolValue::text(DOC), &params("$.users[?@.age > 28].name"))
            .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out.as_text()).unwrap();
        assert_eq!(v, serde_json::json!(["a"]));
    }

    #[test]
    fn rejects_invalid_path() {
        let err = JsonPathTool
            .run(ToolValue::text(DOC), &params("$["))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }

    #[test]
    fn rejects_invalid_json() {
        let err = JsonPathTool
            .run(ToolValue::text("{bad}"), &params("$"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
