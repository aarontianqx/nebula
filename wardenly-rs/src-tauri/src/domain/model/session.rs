use serde::{Deserialize, Serialize};

/// Session states representing the lifecycle of a browser session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SessionState {
    #[default]
    Idle,
    Starting,
    LoggingIn,
    Ready,
    ScriptRunning,
    Stopped,
}

impl SessionState {
    /// Check if the session can accept click/drag interactions
    pub fn can_accept_interaction(&self) -> bool {
        matches!(self, Self::LoggingIn | Self::Ready | Self::ScriptRunning)
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
