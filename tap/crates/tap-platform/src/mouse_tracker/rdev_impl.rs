//! rdev-based implementation for Windows/Linux mouse tracking.

use super::{MouseTrackerCommand, MouseTrackerConfig, MouseTrackerEvent};
use crossbeam_channel::{Receiver, Sender};
use rdev::{listen, Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info};

/// Start the mouse tracker using rdev.
pub fn start_tracker(
    config: MouseTrackerConfig,
    event_tx: Sender<MouseTrackerEvent>,
    cmd_rx: Receiver<MouseTrackerCommand>,
    running: Arc<AtomicBool>,
) {
    info!("Mouse tracker thread started (rdev)");

    let mut last_emit_time = Instant::now();
    let throttle_duration = Duration::from_millis(config.throttle_ms);

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
                let _ = event_tx.try_send(MouseTrackerEvent::PositionUpdate {
                    x: x as i32,
                    y: y as i32,
                });
            }
        }
    };

    if let Err(error) = listen(callback) {
        error!(?error, "Mouse tracker error");
    }

    running.store(false, Ordering::SeqCst);
    info!("Mouse tracker thread exiting");
}

