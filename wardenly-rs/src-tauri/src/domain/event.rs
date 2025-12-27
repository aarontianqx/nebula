use serde::{Deserialize, Serialize};
use super::model::SessionState;

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
}

impl DomainEvent {
    /// Get the session ID associated with this event
    #[allow(dead_code)]
    pub fn session_id(&self) -> &str {
        match self {
            Self::SessionCreated { session_id, .. } => session_id,
            Self::SessionStateChanged { session_id, .. } => session_id,
            Self::ScreencastFrame { session_id, .. } => session_id,
            Self::SessionStopped { session_id } => session_id,
            Self::LoginSucceeded { session_id } => session_id,
            Self::LoginFailed { session_id, .. } => session_id,
        }
    }

    /// Get the event type name for frontend routing
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::SessionCreated { .. } => "session_created",
            Self::SessionStateChanged { .. } => "session_state_changed",
            Self::ScreencastFrame { .. } => "screencast_frame",
            Self::SessionStopped { .. } => "session_stopped",
            Self::LoginSucceeded { .. } => "login_succeeded",
            Self::LoginFailed { .. } => "login_failed",
        }
    }
}

