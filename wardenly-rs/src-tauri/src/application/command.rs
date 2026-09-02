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

    /// Stop the currently running script.
    /// If run_id is provided, only stop if it matches the current script's run_id.
    /// This prevents stale stop events from terminating newly started scripts.
    StopScript { run_id: Option<String> },

    /// Refresh/reload the current page
    Refresh,

    /// Capture a single screenshot (for manual refresh when screencast is off)
    CaptureScreenshot,

    /// Insert text into the currently focused element via CDP Input.insertText
    InsertText { text: String },

    /// Send a protocol message to the game via the page bridge
    /// (name per the game's own registry, e.g. "C_2_S_MAIL_INFO")
    SendProtocol {
        name: String,
        payload: serde_json::Value,
    },
}
