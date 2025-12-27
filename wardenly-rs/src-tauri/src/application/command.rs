use serde::{Deserialize, Serialize};

/// Commands that can be sent to a SessionActor
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SessionCommand {
    /// Start the session (launch browser, navigate, begin login)
    Start,

    /// Stop the session and cleanup resources
    Stop,

    /// Click at coordinates
    Click { x: f64, y: f64 },

    /// Drag from one point to another
    Drag { from: (f64, f64), to: (f64, f64) },

    /// Start screencast streaming
    StartScreencast,

    /// Stop screencast streaming
    StopScreencast,
}

/// Commands that can be sent to the Coordinator
#[derive(Debug, Clone)]
pub enum CoordinatorCommand {
    /// Create a new session for an account
    CreateSession { account_id: String },

    /// Stop a specific session
    StopSession { session_id: String },

    /// Stop all sessions
    StopAll,

    /// Send a command to a specific session
    SessionCommand {
        session_id: String,
        command: SessionCommand,
    },

    /// Click on all active sessions
    ClickAll { x: f64, y: f64 },

    /// Drag on all active sessions  
    DragAll { from: (f64, f64), to: (f64, f64) },
}

