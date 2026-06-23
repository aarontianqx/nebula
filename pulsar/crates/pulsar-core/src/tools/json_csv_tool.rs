//! JSON ↔ CSV 互转（`converters.json_csv`）。
//!
//! - `json2csv`：输入一个对象数组，列头取所有对象键的并集（按首次出现顺序），
//!   缺失字段留空；嵌套值序列化为紧凑 JSON 字符串。
//! - `csv2json`：首行为表头，每行转成一个对象，输出格式化后的对象数组。
//!
//! CSV 的引号/逗号/换行转义交由 `csv` crate 处理，避免手写出错。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use serde_json::{Map, Value};
use std::sync::OnceLock;

const MODE: &str = "mode";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    MODE,
    "方向",
    "json2csv",
    &["json2csv", "csv2json"],
)];

pub struct JsonCsvTool;

impl Tool for JsonCsvTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.json_csv".to_string(),
            category: Category::Converters,
            name: "JSON ↔ CSV",
            description: "对象数组与 CSV 表格互转，自动处理表头与引号转义。",
            keywords: &["json", "csv", "表格", "转换"],
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
            .unwrap_or("json2csv");
        let text = input.as_text();
        match mode {
            "json2csv" => json_to_csv(&text),
            "csv2json" => csv_to_json(&text),
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知方向 '{other}'"),
            }),
        }
    }
}

fn json_to_csv(text: &str) -> ToolResult {
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|e| ToolError::InvalidInput(format!("JSON 解析失败: {e}")))?;
    let array = value
        .as_array()
        .ok_or_else(|| ToolError::InvalidInput("JSON→CSV 需要一个对象数组".to_string()))?;

    // 收集列头：按首次出现顺序去重。
    let mut headers: Vec<String> = Vec::new();
    for item in array {
        let obj = item
            .as_object()
            .ok_or_else(|| ToolError::InvalidInput("数组元素必须都是对象".to_string()))?;
        for key in obj.keys() {
            if !headers.iter().any(|h| h == key) {
                headers.push(key.clone());
            }
        }
    }

    let mut wtr = csv::Writer::from_writer(vec![]);
    wtr.write_record(&headers)
        .map_err(|e| ToolError::InvalidInput(format!("写出 CSV 失败: {e}")))?;
    for item in array {
        let obj = item.as_object().unwrap();
        let row: Vec<String> = headers.iter().map(|h| cell_to_string(obj.get(h))).collect();
        wtr.write_record(&row)
            .map_err(|e| ToolError::InvalidInput(format!("写出 CSV 失败: {e}")))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| ToolError::InvalidInput(format!("写出 CSV 失败: {e}")))?;
    let out = String::from_utf8(bytes)
        .map_err(|e| ToolError::InvalidInput(format!("CSV 非 UTF-8: {e}")))?;
    Ok(ToolValue::text(out))
}

/// 标量直接转字符串；null 转空串；嵌套对象/数组序列化为紧凑 JSON。
fn cell_to_string(v: Option<&Value>) -> String {
    match v {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Bool(b)) => b.to_string(),
        Some(Value::Number(n)) => n.to_string(),
        Some(other) => other.to_string(),
    }
}

fn csv_to_json(text: &str) -> ToolResult {
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(text.trim_start().as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| ToolError::InvalidInput(format!("读取 CSV 表头失败: {e}")))?
        .clone();

    let mut rows: Vec<Value> = Vec::new();
    for record in rdr.records() {
        let record =
            record.map_err(|e| ToolError::InvalidInput(format!("读取 CSV 行失败: {e}")))?;
        let mut obj = Map::new();
        for (i, header) in headers.iter().enumerate() {
            let cell = record.get(i).unwrap_or("");
            obj.insert(header.to_string(), infer_cell(cell));
        }
        rows.push(Value::Object(obj));
    }

    serde_json::to_string_pretty(&Value::Array(rows))
        .map(ToolValue::text)
        .map_err(|e| ToolError::InvalidInput(format!("写出 JSON 失败: {e}")))
}

/// 尝试把单元格还原为合适的 JSON 标量（数字/布尔/null），否则保留字符串。
fn infer_cell(cell: &str) -> Value {
    if cell.is_empty() {
        return Value::Null;
    }
    match cell {
        "true" => return Value::Bool(true),
        "false" => return Value::Bool(false),
        "null" => return Value::Null,
        _ => {}
    }
    if let Ok(i) = cell.parse::<i64>() {
        return Value::Number(i.into());
    }
    if let Ok(f) = cell.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return Value::Number(n);
        }
    }
    Value::String(cell.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, mode: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        JsonCsvTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn json_array_to_csv() {
        let out = run(r#"[{"a":1,"b":"x"},{"a":2,"b":"y"}]"#, "json2csv");
        assert_eq!(out.lines().next().unwrap(), "a,b");
        assert!(out.contains("1,x"));
        assert!(out.contains("2,y"));
    }

    #[test]
    fn csv_quotes_values_with_commas() {
        let out = run(r#"[{"name":"a,b"}]"#, "json2csv");
        assert!(out.contains("\"a,b\""));
    }

    #[test]
    fn union_of_keys_used_as_headers() {
        let out = run(r#"[{"a":1},{"b":2}]"#, "json2csv");
        assert_eq!(out.lines().next().unwrap(), "a,b");
    }

    #[test]
    fn csv_to_json_infers_types() {
        let out = run("a,b\n1,true\n2,hello", "csv2json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v[0]["a"], serde_json::json!(1));
        assert_eq!(v[0]["b"], serde_json::json!(true));
        assert_eq!(v[1]["b"], serde_json::json!("hello"));
    }

    #[test]
    fn non_array_json_errors() {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("json2csv".to_string()));
        let err = JsonCsvTool
            .run(ToolValue::text(r#"{"a":1}"#), &p)
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
