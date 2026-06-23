//! Cron 表达式解析（`converters.cron`）。
//!
//! 接受标准 5 段（分 时 日 月 周）或带秒的 6/7 段表达式，输出字段拆解与
//! 接下来若干次执行时间（UTC）。底层用 `cron` crate（要求 6/7 段，故 5 段会自动补秒）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use chrono::Utc;
use cron::Schedule;
use std::str::FromStr;
use std::sync::OnceLock;

const COUNT: &str = "count";

static PARAMS: &[ParamSpec] = &[ParamSpec::int(COUNT, "预测次数", "5")];

pub struct CronTool;

impl Tool for CronTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.cron".to_string(),
            category: Category::Converters,
            name: "Cron 解析",
            description: "解析 Cron 表达式，拆解字段并预测接下来的执行时间（UTC）。",
            keywords: &["cron", "crontab", "定时", "schedule", "表达式"],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: false,
            detectors: &[],
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let raw = input.as_text();
        let expr = raw.trim();
        if expr.is_empty() {
            return Err(ToolError::InvalidInput("请输入 Cron 表达式".to_string()));
        }
        let count = params
            .get(COUNT)
            .and_then(|v| v.as_int())
            .unwrap_or(5)
            .clamp(1, 50) as usize;

        let fields: Vec<&str> = expr.split_whitespace().collect();
        let describe = describe_fields(&fields)?;
        let normalized = normalize(&fields)?;

        let schedule = Schedule::from_str(&normalized)
            .map_err(|e| ToolError::InvalidInput(format!("无效 Cron 表达式: {e}")))?;

        let mut out = describe;
        out.push_str("\n\n接下来的执行时间 (UTC):\n");
        let mut any = false;
        for dt in schedule.upcoming(Utc).take(count) {
            out.push_str(&format!("  {}\n", dt.format("%Y-%m-%d %H:%M:%S %a")));
            any = true;
        }
        if !any {
            out.push_str("  （未来无匹配时间）\n");
        }
        Ok(ToolValue::text(out.trim_end().to_string()))
    }
}

/// 把用户输入规整为 `cron` crate 接受的 6 段（秒 分 时 日 月 周）。
fn normalize(fields: &[&str]) -> Result<String, ToolError> {
    match fields.len() {
        // 标准 Unix 5 段：补秒为 0。
        5 => Ok(format!("0 {}", fields.join(" "))),
        // 已含秒（6 段）或含年（7 段）：原样。
        6 | 7 => Ok(fields.join(" ")),
        n => Err(ToolError::InvalidInput(format!(
            "Cron 需要 5、6 或 7 段，实际 {n} 段"
        ))),
    }
}

/// 给出字段拆解说明（按规整后的语义对齐）。
fn describe_fields(fields: &[&str]) -> Result<String, ToolError> {
    let (sec, min, hour, dom, mon, dow) = match fields.len() {
        5 => ("0", fields[0], fields[1], fields[2], fields[3], fields[4]),
        6 => (
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5],
        ),
        7 => (
            fields[0], fields[1], fields[2], fields[3], fields[4], fields[5],
        ),
        n => {
            return Err(ToolError::InvalidInput(format!(
                "Cron 需要 5、6 或 7 段，实际 {n} 段"
            )))
        }
    };
    let mut s = String::from("字段拆解:\n");
    s.push_str(&format!("  秒:   {sec}\n"));
    s.push_str(&format!("  分:   {min}\n"));
    s.push_str(&format!("  时:   {hour}\n"));
    s.push_str(&format!("  日:   {dom}\n"));
    s.push_str(&format!("  月:   {mon}\n"));
    s.push_str(&format!("  周:   {dow}"));
    if fields.len() == 7 {
        s.push_str(&format!("\n  年:   {}", fields[6]));
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(expr: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(COUNT.to_string(), ParamValue::Int(3));
        CronTool
            .run(ToolValue::text(expr), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn parses_standard_five_field() {
        // 每天 0 点。
        let out = run("0 0 * * *");
        assert!(out.contains("字段拆解"));
        assert!(out.contains("接下来的执行时间"));
        // 应预测 3 行时间。
        let times = out.lines().filter(|l| l.trim().starts_with("20")).count();
        assert_eq!(times, 3);
    }

    #[test]
    fn accepts_six_field_with_seconds() {
        let out = run("30 0 0 * * *");
        assert!(out.contains("秒:   30"));
    }

    #[test]
    fn five_field_seconds_default_zero() {
        let out = run("0 12 * * *");
        assert!(out.contains("秒:   0"));
        assert!(out.contains("分:   0"));
        assert!(out.contains("时:   12"));
    }

    #[test]
    fn invalid_field_count_errors() {
        let err = CronTool
            .run(ToolValue::text("* * *"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[test]
    fn invalid_expression_errors() {
        let err = CronTool
            .run(ToolValue::text("99 99 99 99 99"), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
