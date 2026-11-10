//! Engine lifecycle commands: start / pause / resume / stop / emergency stop, plus the dry-run toggle.

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use tap_application::{simple_repeat_document, EngineState};
use tap_core::{DslAction, DslMouseButton, MouseButton};
use tauri::State;
use tracing::{info, warn};

use crate::app::stop_all;
use crate::state::AppState;

#[tauri::command]
pub fn get_state(state: State<'_, Mutex<AppState>>) -> EngineState {
    state.lock().unwrap().coordinator.engine_state()
}

#[tauri::command]
pub fn start_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();
    app_state.coordinator.start()?;
    info!("Sent start command to player");
    Ok(())
}

/// The one action the simple-repeat tool performs.
#[derive(Debug, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimpleAction {
    Click {
        x: i32,
        y: i32,
        button: DslMouseButton,
    },
    Key {
        key: String,
    },
}

/// Repeat one action at a fixed interval (`repeat_count: None` repeats forever).
///
/// Runs a throwaway macro: the document in the editor is not replaced.
#[tauri::command]
pub fn start_simple_repeat(
    state: State<'_, Mutex<AppState>>,
    action: SimpleAction,
    interval_ms: u64,
    repeat_count: Option<u32>,
    countdown_secs: u32,
) -> Result<(), String> {
    let action = match action {
        SimpleAction::Click { x, y, button } => DslAction::click(x, y, MouseButton::from(button)),
        SimpleAction::Key { key } => DslAction::KeyTap { key },
    };
    let document = simple_repeat_document(
        action,
        interval_ms,
        repeat_count,
        u64::from(countdown_secs) * 1000,
    );
    state.lock().unwrap().coordinator.start_standalone(document)
}

#[tauri::command]
pub fn pause_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.pause()
}

#[tauri::command]
pub fn resume_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.resume()
}

#[tauri::command]
pub fn stop_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    state.lock().unwrap().coordinator.stop();
    Ok(())
}

#[tauri::command]
pub fn emergency_stop(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    warn!("Emergency stop triggered!");
    stop_all(&state.lock().unwrap());
    Ok(())
}

#[tauri::command]
pub fn cmd_set_dry_run(state: State<'_, Mutex<AppState>>, enabled: bool) {
    state
        .lock()
        .unwrap()
        .dry_run
        .store(enabled, Ordering::SeqCst);
    info!(enabled, "Dry-run toggled");
}

#[tauri::command]
pub fn cmd_get_dry_run(state: State<'_, Mutex<AppState>>) -> bool {
    state.lock().unwrap().dry_run.load(Ordering::SeqCst)
}
