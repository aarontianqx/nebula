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

    /// Start executing a script
    StartScript { script_name: String },

    /// Stop the currently running script
    StopScript,

    /// Refresh/reload the current page
    Refresh,
}

