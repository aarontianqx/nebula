//! rdev-based implementation for Windows/Linux mouse tracking.
//!
//! Uses rdev::listen() directly for mouse move events.

use super::{MouseTrackerCommand, MouseTrackerConfig, MouseTrackerEvent};
use crossbeam_channel::{bounded, Receiver, Sender};
use rdev::{listen, Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// Start the mouse tracker using rdev.
pub fn start_tracker(
    config: MouseTrackerConfig,
    event_tx: Sender<MouseTrackerEvent>,
    cmd_rx: Receiver<MouseTrackerCommand>,
    running: Arc<AtomicBool>,
) {
    info!("Mouse tracker thread started (rdev direct)");

    let throttle_duration = Duration::from_millis(config.throttle_ms);

    // Channel to receive mouse positions from rdev thread
    let (pos_tx, pos_rx) = bounded::<(i32, i32)>(256);

    // Spawn rdev listener thread
    let pos_tx_clone = pos_tx.clone();
    thread::spawn(move || {
        debug!("Mouse tracker rdev::listen thread starting");
        let callback = move |event: Event| {
            if let EventType::MouseMove { x, y } = event.event_type {
                let _ = pos_tx_clone.try_send((x as i32, y as i32));
            }
        };

        if let Err(error) = listen(callback) {
            error!(?error, "Mouse tracker rdev listen error");
        }
        debug!("Mouse tracker rdev::listen thread exiting");
    });

    let mut last_emit_time = Instant::now();

    while running.load(Ordering::SeqCst) {
        // Check for stop command
        if let Ok(MouseTrackerCommand::Stop) = cmd_rx.try_recv() {
            info!("Tracker stop requested");
            break;
        }

        // Process mouse positions with throttling
        while let Ok((x, y)) = pos_rx.try_recv() {
            let now = Instant::now();
            if now.duration_since(last_emit_time) >= throttle_duration {
                last_emit_time = now;
                let _ = event_tx.try_send(MouseTrackerEvent::PositionUpdate { x, y });
            }
        }

        // Small sleep to avoid busy loop
        thread::sleep(Duration::from_millis(10));
    }

    running.store(false, Ordering::SeqCst);
    info!("Mouse tracker thread exiting");
}
