//! Global mouse position tracking service.
//!
//! This module provides a lightweight service for tracking global mouse position
//! across the entire screen (not just within the WebView window).

use crossbeam_channel::{bounded, Receiver, Sender, TryRecvError};
use rdev::{listen, Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{error, info};

/// Current mouse position.
#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

/// Events emitted by the mouse tracker.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "type")]
pub enum MouseTrackerEvent {
    /// Mouse position update (throttled).
    PositionUpdate { x: i32, y: i32 },
}

/// Commands for the mouse tracker.
#[derive(Debug)]
pub enum MouseTrackerCommand {
    /// Stop the tracker.
    Stop,
}

/// Handle to control the mouse tracker.
pub struct MouseTrackerHandle {
    event_rx: Receiver<MouseTrackerEvent>,
    cmd_tx: Sender<MouseTrackerCommand>,
    running: Arc<AtomicBool>,
    /// Kept for potential future graceful shutdown.
    /// Currently unused because rdev::listen blocks and cannot be interrupted gracefully.
    #[allow(dead_code)]
    thread: Option<JoinHandle<()>>,
}

impl MouseTrackerHandle {
    /// Try to receive an event (non-blocking).
    pub fn try_recv(&self) -> Option<MouseTrackerEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Drain all pending events.
    pub fn drain(&self) -> Vec<MouseTrackerEvent> {
        let mut events = Vec::new();
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => events.push(event),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        events
    }

    /// Stop the tracker.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = self.cmd_tx.send(MouseTrackerCommand::Stop);
    }

    /// Check if the tracker is still running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
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
/// This spawns a thread that uses rdev to listen for global mouse events.
/// Position updates are throttled to reduce IPC overhead.
pub fn start_mouse_tracker(config: MouseTrackerConfig) -> MouseTrackerHandle {
    let (event_tx, event_rx) = bounded(64);
    let (cmd_tx, cmd_rx) = bounded(16);
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    let thread = thread::spawn(move || {
        info!("Mouse tracker thread started");

        // Shared state for the callback
        let mut last_emit_time = Instant::now();
        let throttle_duration = Duration::from_millis(config.throttle_ms);

        // Clone channels for the callback
        let event_tx_clone = event_tx.clone();

        let callback = move |event: Event| {
            // Check for stop command
            if let Ok(MouseTrackerCommand::Stop) = cmd_rx.try_recv() {
                info!("Tracker stop requested");
                return;
            }

            if let EventType::MouseMove { x, y } = event.event_type {
                // Throttle position updates
                let now = Instant::now();
                if now.duration_since(last_emit_time) >= throttle_duration {
                    last_emit_time = now;
                    let _ = event_tx_clone.try_send(MouseTrackerEvent::PositionUpdate {
                        x: x as i32,
                        y: y as i32,
                    });
                }
            }
        };

        if let Err(error) = listen(callback) {
            error!(?error, "Mouse tracker error");
        }

        running_clone.store(false, Ordering::SeqCst);
        info!("Mouse tracker thread exiting");
    });

    MouseTrackerHandle {
        event_rx,
        cmd_tx,
        running,
        thread: Some(thread),
    }
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
