use super::model::SessionState;
use serde::{Deserialize, Serialize};

/// Domain events that represent state changes in the system
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DomainEvent {
    /// A new session was created
    SessionCreated {
        session_id: String,
        account_id: String,
        display_name: String,
    },

    /// Session state changed
    SessionStateChanged {
        session_id: String,
        old_state: SessionState,
        new_state: SessionState,
    },

    /// A screencast frame was received
    ScreencastFrame {
        session_id: String,
        image_base64: String,
        timestamp: u64,
    },

    /// Session was stopped
    SessionStopped { session_id: String },

    /// Login succeeded
    LoginSucceeded { session_id: String },

    /// Login failed
    LoginFailed { session_id: String, reason: String },

    /// Script started
    ScriptStarted {
        session_id: String,
        script_name: String,
        /// Unique identifier for this script run instance
        run_id: String,
    },

    /// Script stopped
    ScriptStopped {
        session_id: String,
        script_name: String,
        /// Unique identifier for this script run instance
        run_id: String,
    },

    /// Script step executed
    ScriptStepExecuted {
        session_id: String,
        step_index: usize,
        scene_name: String,
    },
}

impl DomainEvent {
    /// Get the event type name for frontend routing
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "session_created",
            Self::SessionStateChanged { .. } => "session_state_changed",
            Self::ScreencastFrame { .. } => "screencast_frame",
            Self::SessionStopped { .. } => "session_stopped",
            Self::LoginSucceeded { .. } => "login_succeeded",
            Self::LoginFailed { .. } => "login_failed",
            Self::ScriptStarted { .. } => "script_started",
            Self::ScriptStopped { .. } => "script_stopped",
            Self::ScriptStepExecuted { .. } => "script_step_executed",
        }
    }
}
