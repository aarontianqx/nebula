//! 工具注册表。
//!
//! 启动时收集所有 `Tool` 实例，建立 id 索引。GUI 与 CLI 都从这里取工具与元数据，
//! 因此**新增工具只需在 [`build_registry`] 里加一行**。

use pulsar_core::tools::{
    Base64Tool, BcryptTool, CaseTool, ColorTool, CronTool, DedupSortTool, DiffTool, HashTool,
    HexTool, HmacTool, HtmlEntityTool, IdGenTool, JsonCsvTool, JsonFormatTool, JsonPathTool,
    JsonYamlTool, JwtTool, NumberBaseTool, PasswordTool, QrTool, RegexTool, SlugTool, SqlTool,
    TextStatsTool, TimestampTool, TomlTool, UnicodeTool, UrlTool, XmlFormatTool, XmlJsonTool,
};
use pulsar_core::{Tool, ToolDescriptor, ToolError, ToolParams, ToolResult, ToolValue};
use serde::Serialize;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Smart Detection 的一个候选结果。
#[derive(Debug, Clone, Serialize)]
pub struct DetectionResult {
    pub tool_id: String,
    pub tool_name: String,
    pub confidence: u8,
}

/// 全部已注册工具的运行时索引。
pub struct ToolRegistry {
    tools: BTreeMap<String, Arc<dyn Tool>>,
}

impl ToolRegistry {
    fn from_tools(list: Vec<Arc<dyn Tool>>) -> Self {
        let mut tools = BTreeMap::new();
        for tool in list {
            let id = tool.descriptor().id.clone();
            debug_assert!(!tools.contains_key(&id), "duplicate tool id: {id}");
            tools.insert(id, tool);
        }
        ToolRegistry { tools }
    }

    /// 所有工具的描述符（顺序稳定）。
    pub fn descriptors(&self) -> Vec<ToolDescriptor> {
        self.tools
            .values()
            .map(|t| t.descriptor().clone())
            .collect()
    }

    /// 按 id 取工具。
    pub fn get(&self, id: &str) -> Option<Arc<dyn Tool>> {
        self.tools.get(id).cloned()
    }

    /// 运行指定工具。
    pub fn run(&self, id: &str, input: ToolValue, params: &ToolParams) -> ToolResult {
        let tool = self
            .get(id)
            .ok_or_else(|| ToolError::InvalidInput(format!("未知工具: {id}")))?;
        tool.run(input, params)
    }

    /// Smart Detection：对输入运行所有工具的检测规则，返回按置信度降序的候选。
    ///
    /// 每个工具取其命中规则里的最高置信度；空输入返回空列表。
    pub fn detect(&self, input: &str) -> Vec<DetectionResult> {
        if input.trim().is_empty() {
            return Vec::new();
        }
        let mut results: Vec<DetectionResult> = self
            .tools
            .values()
            .filter_map(|tool| {
                let d = tool.descriptor();
                let best = d
                    .detectors
                    .iter()
                    .filter(|det| det.rule.matches(input))
                    .map(|det| det.confidence)
                    .max()?;
                Some(DetectionResult {
                    tool_id: d.id.clone(),
                    tool_name: d.name.to_string(),
                    confidence: best,
                })
            })
            .collect();
        // 置信度降序；同分按工具名稳定排序。
        results.sort_by(|a, b| {
            b.confidence
                .cmp(&a.confidence)
                .then_with(|| a.tool_name.cmp(&b.tool_name))
        });
        results
    }

    /// 关键词模糊搜索（供 Command Palette）。
    pub fn search(&self, query: &str) -> Vec<ToolDescriptor> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return self.descriptors();
        }
        self.tools
            .values()
            .filter(|t| {
                let d = t.descriptor();
                d.name.to_lowercase().contains(&q)
                    || d.id.contains(&q)
                    || d.keywords.iter().any(|k| k.to_lowercase().contains(&q))
            })
            .map(|t| t.descriptor().clone())
            .collect()
    }
}

/// 构建全局注册表。**新增工具在此登记。**
pub fn build_registry() -> ToolRegistry {
    let tools: Vec<Arc<dyn Tool>> = vec![
        // Converters
        Arc::new(JsonYamlTool),
        Arc::new(TimestampTool),
        Arc::new(NumberBaseTool),
        Arc::new(JsonCsvTool),
        Arc::new(ColorTool),
        Arc::new(CronTool),
        Arc::new(TomlTool),
        Arc::new(XmlJsonTool),
        // Encoders / Decoders
        Arc::new(Base64Tool),
        Arc::new(UrlTool),
        Arc::new(HexTool),
        Arc::new(JwtTool),
        Arc::new(HtmlEntityTool),
        Arc::new(UnicodeTool),
        // Formatters
        Arc::new(JsonFormatTool),
        Arc::new(SqlTool),
        Arc::new(XmlFormatTool),
        // Generators
        Arc::new(IdGenTool),
        Arc::new(HashTool),
        Arc::new(PasswordTool),
        Arc::new(HmacTool),
        Arc::new(BcryptTool),
        Arc::new(QrTool),
        // Testers
        Arc::new(RegexTool),
        Arc::new(JsonPathTool),
        Arc::new(DiffTool),
        // Text
        Arc::new(CaseTool),
        Arc::new(TextStatsTool),
        Arc::new(DedupSortTool),
        Arc::new(SlugTool),
    ];
    ToolRegistry::from_tools(tools)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulsar_core::ParamValue;

    #[test]
    fn registry_contains_sample_tools() {
        let reg = build_registry();
        assert!(reg.get("formatters.json").is_some());
        assert!(reg.get("encoders.base64").is_some());
        assert!(reg.get("generators.id").is_some());
        assert!(reg.get("testers.regex").is_some());
        // P1 新增工具。
        assert!(reg.get("converters.color").is_some());
        assert!(reg.get("converters.json_csv").is_some());
        assert!(reg.get("converters.cron").is_some());
        assert!(reg.get("encoders.html_entity").is_some());
        assert!(reg.get("encoders.unicode").is_some());
        assert!(reg.get("formatters.sql").is_some());
        assert!(reg.get("generators.password").is_some());
        assert!(reg.get("generators.hmac").is_some());
        assert!(reg.get("text.stats").is_some());
        assert!(reg.get("text.dedup_sort").is_some());
        assert!(reg.get("text.slug").is_some());
        assert!(reg.get("converters.toml").is_some());
        assert!(reg.get("converters.xml_json").is_some());
        assert!(reg.get("formatters.xml").is_some());
        assert!(reg.get("generators.bcrypt").is_some());
        assert!(reg.get("generators.qr").is_some());
        assert_eq!(reg.descriptors().len(), 30);
    }

    #[test]
    fn runs_tool_by_id() {
        let reg = build_registry();
        let mut p = ToolParams::new();
        p.insert("mode".to_string(), ParamValue::Str("encode".to_string()));
        let out = reg
            .run("encoders.base64", ToolValue::text("hello"), &p)
            .unwrap();
        assert_eq!(out.as_text(), "aGVsbG8=");
    }

    #[test]
    fn search_matches_keywords() {
        let reg = build_registry();
        let hits = reg.search("decode");
        assert!(hits.iter().any(|d| d.id == "encoders.base64"));
    }

    #[test]
    fn unknown_tool_errors() {
        let reg = build_registry();
        let err = reg
            .run("nope.nope", ToolValue::text(""), &ToolParams::new())
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[test]
    fn detect_jwt_ranks_first() {
        let reg = build_registry();
        let jwt = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjMifQ.sig";
        let results = reg.detect(jwt);
        assert!(!results.is_empty());
        assert_eq!(results[0].tool_id, "encoders.jwt");
    }

    #[test]
    fn detect_json_prefers_formatter() {
        let reg = build_registry();
        let results = reg.detect(r#"{"a":1,"b":2}"#);
        // JSON Formatter (80) 应排在 JSON↔YAML (55) 之前。
        assert_eq!(results[0].tool_id, "formatters.json");
        assert!(results.iter().any(|r| r.tool_id == "converters.json_yaml"));
    }

    #[test]
    fn detect_timestamp() {
        let reg = build_registry();
        let results = reg.detect("1609459200");
        assert!(results.iter().any(|r| r.tool_id == "converters.timestamp"));
    }

    #[test]
    fn detect_empty_is_empty() {
        let reg = build_registry();
        assert!(reg.detect("   ").is_empty());
    }

    #[test]
    fn detect_hex_color() {
        let reg = build_registry();
        let results = reg.detect("#1E90FF");
        assert!(results.iter().any(|r| r.tool_id == "converters.color"));
    }

    #[test]
    fn detect_results_sorted_desc() {
        let reg = build_registry();
        let results = reg.detect(r#"{"x":1}"#);
        for w in results.windows(2) {
            assert!(w[0].confidence >= w[1].confidence);
        }
    }
}
