//! State shared between commands and background threads. Tauri manages each piece on its own,
//! so a command only waits for the state it actually uses.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tap_application::{Coordinator, EngineState, RecorderState};

use crate::key_click::KeyClickState;
use crate::recording::RecordingState;

pub type CoordinatorState = Mutex<Coordinator>;

/// While on, actions are logged instead of injected.
#[derive(Clone, Default)]
pub struct DryRun(Arc<AtomicBool>);

impl DryRun {
    pub fn get(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }

    pub fn set(&self, enabled: bool) {
        self.0.store(enabled, Ordering::SeqCst);
    }
}

/// Playback, recording, Key->Click and key capture all take over the keyboard and mouse, so only
/// one may run at a time. `what` names the one being started, for the error message.
pub fn ensure_idle(
    what: &str,
    coordinator: &CoordinatorState,
    recording: &RecordingState,
    key_click: &KeyClickState,
) -> Result<(), String> {
    if coordinator.lock().unwrap().engine_state() != EngineState::Idle {
        return Err(format!("Cannot {what}: a macro is running"));
    }
    if recording.state() != RecorderState::Idle {
        return Err(format!("Cannot {what}: a recording is in progress"));
    }
    if key_click.is_running() {
        return Err(format!("Cannot {what}: Key->Click is running"));
    }
    Ok(())
}
