//! Bcrypt 生成 / 校验（`generators.bcrypt`）。
//!
//! - `hash`：用指定 cost 对输入口令计算 bcrypt 散列。
//! - `verify`：把输入按「口令<空格>散列」拆分校验是否匹配（也支持换行分隔）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use std::sync::OnceLock;

const MODE: &str = "mode";
const COST: &str = "cost";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::enumerated(MODE, "模式", "hash", &["hash", "verify"]),
    ParamSpec::int(COST, "cost (4–15)", "10"),
];

pub struct BcryptTool;

impl Tool for BcryptTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "generators.bcrypt".to_string(),
            category: Category::Generators,
            name: "Bcrypt",
            description: "生成 bcrypt 散列，或校验「口令 散列」是否匹配。",
            keywords: &["bcrypt", "password", "hash", "口令", "校验"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let mode = params.get(MODE).and_then(|v| v.as_str()).unwrap_or("hash");
        let text = input.as_text();

        match mode {
            "hash" => {
                let cost = params
                    .get(COST)
                    .and_then(|v| v.as_int())
                    .unwrap_or(bcrypt::DEFAULT_COST as i64)
                    .clamp(4, 15) as u32;
                let password = text.trim_end_matches('\n');
                let hashed = bcrypt::hash(password, cost)
                    .map_err(|e| ToolError::InvalidInput(format!("bcrypt 计算失败: {e}")))?;
                Ok(ToolValue::text(hashed))
            }
            "verify" => {
                let (password, hash) = split_verify_input(&text)?;
                let ok = bcrypt::verify(password, hash)
                    .map_err(|e| ToolError::InvalidInput(format!("校验失败: {e}")))?;
                Ok(ToolValue::text(if ok {
                    "✅ 匹配"
                } else {
                    "❌ 不匹配"
                }))
            }
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知模式 '{other}'"),
            }),
        }
    }
}

/// 校验模式输入解析：第一段是口令，其余是 bcrypt 散列（以空白分隔）。
fn split_verify_input(text: &str) -> Result<(&str, &str), ToolError> {
    let trimmed = text.trim();
    // bcrypt 散列以 $2 开头；以此定位散列起点，前面即口令。
    if let Some(pos) = trimmed.find("$2") {
        let password = trimmed[..pos].trim();
        let hash = trimmed[pos..].trim();
        if !password.is_empty() && !hash.is_empty() {
            return Ok((password, hash));
        }
    }
    Err(ToolError::InvalidInput(
        "校验需输入「口令 散列」，散列以 $2 开头".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn hash(password: &str, cost: i64) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("hash".to_string()));
        p.insert(COST.to_string(), ParamValue::Int(cost));
        BcryptTool
            .run(ToolValue::text(password), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    fn verify(input: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("verify".to_string()));
        BcryptTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn hash_has_bcrypt_shape() {
        // 用低 cost 让测试快。
        let h = hash("secret", 4);
        assert!(h.starts_with("$2"));
        assert!(h.len() == 60);
    }

    #[test]
    fn verify_matches_generated_hash() {
        let h = hash("hunter2", 4);
        let out = verify(&format!("hunter2 {h}"));
        assert!(out.contains("匹配"));
        assert!(!out.contains("不匹配"));
    }

    #[test]
    fn verify_rejects_wrong_password() {
        let h = hash("right", 4);
        let out = verify(&format!("wrong {h}"));
        assert!(out.contains("不匹配"));
    }

    #[test]
    fn verify_bad_input_errors() {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("verify".to_string()));
        let err = BcryptTool
            .run(ToolValue::text("no hash here"), &p)
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
