//! Application state for Tauri backend.

use tap_core::{EngineState, PlayerHandle, Profile};

/// Global application state.
pub struct AppState {
    pub engine_state: EngineState,
    pub profile: Profile,
    pub player_handle: Option<PlayerHandle>,
    pub executed_count: u64,
    pub current_action_index: Option<usize>,
}
