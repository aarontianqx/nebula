//! Engine lifecycle commands: start / pause / resume / stop / emergency stop, plus the dry-run toggle.

use std::sync::atomic::Ordering;
use std::sync::Mutex;

use tap_application::EngineState;
use tauri::State;
use tracing::{info, warn};

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
    let app_state = state.lock().unwrap();

    app_state.coordinator.emergency_stop();
    warn!("Emergency stop triggered!");

    // Also stop key-click mode if running (just signal, don't take)
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested by emergency stop");
    }

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
