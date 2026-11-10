//! The canonical document: edits, persistence, metadata, templates and native file import / export.

use std::sync::Mutex;

use tap_application::MacroLibrary;
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
    library: State<'_, MacroLibrary>,
    name: Option<String>,
) -> Result<String, String> {
    let mut app_state = state.lock().unwrap();

    if let Some(n) = name {
        app_state.coordinator.session_mut().set_name(n);
    }

    let document = app_state.coordinator.document();
    let path = library.save(document).map_err(|e| e.to_string())?;
    library.remember(&document.name);

    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn cmd_load_profile(
    state: State<'_, Mutex<AppState>>,
    library: State<'_, MacroLibrary>,
    name: String,
) -> Result<MacroDocument, String> {
    let document = library.load(&name).map_err(|e| e.to_string())?;

    let mut app_state = state.lock().unwrap();
    app_state.coordinator.set_document(document.clone());
    library.remember(&name);

    Ok(document)
}

#[tauri::command]
pub fn cmd_delete_profile(library: State<'_, MacroLibrary>, name: String) -> Result<(), String> {
    library.delete(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_list_profiles(library: State<'_, MacroLibrary>) -> Result<Vec<String>, String> {
    library.list().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn cmd_get_last_used(library: State<'_, MacroLibrary>) -> Option<String> {
    library.last_used()
}

#[tauri::command]
pub fn cmd_get_recent_profiles(library: State<'_, MacroLibrary>) -> Vec<String> {
    library.recent()
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
