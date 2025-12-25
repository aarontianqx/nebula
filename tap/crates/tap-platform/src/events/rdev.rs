//! Windows/Linux global event listening using rdev.
//!
//! This module provides a simple wrapper around rdev for non-macOS platforms.
//! Unlike macOS, we don't use a singleton pattern here - each caller gets its own
//! rdev::listen() instance. This is simpler and avoids initialization timing issues.

// This module is intentionally minimal on Windows/Linux.
// The input_hook and mouse_tracker modules use rdev directly.

use std::time::Duration;

/// Placeholder types for API compatibility with macOS.
/// On Windows/Linux, we don't actually use these - the input_hook and mouse_tracker
/// modules call rdev::listen() directly.

#[derive(Debug, Clone)]
pub enum RdevEventType {
    MouseMove { x: f64, y: f64 },
    MouseDown { x: f64, y: f64, button: u8 },
    MouseUp { x: f64, y: f64, button: u8 },
    Scroll { delta_x: i64, delta_y: i64 },
    KeyDown { key: String },
    KeyUp { key: String },
}

#[derive(Debug, Clone)]
pub struct RdevEvent {
    pub event_type: RdevEventType,
    pub timestamp_ms: u64,
}

/// Subscription handle (not used on Windows/Linux, kept for API compatibility).
pub struct RdevEventSubscription {
    _private: (),
}

impl RdevEventSubscription {
    pub fn recv_timeout(
        &self,
        _timeout: Duration,
    ) -> Result<RdevEvent, crossbeam_channel::RecvTimeoutError> {
        Err(crossbeam_channel::RecvTimeoutError::Disconnected)
    }

    pub fn try_recv(&self) -> Option<RdevEvent> {
        None
    }

    pub fn drain(&self) -> Vec<RdevEvent> {
        Vec::new()
    }
}

/// This function is not used on Windows/Linux.
/// The input_hook module uses rdev::listen() directly.
pub fn subscribe_events() -> RdevEventSubscription {
    RdevEventSubscription { _private: () }
}
