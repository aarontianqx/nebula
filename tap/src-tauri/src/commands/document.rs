//! The canonical document: edits, persistence, metadata, templates and native file import / export.

use std::sync::Mutex;

use tap_application::{
    delete_profile, list_profiles, load_document, load_last_used, load_recent, save_document,
    save_last_used, save_recent, simple_repeat_document,
};
use tap_core::{document_to_yaml, parse_yaml, DslAction, MacroDocument};
use tauri::State;
use tracing::info;

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

/// Parse a frontend mouse-button string (`left`/`right`/`middle`), defaulting to Left.
pub fn parse_mouse_button(s: Option<&str>) -> tap_core::MouseButton {
    match s.map(|v| v.to_lowercase()).as_deref() {
        Some("right") => tap_core::MouseButton::Right,
        Some("middle") => tap_core::MouseButton::Middle,
        _ => tap_core::MouseButton::Left,
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // Tauri command: arguments mirror the UI form fields.
pub fn set_simple_repeat(
    state: State<'_, Mutex<AppState>>,
    action_type: String,
    x: Option<i32>,
    y: Option<i32>,
    key: Option<String>,
    button: Option<String>,
    interval_ms: u64,
    repeat_count: Option<u32>,
    countdown_secs: u32,
) -> Result<(), String> {
    let action = match action_type.as_str() {
        "click" => DslAction::click(
            x.unwrap_or(0),
            y.unwrap_or(0),
            parse_mouse_button(button.as_deref()),
        ),
        "key" => DslAction::KeyTap {
            key: key.unwrap_or_else(|| "Space".into()),
        },
        _ => return Err(format!("Unknown action type: {}", action_type)),
    };

    let document = simple_repeat_document(
        action,
        interval_ms,
        repeat_count,
        u64::from(countdown_secs) * 1000,
    );
    info!(?document, "Updated document for simple repeat");
    state.lock().unwrap().coordinator.set_document(document);

    Ok(())
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
