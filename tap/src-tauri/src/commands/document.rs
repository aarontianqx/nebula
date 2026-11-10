//! The canonical document: edits, persistence, metadata, templates and native file import / export.

use std::sync::Mutex;

use tap_application::{
    delete_profile, list_profiles, load_document, load_last_used, load_recent, save_document,
    save_last_used, save_recent,
};
use tap_core::{document_to_yaml, parse_yaml, MacroDocument};
use tauri::State;

use super::yaml::{ensure_valid, ValidationErrorResponse};
use crate::state::AppState;
use crate::templates;

/// The canonical document, exactly as the engine will run it.
#[tauri::command]
pub fn get_document(state: State<'_, Mutex<AppState>>) -> MacroDocument {
    state.lock().unwrap().coordinator.document().clone()
}

/// Replace the canonical document with an edit from the UI.
///
/// Never rejected: the UI passes through states that are not runnable yet. The findings come back
/// as warnings for the UI to show inline.
#[tauri::command]
pub fn update_document(
    state: State<'_, Mutex<AppState>>,
    document: MacroDocument,
) -> Vec<ValidationErrorResponse> {
    state
        .lock()
        .unwrap()
        .coordinator
        .update_document(document)
        .into_iter()
        .map(ValidationErrorResponse::from)
        .collect()
}

#[tauri::command]
pub fn cmd_save_profile(
    state: State<'_, Mutex<AppState>>,
    name: Option<String>,
) -> Result<String, String> {
    let mut app_state = state.lock().unwrap();

    if let Some(n) = name {
        app_state.coordinator.session_mut().set_name(n);
    }

    // Persist the lossless canonical document (variables + metadata included).
    let document = app_state.coordinator.document().clone();
    let path = save_document(&document).map_err(|e| e.to_string())?;
    let _ = save_last_used(&document.name);
    let _ = save_recent(&document.name);

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn cmd_load_profile(
    state: State<'_, Mutex<AppState>>,
    name: String,
) -> Result<MacroDocument, String> {
    let document = load_document(&name).map_err(|e| e.to_string())?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document.clone());
    let _ = save_last_used(&name);
    let _ = save_recent(&name);

    Ok(document)
}

#[tauri::command]
pub fn cmd_delete_profile(name: String) -> Result<(), String> {
    delete_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_profiles() -> Result<Vec<String>, String> {
    list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_last_used() -> Option<String> {
    load_last_used()
}

#[tauri::command]
pub fn cmd_get_recent_profiles() -> Vec<String> {
    load_recent()
}

#[tauri::command]
pub fn cmd_list_templates() -> Vec<templates::TemplateInfo> {
    templates::list_templates()
}

#[tauri::command]
pub fn cmd_apply_template(
    state: State<'_, Mutex<AppState>>,
    id: String,
) -> Result<MacroDocument, String> {
    let yaml = templates::template_yaml(&id).ok_or_else(|| format!("Unknown template: {id}"))?;
    let document = parse_yaml(yaml).map_err(|e| e.to_string())?;
    ensure_valid(&document)?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document.clone());
    Ok(document)
}

#[tauri::command]
pub fn cmd_export_yaml_to_path(
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<(), String> {
    let yaml = {
        let app_state = state.lock().unwrap();
        document_to_yaml(app_state.coordinator.document()).map_err(|e| e.to_string())?
    };
    std::fs::write(&path, yaml).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_import_yaml_from_path(
    state: State<'_, Mutex<AppState>>,
    path: String,
) -> Result<MacroDocument, String> {
    let yaml = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let document = parse_yaml(&yaml).map_err(|e| e.to_string())?;
    ensure_valid(&document)?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document.clone());
    Ok(document)
}
