//! 时间戳 ↔ 日期转换工具（`converters.timestamp`）。
//!
//! - to_datetime：Unix 时间戳（秒/毫秒自动识别）→ 所选时区 + UTC 的日期字符串。
//!   输入留空则使用「当前时间」。
//! - to_timestamp：日期字符串 → Unix 秒与毫秒。
//!   带时区的输入（RFC3339）尊重其偏移；无时区的输入按「所选时区」解释。
//!
//! 时区：`timezone` 参数，默认 `system`（跟随系统时区，检测失败回退 UTC），
//! 也可选 UTC 或常用 IANA 时区。这样中国用户开箱即默认上海/系统时区，
//! 同时保留切换到任意常用时区的通用性。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::detect::{Detector, Rule};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;
use std::str::FromStr;
use std::sync::OnceLock;

const MODE: &str = "mode";
const TIMEZONE: &str = "timezone";

static PARAMS: &[ParamSpec] = &[
    ParamSpec::enumerated(
        MODE,
        "方向",
        "to_datetime",
        &["to_datetime", "to_timestamp"],
    ),
    ParamSpec::enumerated(
        TIMEZONE,
        "时区",
        "system",
        &[
            "system",
            "UTC",
            "Asia/Shanghai",
            "Asia/Tokyo",
            "Asia/Kolkata",
            "Europe/London",
            "Europe/Paris",
            "America/New_York",
            "America/Los_Angeles",
            "Australia/Sydney",
        ],
    ),
];

// 10 位（秒）或 13 位（毫秒）纯数字，强烈暗示是时间戳。
static DETECTORS: &[Detector] = &[Detector::new(Rule::Regex(r"^\s*\d{10}(\d{3})?\s*$"), 75)];

pub struct TimestampTool;

impl Tool for TimestampTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.timestamp".to_string(),
            category: Category::Converters,
            name: "时间戳转换",
            description: "Unix 时间戳与日期互转（秒/毫秒自动识别，可选时区，留空取当前时间）。",
            keywords: &[
                "timestamp",
                "unix",
                "epoch",
                "date",
                "时间戳",
                "日期",
                "时区",
            ],
            params: PARAMS,
            input_kind: IoKind::Text,
            output_kind: IoKind::Text,
            pipeable: true,
            detectors: DETECTORS,
        })
    }

    fn run(&self, input: ToolValue, params: &ToolParams) -> ToolResult {
        let mode = params
            .get(MODE)
            .and_then(|v| v.as_str())
            .unwrap_or("to_datetime");
        let tz = resolve_timezone(
            params
                .get(TIMEZONE)
                .and_then(|v| v.as_str())
                .unwrap_or("system"),
        )?;

        let text = input.as_text();
        let trimmed = text.trim();

        match mode {
            "to_datetime" => to_datetime(trimmed, tz),
            "to_timestamp" => to_timestamp(trimmed, tz),
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知方向 '{other}'"),
            }),
        }
    }
}

/// 解析 `timezone` 参数为具体时区。`system` → 检测系统时区（失败回退 UTC）。
fn resolve_timezone(name: &str) -> Result<Tz, ToolError> {
    if name == "system" {
        let detected = iana_time_zone::get_timezone().unwrap_or_else(|_| "UTC".to_string());
        return Ok(Tz::from_str(&detected).unwrap_or(Tz::UTC));
    }
    Tz::from_str(name).map_err(|_| ToolError::InvalidParam {
        name: TIMEZONE.to_string(),
        reason: format!("未知时区 '{name}'"),
    })
}

fn to_datetime(input: &str, tz: Tz) -> ToolResult {
    // 留空 → 当前时间。
    let dt_utc: DateTime<Utc> = if input.is_empty() {
        Utc::now()
    } else {
        let n: i64 = input
            .parse()
            .map_err(|_| ToolError::InvalidInput(format!("不是合法的整数时间戳: {input}")))?;
        // 启发式：>= 1e12 视为毫秒，否则秒。
        if n.abs() >= 1_000_000_000_000 {
            Utc.timestamp_millis_opt(n)
                .single()
                .ok_or_else(|| ToolError::InvalidInput("时间戳超出范围".into()))?
        } else {
            Utc.timestamp_opt(n, 0)
                .single()
                .ok_or_else(|| ToolError::InvalidInput("时间戳超出范围".into()))?
        }
    };

    let zoned = dt_utc.with_timezone(&tz);
    let weekday = weekday_zh(zoned.weekday());
    let relative = humanize_relative(dt_utc);

    let out = format!(
        "{tz_name}: {local} {weekday}（{relative}）\nUTC:    {utc}\nRFC3339: {rfc}\n\nseconds:      {secs}\nmilliseconds: {millis}",
        tz_name = tz.name(),
        local = zoned.format("%Y-%m-%d %H:%M:%S %z"),
        utc = dt_utc.format("%Y-%m-%d %H:%M:%S"),
        rfc = zoned.to_rfc3339(),
        secs = dt_utc.timestamp(),
        millis = dt_utc.timestamp_millis(),
    );
    Ok(ToolValue::text(out))
}

fn to_timestamp(input: &str, tz: Tz) -> ToolResult {
    // 留空 → 当前时间（与 to_datetime 一致）。
    if input.is_empty() {
        return Ok(format_timestamp(Utc::now()));
    }

    // 1) 带时区偏移的 RFC3339：尊重其自带偏移。
    if let Ok(dt) = DateTime::parse_from_rfc3339(input) {
        return Ok(format_timestamp(dt.with_timezone(&Utc)));
    }

    // 2) 无时区的 datetime：按所选时区解释。
    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M:%S") {
        let dt = tz.from_local_datetime(&naive).single().ok_or_else(|| {
            ToolError::InvalidInput("该本地时间不存在或有歧义（夏令时切换）".into())
        })?;
        return Ok(format_timestamp(dt.with_timezone(&Utc)));
    }

    // 3) 仅日期：按所选时区当天 00:00:00 解释。
    if let Ok(date) = NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let naive = date
            .and_hms_opt(0, 0, 0)
            .ok_or_else(|| ToolError::InvalidInput("非法日期".into()))?;
        let dt = tz
            .from_local_datetime(&naive)
            .single()
            .ok_or_else(|| ToolError::InvalidInput("该本地时间不存在或有歧义".into()))?;
        return Ok(format_timestamp(dt.with_timezone(&Utc)));
    }

    Err(ToolError::InvalidInput(format!(
        "无法解析日期: {input}（支持 RFC3339、'YYYY-MM-DD HH:MM:SS' 或 'YYYY-MM-DD'）"
    )))
}

fn format_timestamp(dt: DateTime<Utc>) -> ToolValue {
    ToolValue::text(format!(
        "seconds:      {}\nmilliseconds: {}",
        dt.timestamp(),
        dt.timestamp_millis()
    ))
}

fn weekday_zh(w: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match w {
        Mon => "周一",
        Tue => "周二",
        Wed => "周三",
        Thu => "周四",
        Fri => "周五",
        Sat => "周六",
        Sun => "周日",
    }
}

/// 相对当前时间的人性化描述（如「3 天前」「2 小时后」「刚刚」）。
fn humanize_relative(dt: DateTime<Utc>) -> String {
    let delta = Utc::now().signed_duration_since(dt);
    let secs = delta.num_seconds();
    let abs = secs.abs();
    let (value, unit) = if abs < 60 {
        return "刚刚".to_string();
    } else if abs < 3600 {
        (abs / 60, "分钟")
    } else if abs < 86_400 {
        (abs / 3600, "小时")
    } else if abs < 2_592_000 {
        (abs / 86_400, "天")
    } else if abs < 31_536_000 {
        (abs / 2_592_000, "个月")
    } else {
        (abs / 31_536_000, "年")
    };
    if secs >= 0 {
        format!("{value} {unit}前")
    } else {
        format!("{value} {unit}后")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn params(mode: &str, tz: &str) -> ToolParams {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        p.insert(TIMEZONE.to_string(), ParamValue::Str(tz.to_string()));
        p
    }

    #[test]
    fn seconds_to_datetime_in_shanghai() {
        // 1609459200 = 2021-01-01 00:00:00 UTC = 2021-01-01 08:00:00 +08:00
        let out = TimestampTool
            .run(
                ToolValue::text("1609459200"),
                &params("to_datetime", "Asia/Shanghai"),
            )
            .unwrap();
        let text = out.as_text();
        assert!(text.contains("Asia/Shanghai: 2021-01-01 08:00:00 +0800"));
        assert!(text.contains("UTC:    2021-01-01 00:00:00"));
        assert!(text.contains("seconds:      1609459200"));
    }

    #[test]
    fn utc_timezone_explicit() {
        let out = TimestampTool
            .run(ToolValue::text("0"), &params("to_datetime", "UTC"))
            .unwrap();
        assert!(out.as_text().contains("UTC:    1970-01-01 00:00:00"));
    }

    #[test]
    fn millis_auto_detected() {
        let out = TimestampTool
            .run(
                ToolValue::text("1609459200000"),
                &params("to_datetime", "UTC"),
            )
            .unwrap();
        assert!(out.as_text().contains("UTC:    2021-01-01 00:00:00"));
    }

    #[test]
    fn empty_input_uses_now() {
        let out = TimestampTool
            .run(ToolValue::text(""), &params("to_datetime", "UTC"))
            .unwrap();
        // 当前时间应被识别为「刚刚」。
        assert!(out.as_text().contains("刚刚"));
    }

    #[test]
    fn to_timestamp_empty_uses_now() {
        let before = Utc::now().timestamp();
        let out = TimestampTool
            .run(ToolValue::text(""), &params("to_timestamp", "UTC"))
            .unwrap();
        let after = Utc::now().timestamp();
        let text = out.as_text();
        // 提取 seconds 行的值并断言落在调用窗口内。
        let secs: i64 = text
            .lines()
            .find_map(|l| l.strip_prefix("seconds:"))
            .and_then(|s| s.trim().parse().ok())
            .expect("seconds line present");
        assert!(before <= secs && secs <= after, "now timestamp in range");
    }

    #[test]
    fn naive_datetime_parsed_in_shanghai() {
        // 2021-01-01 08:00:00 在上海 = 1609459200 (UTC midnight)
        let out = TimestampTool
            .run(
                ToolValue::text("2021-01-01 08:00:00"),
                &params("to_timestamp", "Asia/Shanghai"),
            )
            .unwrap();
        assert!(out.as_text().contains("seconds:      1609459200"));
    }

    #[test]
    fn naive_datetime_parsed_in_utc() {
        let out = TimestampTool
            .run(
                ToolValue::text("2021-01-01 00:00:00"),
                &params("to_timestamp", "UTC"),
            )
            .unwrap();
        assert!(out.as_text().contains("seconds:      1609459200"));
    }

    #[test]
    fn rfc3339_respects_its_own_offset() {
        // 带偏移的输入应忽略所选时区，用自带偏移。
        let out = TimestampTool
            .run(
                ToolValue::text("2021-01-01T08:00:00+08:00"),
                &params("to_timestamp", "America/New_York"),
            )
            .unwrap();
        assert!(out.as_text().contains("seconds:      1609459200"));
    }

    #[test]
    fn date_only_in_shanghai() {
        let out = TimestampTool
            .run(
                ToolValue::text("2021-01-01"),
                &params("to_timestamp", "Asia/Shanghai"),
            )
            .unwrap();
        // 2021-01-01 00:00:00 +08:00 = 1609430400
        assert!(out.as_text().contains("seconds:      1609430400"));
    }

    #[test]
    fn unknown_timezone_errors() {
        let err = TimestampTool
            .run(ToolValue::text("0"), &params("to_datetime", "Mars/Olympus"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidParam { .. }));
    }

    #[test]
    fn rejects_garbage_timestamp() {
        let err = TimestampTool
            .run(ToolValue::text("abc"), &params("to_datetime", "UTC"))
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
