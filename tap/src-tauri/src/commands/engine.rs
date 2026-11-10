//! Engine lifecycle commands: start / pause / resume / stop / emergency stop, plus the dry-run toggle.

use tap_application::{simple_repeat_document, EngineState};
use tap_core::{DslAction, DslMouseButton, MouseButton};
use tauri::State;
use tracing::{info, warn};

use crate::app::stop_all;
use crate::key_click::KeyClickState;
use crate::state::{CoordinatorState, DryRun};

#[tauri::command]
pub fn get_state(coordinator: State<'_, CoordinatorState>) -> EngineState {
    coordinator.lock().unwrap().engine_state()
}

#[tauri::command]
pub fn start_execution(coordinator: State<'_, CoordinatorState>) -> Result<(), String> {
    coordinator.lock().unwrap().start()?;
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
    coordinator: State<'_, CoordinatorState>,
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
    coordinator.lock().unwrap().start_standalone(document)
}

#[tauri::command]
pub fn pause_execution(coordinator: State<'_, CoordinatorState>) -> Result<(), String> {
    coordinator.lock().unwrap().pause()
}

#[tauri::command]
pub fn resume_execution(coordinator: State<'_, CoordinatorState>) -> Result<(), String> {
    coordinator.lock().unwrap().resume()
}

#[tauri::command]
pub fn stop_execution(coordinator: State<'_, CoordinatorState>) {
    coordinator.lock().unwrap().stop();
}

#[tauri::command]
pub fn emergency_stop(
    coordinator: State<'_, CoordinatorState>,
    key_click: State<'_, KeyClickState>,
) {
    warn!("Emergency stop triggered!");
    stop_all(&coordinator, &key_click);
}

#[tauri::command]
pub fn cmd_set_dry_run(dry_run: State<'_, DryRun>, enabled: bool) {
    dry_run.set(enabled);
    info!(enabled, "Dry-run toggled");
}

#[tauri::command]
pub fn cmd_get_dry_run(dry_run: State<'_, DryRun>) -> bool {
    dry_run.get()
}
