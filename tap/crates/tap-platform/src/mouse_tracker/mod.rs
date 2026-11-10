//! Global mouse position tracking service.
//!
//! This module provides a lightweight service for tracking global mouse position
//! across the entire screen (not just within the WebView window).
//!
//! Platform implementations:
//! - Windows/Linux: Uses rdev crate (`rdev_impl.rs`)
//! - macOS: Uses native Core Graphics API (`macos.rs`)

use crossbeam_channel::{bounded, Receiver};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

#[cfg(not(target_os = "macos"))]
mod rdev_impl;

#[cfg(target_os = "macos")]
mod macos;

/// Events emitted by the mouse tracker.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum MouseTrackerEvent {
    /// Mouse position update (throttled).
    PositionUpdate { x: i32, y: i32 },
}

/// Handle to the mouse tracker.
///
/// The worker thread is detached and ends on its own once `running` is cleared.
pub struct MouseTrackerHandle {
    event_rx: Receiver<MouseTrackerEvent>,
    running: Arc<AtomicBool>,
}

impl MouseTrackerHandle {
    /// The tracker's events; iterating blocks until the next one.
    pub fn events(&self) -> &Receiver<MouseTrackerEvent> {
        &self.event_rx
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

impl Drop for MouseTrackerHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Configuration for the mouse tracker.
#[derive(Debug, Clone)]
pub struct MouseTrackerConfig {
    /// Throttle interval for position updates (milliseconds).
    /// Set to 0 to disable throttling.
    pub throttle_ms: u64,
}

impl Default for MouseTrackerConfig {
    fn default() -> Self {
        Self {
            throttle_ms: 50, // ~20 FPS for smooth display
        }
    }
}

/// Start the global mouse position tracker.
///
/// This spawns a thread that listens for global mouse events.
/// Position updates are throttled to reduce IPC overhead.
///
/// On Windows/Linux: Uses rdev.
/// On macOS: Uses native Core Graphics API to avoid thread-safety issues.
pub fn start_mouse_tracker(config: MouseTrackerConfig) -> MouseTrackerHandle {
    let (event_tx, event_rx) = bounded(64);
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    #[cfg(not(target_os = "macos"))]
    thread::spawn(move || {
        rdev_impl::start_tracker(config, event_tx, running_clone);
    });

    #[cfg(target_os = "macos")]
    thread::spawn(move || {
        macos::start_tracker(config, event_tx, running_clone);
    });

    MouseTrackerHandle { event_rx, running }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = MouseTrackerConfig::default();
        assert_eq!(config.throttle_ms, 50);
    }
}
