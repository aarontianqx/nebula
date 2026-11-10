//! YAML / DSL commands: export, import, validation, and macro variables.

use std::sync::Mutex;

use tap_core::{
    document_to_yaml, parse_yaml, validate_profile, MacroDocument, ValidationError, VariableValue,
};
use tauri::State;

use crate::state::AppState;

/// Validation error for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationErrorResponse {
    pub path: String,
    pub message: String,
    pub line: Option<usize>,
}

impl From<ValidationError> for ValidationErrorResponse {
    fn from(e: ValidationError) -> Self {
        Self {
            path: e.path,
            message: e.message,
            line: e.line,
        }
    }
}

#[tauri::command]
pub fn cmd_export_yaml(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    // Serialize the lossless canonical document directly (keeps variables/metadata).
    document_to_yaml(app_state.coordinator.document()).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_export_yaml_with_metadata(
    state: State<'_, Mutex<AppState>>,
    description: Option<String>,
    author: Option<String>,
) -> Result<String, String> {
    let app_state = state.lock().unwrap();
    let mut document = app_state.coordinator.document().clone();
    if description.is_some() {
        document.description = description;
    }
    if author.is_some() {
        document.author = author;
    }
    document_to_yaml(&document).map_err(|e| e.to_string())
}

/// Validate a document about to replace the current one, flattening the findings into one message.
///
/// Used by the import paths, which (unlike live editing) refuse documents that would not run.
pub(super) fn ensure_valid(document: &MacroDocument) -> Result<(), String> {
    validate_profile(document).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

#[tauri::command]
pub fn cmd_import_yaml(
    state: State<'_, Mutex<AppState>>,
    yaml_content: String,
) -> Result<MacroDocument, String> {
    // Parse into the canonical document (lossless: keeps variables + metadata).
    let document = parse_yaml(&yaml_content).map_err(|e| e.to_string())?;
    ensure_valid(&document)?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document.clone());
    Ok(document)
}

#[tauri::command]
pub fn cmd_validate_yaml(yaml_content: String) -> Result<(), Vec<ValidationErrorResponse>> {
    let dsl_profile = parse_yaml(&yaml_content).map_err(|e| {
        vec![ValidationErrorResponse {
            path: "".to_string(),
            message: e.to_string(),
            line: None,
        }]
    })?;

    validate_profile(&dsl_profile).map_err(|errors| errors.into_iter().map(|e| e.into()).collect())
}

#[tauri::command]
pub fn cmd_set_runtime_variables(
    state: State<'_, Mutex<AppState>>,
    vars: std::collections::HashMap<String, serde_json::Value>,
) -> Result<(), String> {
    // Convert the JSON payload into typed run-time overrides applied on the next
    // run (these win over the document's variable defaults in the Resolve stage).
    let mut overrides = std::collections::HashMap::new();
    for (key, value) in vars {
        let typed = if let Some(s) = value.as_str() {
            VariableValue::String(s.to_string())
        } else if let Some(n) = value.as_f64() {
            VariableValue::Number(n)
        } else if let Some(b) = value.as_bool() {
            VariableValue::Boolean(b)
        } else {
            continue;
        };
        overrides.insert(key, typed);
    }

    state
        .lock()
        .unwrap()
        .coordinator
        .set_runtime_vars(overrides);

    Ok(())
}

#[tauri::command]
pub fn cmd_get_runtime_variables(
    state: State<'_, Mutex<AppState>>,
) -> std::collections::HashMap<String, serde_json::Value> {
    let app_state = state.lock().unwrap();
    let mut result = std::collections::HashMap::new();

    for (key, value) in app_state.coordinator.runtime_vars() {
        let json_val = match value {
            VariableValue::String(s) => serde_json::Value::String(s.clone()),
            VariableValue::Number(n) => serde_json::Value::Number(
                serde_json::Number::from_f64(*n).unwrap_or(serde_json::Number::from(0)),
            ),
            VariableValue::Boolean(b) => serde_json::Value::Bool(*b),
        };
        result.insert(key.clone(), json_val);
    }

    result
}
