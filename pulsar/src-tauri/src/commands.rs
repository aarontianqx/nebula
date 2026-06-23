//! Tauri IPC 命令（Adapter 层，薄）。
//!
//! 前端通过这些命令访问注册表与工具。所有业务逻辑都在 `pulsar-app` / `pulsar-core`。

use pulsar_app::{DetectionResult, ToolRegistry};
use pulsar_core::{ParamValue, ToolDescriptor, ToolParams, ToolValue};
use serde::Deserialize;
use std::collections::BTreeMap;
use tauri::State;

/// 全局应用状态：持有注册表。
pub struct AppState {
    pub registry: ToolRegistry,
}

/// 前端传入的参数（JSON 值，按工具 descriptor 解释）。
type RawParams = BTreeMap<String, serde_json::Value>;

#[derive(Debug, Deserialize)]
pub struct RunRequest {
    pub id: String,
    /// 文本输入（scaffold 阶段仅支持文本工具）。
    pub input: String,
    #[serde(default)]
    pub params: RawParams,
}

/// 列出全部工具的描述符。
#[tauri::command]
pub fn list_tools(state: State<'_, AppState>) -> Vec<ToolDescriptor> {
    state.registry.descriptors()
}

/// 运行指定工具，返回文本结果。
#[tauri::command]
pub fn run_tool(state: State<'_, AppState>, request: RunRequest) -> Result<String, String> {
    let params = convert_params(request.params);
    let output = state
        .registry
        .run(&request.id, ToolValue::text(request.input), &params)
        .map_err(|e| e.to_string())?;
    Ok(output.as_text().into_owned())
}

/// 关键词搜索工具（Command Palette）。
#[tauri::command]
pub fn search_tools(state: State<'_, AppState>, query: String) -> Vec<ToolDescriptor> {
    state.registry.search(&query)
}

/// Smart Detection：识别输入内容，返回候选工具（按置信度降序）。
#[tauri::command]
pub fn detect(state: State<'_, AppState>, input: String) -> Vec<DetectionResult> {
    state.registry.detect(&input)
}

/// 把前端 JSON 参数转换为 `ToolParams`。
fn convert_params(raw: RawParams) -> ToolParams {
    let mut params = ToolParams::new();
    for (key, value) in raw {
        let pv = match value {
            serde_json::Value::Bool(b) => ParamValue::Bool(b),
            serde_json::Value::Number(n) if n.is_i64() => {
                ParamValue::Int(n.as_i64().unwrap_or_default())
            }
            serde_json::Value::Number(n) => ParamValue::Str(n.to_string()),
            serde_json::Value::String(s) => ParamValue::Str(s),
            other => ParamValue::Str(other.to_string()),
        };
        params.insert(key, pv);
    }
    params
}
