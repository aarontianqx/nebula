//! XML ↔ JSON 互转（`converters.xml_json`）。
//!
//! 采用一套实用约定（非标准，但覆盖常见场景）：
//! - 元素属性 → 以 `@` 前缀的键，如 `@id`。
//! - 元素文本 → 若该元素无子元素/属性则直接为字符串；否则放入 `#text` 键。
//! - 同名重复子元素 → 合并为数组。
//!
//! `xml2json` 用 `quick-xml` 事件流构建嵌套结构；`json2xml` 反向生成（属性、`#text`
//! 按上述约定还原，数组展开为重复元素）。

use crate::descriptor::{Category, ParamSpec, ToolDescriptor};
use crate::error::{ToolError, ToolResult};
use crate::tool::Tool;
use crate::value::{IoKind, ToolParams, ToolValue};
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde_json::{Map, Value};
use std::sync::OnceLock;

const MODE: &str = "mode";

static PARAMS: &[ParamSpec] = &[ParamSpec::enumerated(
    MODE,
    "方向",
    "xml2json",
    &["xml2json", "json2xml"],
)];

pub struct XmlJsonTool;

impl Tool for XmlJsonTool {
    fn descriptor(&self) -> &ToolDescriptor {
        static DESC: OnceLock<ToolDescriptor> = OnceLock::new();
        DESC.get_or_init(|| ToolDescriptor {
            id: "converters.xml_json".to_string(),
            category: Category::Converters,
            name: "XML ↔ JSON",
            description: "XML 与 JSON 互转（属性用 @ 前缀、文本用 #text、重复元素转数组）。",
            keywords: &["xml", "json", "convert", "转换"],
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
            .unwrap_or("xml2json");
        let text = input.as_text();
        let src = text.trim();
        match mode {
            "xml2json" => {
                let value = xml_to_value(src)?;
                serde_json::to_string_pretty(&value)
                    .map(ToolValue::text)
                    .map_err(|e| ToolError::InvalidInput(format!("JSON 序列化失败: {e}")))
            }
            "json2xml" => {
                let value: Value = serde_json::from_str(src)
                    .map_err(|e| ToolError::InvalidInput(format!("JSON 解析失败: {e}")))?;
                let xml = value_to_xml(&value)?;
                Ok(ToolValue::text(xml))
            }
            other => Err(ToolError::InvalidParam {
                name: MODE.to_string(),
                reason: format!("未知方向 '{other}'"),
            }),
        }
    }
}

// ---------- XML → JSON ----------

/// 解析后的元素中间表示。
#[derive(Default)]
struct Node {
    attrs: Vec<(String, String)>,
    children: Vec<(String, Node)>,
    text: String,
}

fn xml_to_value(src: &str) -> Result<Value, ToolError> {
    if src.is_empty() {
        return Err(ToolError::InvalidInput("请输入 XML".to_string()));
    }
    let mut reader = Reader::from_str(src);
    reader.config_mut().trim_text(true);

    // 用栈维护当前路径；根哨兵承接顶层元素。
    let mut stack: Vec<(String, Node)> = vec![(String::new(), Node::default())];

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(e)) => {
                let name = qname_to_string(e.name().as_ref());
                let mut node = Node::default();
                read_attrs(&e, &mut node)?;
                stack.push((name, node));
            }
            Ok(Event::Empty(e)) => {
                let name = qname_to_string(e.name().as_ref());
                let mut node = Node::default();
                read_attrs(&e, &mut node)?;
                let parent = stack.last_mut().unwrap();
                parent.1.children.push((name, node));
            }
            Ok(Event::End(_)) => {
                let (name, node) = stack.pop().unwrap();
                let parent = stack
                    .last_mut()
                    .ok_or_else(|| ToolError::InvalidInput("XML 结构不平衡".to_string()))?;
                parent.1.children.push((name, node));
            }
            Ok(Event::Text(e)) => {
                let t = e
                    .decode()
                    .map_err(|err| ToolError::InvalidInput(format!("文本解码失败: {err}")))?;
                if !t.trim().is_empty() {
                    stack.last_mut().unwrap().1.text.push_str(t.trim());
                }
            }
            Ok(Event::CData(e)) => {
                let bytes = e.into_inner();
                let t = String::from_utf8_lossy(&bytes);
                stack.last_mut().unwrap().1.text.push_str(t.trim());
            }
            Ok(_) => {}
            Err(e) => {
                return Err(ToolError::InvalidInput(format!(
                    "XML 解析失败 (位置 {}): {e}",
                    reader.error_position()
                )))
            }
        }
    }

    let root = stack.pop().unwrap().1;
    if root.children.len() != 1 {
        return Err(ToolError::InvalidInput(
            "XML 需要恰好一个根元素".to_string(),
        ));
    }
    let (root_name, root_node) = root.children.into_iter().next().unwrap();
    let mut obj = Map::new();
    obj.insert(root_name, node_to_value(root_node));
    Ok(Value::Object(obj))
}

fn read_attrs(e: &quick_xml::events::BytesStart, node: &mut Node) -> Result<(), ToolError> {
    for attr in e.attributes() {
        let attr = attr.map_err(|err| ToolError::InvalidInput(format!("属性解析失败: {err}")))?;
        let key = qname_to_string(attr.key.as_ref());
        let val = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|err| ToolError::InvalidInput(format!("属性值解码失败: {err}")))?
            .into_owned();
        node.attrs.push((format!("@{key}"), val));
    }
    Ok(())
}

/// 把中间节点转成 JSON。无属性无子元素的纯文本节点直接折叠为字符串。
fn node_to_value(node: Node) -> Value {
    if node.attrs.is_empty() && node.children.is_empty() {
        return Value::String(node.text);
    }
    let mut obj = Map::new();
    for (k, v) in node.attrs {
        obj.insert(k, Value::String(v));
    }
    // 子元素：同名合并为数组。
    for (name, child) in node.children {
        let child_value = node_to_value(child);
        match obj.get_mut(&name) {
            Some(Value::Array(arr)) => arr.push(child_value),
            Some(existing) => {
                let prev = existing.take();
                obj.insert(name, Value::Array(vec![prev, child_value]));
            }
            None => {
                obj.insert(name, child_value);
            }
        }
    }
    if !node.text.is_empty() {
        obj.insert("#text".to_string(), Value::String(node.text));
    }
    Value::Object(obj)
}

// ---------- JSON → XML ----------

fn value_to_xml(value: &Value) -> Result<String, ToolError> {
    let obj = value
        .as_object()
        .ok_or_else(|| ToolError::InvalidInput("JSON→XML 需要一个对象".to_string()))?;
    if obj.len() != 1 {
        return Err(ToolError::InvalidInput(
            "JSON→XML 顶层需恰好一个根键".to_string(),
        ));
    }
    let (root, body) = obj.iter().next().unwrap();
    let mut out = String::new();
    write_element(&mut out, root, body);
    Ok(out)
}

fn write_element(out: &mut String, name: &str, value: &Value) {
    match value {
        Value::Array(arr) => {
            // 数组：展开为多个同名元素。
            for item in arr {
                write_element(out, name, item);
            }
        }
        Value::Object(map) => {
            let mut attrs = String::new();
            let mut text: Option<&str> = None;
            for (k, v) in map {
                if let Some(attr) = k.strip_prefix('@') {
                    attrs.push_str(&format!(" {attr}=\"{}\"", escape_attr(&value_to_text(v))));
                } else if k == "#text" {
                    if let Value::String(s) = v {
                        text = Some(s);
                    }
                }
            }
            let has_children = map.keys().any(|k| !k.starts_with('@') && k != "#text");
            if !has_children && text.is_none() {
                out.push_str(&format!("<{name}{attrs}/>"));
                return;
            }
            out.push_str(&format!("<{name}{attrs}>"));
            if let Some(t) = text {
                out.push_str(&escape_text(t));
            }
            for (k, v) in map {
                if k.starts_with('@') || k == "#text" {
                    continue;
                }
                write_element(out, k, v);
            }
            out.push_str(&format!("</{name}>"));
        }
        other => {
            // 标量：作为元素文本。
            let t = value_to_text(other);
            if t.is_empty() {
                out.push_str(&format!("<{name}/>"));
            } else {
                out.push_str(&format!("<{name}>{}</{name}>", escape_text(&t)));
            }
        }
    }
}

fn value_to_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn qname_to_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::ParamValue;

    fn run(input: &str, mode: &str) -> String {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str(mode.to_string()));
        XmlJsonTool
            .run(ToolValue::text(input), &p)
            .unwrap()
            .as_text()
            .into_owned()
    }

    #[test]
    fn simple_element_to_json() {
        let out = run("<note>hello</note>", "xml2json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["note"], "hello");
    }

    #[test]
    fn attributes_use_at_prefix() {
        let out = run(r#"<user id="1">bob</user>"#, "xml2json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["user"]["@id"], "1");
        assert_eq!(v["user"]["#text"], "bob");
    }

    #[test]
    fn repeated_children_become_array() {
        let out = run("<list><item>a</item><item>b</item></list>", "xml2json");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert!(v["list"]["item"].is_array());
        assert_eq!(v["list"]["item"][0], "a");
        assert_eq!(v["list"]["item"][1], "b");
    }

    #[test]
    fn json_to_xml_basic() {
        let out = run(r#"{"note":"hello"}"#, "json2xml");
        assert_eq!(out, "<note>hello</note>");
    }

    #[test]
    fn json_to_xml_with_attr_and_array() {
        let out = run(r#"{"list":{"item":["a","b"]}}"#, "json2xml");
        assert_eq!(out, "<list><item>a</item><item>b</item></list>");
    }

    #[test]
    fn xml_json_roundtrip_structure() {
        let xml = r#"<root><a>1</a><b>2</b></root>"#;
        let json = run(xml, "xml2json");
        let back = run(&json, "json2xml");
        assert_eq!(back, xml);
    }

    #[test]
    fn invalid_xml_errors() {
        let mut p = ToolParams::new();
        p.insert(MODE.to_string(), ParamValue::Str("xml2json".to_string()));
        let err = XmlJsonTool
            .run(ToolValue::text("<a><b></a>"), &p)
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }
}
