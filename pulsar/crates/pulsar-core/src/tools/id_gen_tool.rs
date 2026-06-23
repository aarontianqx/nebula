//! 唯一 ID 生成器（`generators.id`）。
//!
//! 支持 UUID v4 / UUID v7 / ULID / NanoID，可一次生成多个（忽略输入）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const KIND: &str = "kind";
const COUNT: &str = "count";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::enumerated(
        KIND,
        "类型",
        "uuid_v4",
        &["uuid_v4", "uuid_v7", "ulid", "nanoid"],
    ),
    ParamSpec::int(COUNT, "数量", "1"),
];

pub struct IdGenTool;

impl Tool for IdGenTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.id".to_string(),
            category: Category::Generators,
            name: "ID 生成器",
            description: "生成 UUID v4/v7、ULID 或 NanoID，可批量。",
            keywords: &["uuid", "ulid", "nanoid", "guid", "id", "生成"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, _input: ToolValue, params: &ToolParams) -> ToolResult {
        let kind = params
            .get(KIND)
            .and_then(|v| v.as_str())
            .unwrap_or("uuid_v4");
        let count = params
            .get(COUNT)
            .and_then(|v| v.as_int())
            .unwrap_or(1)
            .clamp(1, 1000) as usize;

        let gen: fn() -> String = match kind {
            "uuid_v4" => || uuid::Uuid::new_v4().to_string(),
            "uuid_v7" => || uuid::Uuid::now_v7().to_string(),
            "ulid" => || ulid::Ulid::new().to_string(),
            "nanoid" => || nanoid::nanoid!(),
            other => {
                return Err(ToolError::InvalidParam {
                    name: KIND.to_string(),
                    reason: format!("未知类型 '{other}'"),
                })
            }
        };

        let out = (0..count).map(|_| gen()).collect::<Vec<_>>().join("\n");
        Ok(ToolValue::text(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(kind: &str, count: i64) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(KIND.to_string(), ParamValue::Str(kind.to_string()));
        p.insert(COUNT.to_string(), ParamValue::Int(count));
        p
    }

    #[test]
    fn generates_requested_count() {
        let out = IdGenTool
            .run(ToolValue::text(""), &params("uuid_v4", 5))
            .unwrap();
        assert_eq!(out.as_text().lines().count(), 5);
    }

    #[test]
    fn uuid_v4_has_valid_shape() {
        let out = IdGenTool
            .run(ToolValue::text(""), &params("uuid_v4", 1))
            .unwrap();
        let id = out.as_text().into_owned();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn ulid_parses() {
        let out = IdGenTool
            .run(ToolValue::text(""), &params("ulid", 1))
            .unwrap();
        assert!(ulid::Ulid::from_string(out.as_text().trim()).is_ok());
    }

    #[test]
    fn nanoid_default_len() {
        let out = IdGenTool
            .run(ToolValue::text(""), &params("nanoid", 1))
            .unwrap();
        assert_eq!(out.as_text().trim().len(), 21);
    }

    #[test]
    fn ids_are_unique() {
        let out = IdGenTool
            .run(ToolValue::text(""), &params("uuid_v4", 10))
            .unwrap();
        let text = out.as_text();
        let lines: std::collections::HashSet<_> = text.lines().collect();
        assert_eq!(lines.len(), 10);
    }
}
