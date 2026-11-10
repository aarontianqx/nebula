//! Recording commands: start / pause / resume / stop and status.

use tap_application::RecordingStatus;
use tap_core::MacroDocument;
use tauri::{AppHandle, State};
use tracing::info;

use crate::key_click::KeyClickState;
use crate::recording::RecordingState;
use crate::state::{ensure_idle, CoordinatorState};

#[tauri::command]
pub fn start_recording(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
    recording: State<'_, RecordingState>,
    key_click: State<'_, KeyClickState>,
) -> Result<(), String> {
    ensure_idle("record", &coordinator, &recording, &key_click)?;
    let document = coordinator.lock().unwrap().document().clone();
    recording.start(&app, &document)?;
    info!("Recording started");
    Ok(())
}

#[tauri::command]
pub fn pause_recording(recording: State<'_, RecordingState>) -> Result<(), String> {
    recording.pause()
}

#[tauri::command]
pub fn resume_recording(recording: State<'_, RecordingState>) -> Result<(), String> {
    recording.resume()
}

#[tauri::command]
pub fn stop_recording(
    coordinator: State<'_, CoordinatorState>,
    recording: State<'_, RecordingState>,
) -> Result<MacroDocument, String> {
    let steps = recording.stop().ok_or("Not recording")?;
    info!("Recording stopped, {} steps captured", steps.len());

    let mut coordinator = coordinator.lock().unwrap();
    let session = coordinator.session_mut();
    session.set_recorded_steps(steps);
    session.set_name("Recorded".into());
    Ok(coordinator.document().clone())
}

#[tauri::command]
pub fn get_recording_status(recording: State<'_, RecordingState>) -> RecordingStatus {
    recording.status()
}
