//! Application state for Tauri backend.

use crate::key_click::KeyClickHandle;
use tap_application::{Coordinator, Recorder, RecorderState};
use tap_platform::{InputHookHandle, MouseTrackerHandle};

/// Global application state.
///
/// The adapter is intentionally thin: the engine state machine and the canonical
/// macro document live in the application-layer [`Coordinator`]. `AppState` only
/// holds the Tauri-side bits the event pump touches directly (recorder, input
/// hook, mouse tracker, key-click handle).
pub struct AppState {
    /// Application-layer entry point: owns the player + canonical document.
    pub coordinator: Coordinator,
    pub executed_count: u64,
    pub current_action_index: Option<usize>,

    // Recording state
    pub recorder: Recorder,
    pub input_hook: Option<InputHookHandle>,

    // Global mouse tracking
    pub mouse_tracker: Option<MouseTrackerHandle>,

    // Key-to-Click tool mode
    pub key_click_handle: Option<KeyClickHandle>,
}

/// Recording status for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct RecordingStatus {
    pub state: RecorderState,
    pub event_count: usize,
    pub duration_ms: u64,
}

/// Mouse position update for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct MousePositionUpdate {
    pub x: i32,
    pub y: i32,
}

/// Position picked event for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct PositionPickedEvent {
    pub x: i32,
    pub y: i32,
}
