//! YAML / DSL commands: export, import, validation, and macro variables.

use tap_core::{
    document_to_yaml, parse_yaml, validate_document, MacroDocument, ValidationError, VariableValue,
};
use tauri::State;

use crate::state::CoordinatorState;

#[tauri::command]
pub fn cmd_export_yaml(coordinator: State<'_, CoordinatorState>) -> Result<String, String> {
    document_to_yaml(coordinator.lock().unwrap().document()).map_err(|e| e.to_string())
}

/// Validate a document about to replace the current one, flattening the findings into one message.
///
/// Used by the import paths, which (unlike live editing) refuse documents that would not run.
pub(super) fn ensure_valid(document: &MacroDocument) -> Result<(), String> {
    validate_document(document).map_err(|errors| {
        errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

#[tauri::command]
pub fn cmd_import_yaml(
    coordinator: State<'_, CoordinatorState>,
    yaml_content: String,
) -> Result<MacroDocument, String> {
    let document = parse_yaml(&yaml_content).map_err(|e| e.to_string())?;
    ensure_valid(&document)?;
    coordinator.lock().unwrap().set_document(document.clone());
    Ok(document)
}

#[tauri::command]
pub fn cmd_validate_yaml(yaml_content: String) -> Result<(), Vec<ValidationError>> {
    let document = parse_yaml(&yaml_content).map_err(|e| {
        vec![ValidationError {
            path: String::new(),
            message: e.to_string(),
            line: None,
        }]
    })?;

    validate_document(&document)
}

#[tauri::command]
pub fn cmd_set_runtime_variables(
    coordinator: State<'_, CoordinatorState>,
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

    coordinator.lock().unwrap().set_runtime_vars(overrides);

    Ok(())
}
