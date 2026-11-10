//! Recording commands: start / pause / resume / stop and status.

use std::sync::Mutex;

use tap_application::{window_binding, EngineState, RecorderState};
use tap_core::Timeline;
use tap_platform::start_input_hook;
use tauri::State;
use tracing::info;

use crate::state::{AppState, RecordingStatus};

#[tauri::command]
pub fn start_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Idle {
        return Err("Recording already in progress".into());
    }

    if app_state.coordinator.engine_state() != EngineState::Idle {
        return Err("Cannot record while playing".into());
    }

    // A window-relative document needs the window on screen to measure positions from.
    if let Some(target) = window_binding(app_state.coordinator.document()) {
        if tap_platform::window_origin(target.title.as_deref(), target.process.as_deref()).is_none()
        {
            return Err(
                "The target window was not found. Bring it on screen to record positions \
                 relative to it."
                    .into(),
            );
        }
    }

    // Start the input hook
    let input_hook = start_input_hook();
    app_state.input_hook = Some(input_hook);

    // Start the recorder
    app_state.recorder.start();

    info!("Recording started");
    Ok(())
}

#[tauri::command]
pub fn pause_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Recording {
        return Err("Not recording".into());
    }

    app_state.recorder.pause();
    info!("Recording paused");
    Ok(())
}

#[tauri::command]
pub fn resume_recording(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() != RecorderState::Paused {
        return Err("Not paused".into());
    }

    app_state.recorder.resume();
    info!("Recording resumed");
    Ok(())
}

#[tauri::command]
pub fn stop_recording(state: State<'_, Mutex<AppState>>) -> Result<Timeline, String> {
    let mut app_state = state.lock().unwrap();

    if app_state.recorder.state() == RecorderState::Idle {
        return Err("Not recording".into());
    }

    // Stop the input hook
    if let Some(hook) = app_state.input_hook.take() {
        hook.stop();
    }

    // Stop the recorder and get the timeline
    let event = app_state.recorder.stop();
    let timeline = match event {
        Some(tap_application::RecorderEvent::RecordingCompleted { timeline }) => timeline,
        _ => Timeline { actions: vec![] },
    };

    info!(
        "Recording stopped, {} actions captured",
        timeline.actions.len()
    );

    // Update the canonical document with the recorded timeline.
    app_state
        .coordinator
        .session_mut()
        .set_recorded_timeline(timeline.clone());
    app_state
        .coordinator
        .session_mut()
        .set_name("Recorded".into());

    Ok(timeline)
}

#[tauri::command]
pub fn get_recording_status(state: State<'_, Mutex<AppState>>) -> RecordingStatus {
    let app_state = state.lock().unwrap();
    RecordingStatus {
        state: app_state.recorder.state(),
        event_count: app_state.recorder.event_count(),
        duration_ms: app_state.recorder.duration_ms(),
    }
}
