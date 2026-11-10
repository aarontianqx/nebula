//! Application state for Tauri backend.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use crate::commands::system::ColorResponse;
use crate::key_click::KeyClickHandle;
use tap_application::{Coordinator, Recorder, RecorderState};
use tap_platform::{EnigoInjector, InputHookHandle, MouseTrackerHandle};

/// Global application state.
///
/// The adapter is intentionally thin: the engine state machine and the canonical
/// macro document live in the application-layer [`Coordinator`]. `AppState` only
/// holds the Tauri-side bits the event pump touches directly (recorder, input
/// hook, mouse tracker, key-click handle) and the few resources shared between
/// commands and background threads.
pub struct AppState {
    /// Application-layer entry point: owns the player + canonical document.
    pub coordinator: Coordinator,

    /// Dry-run switch shared with the engine's executor and the key-click runner:
    /// when set, actions are logged instead of injected.
    pub dry_run: Arc<AtomicBool>,

    // Recording state
    pub recorder: Recorder,
    pub input_hook: Option<InputHookHandle>,

    // Global mouse tracking
    pub mouse_tracker: Option<MouseTrackerHandle>,

    // Key-to-Click tool mode
    pub key_click_handle: Option<KeyClickHandle>,
    /// Injector shared by key-click sessions, created on first use.
    pub key_click_injector: Option<Arc<EnigoInjector>>,
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
    /// The color under the click, only when the picker was opened to read one. `None` there means
    /// the color could not be read (for example, missing Screen Recording permission).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<ColorResponse>,
}
