use serde::{Deserialize, Serialize};

/// Session states representing the lifecycle of a browser session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Idle,
    Starting,
    LoggingIn,
    Ready,
    ScriptRunning,
    Stopped,
}

impl SessionState {
    /// Check if transition to target state is valid
    #[allow(dead_code)]
    pub fn can_transition_to(&self, target: SessionState) -> bool {
        matches!(
            (self, target),
            (Self::Idle, Self::Starting)
                | (Self::Starting, Self::LoggingIn | Self::Stopped)
                | (Self::LoggingIn, Self::Ready | Self::Stopped)
                | (Self::Ready, Self::ScriptRunning | Self::Stopped)
                | (Self::ScriptRunning, Self::Ready | Self::Stopped)
        )
    }

    /// Check if the session can accept click/drag interactions
    pub fn can_accept_interaction(&self) -> bool {
        matches!(self, Self::LoggingIn | Self::Ready | Self::ScriptRunning)
    }

    /// Check if session is in a running state
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        !matches!(self, Self::Idle | Self::Stopped)
    }
}

impl Default for SessionState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Information about a session for UI display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub account_id: String,
    pub display_name: String,
    pub state: SessionState,
}

