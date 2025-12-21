//! Application state for Tauri backend.

use tap_core::{EngineState, PlayerHandle, Profile, Recorder, RecorderState};
use tap_platform::{InputHookHandle, MouseTrackerHandle};

/// Global application state.
pub struct AppState {
    // Playback state
    pub engine_state: EngineState,
    pub profile: Profile,
    pub player_handle: Option<PlayerHandle>,
    pub executed_count: u64,
    pub current_action_index: Option<usize>,
    
    // Recording state
    pub recorder: Recorder,
    pub input_hook: Option<InputHookHandle>,
    
    // Global mouse tracking
    pub mouse_tracker: Option<MouseTrackerHandle>,
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
