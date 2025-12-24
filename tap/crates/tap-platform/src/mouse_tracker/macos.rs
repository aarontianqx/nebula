//! macOS native implementation for mouse tracking.
//!
//! Uses the singleton global event listener pattern.

use super::{MouseTrackerCommand, MouseTrackerConfig, MouseTrackerEvent};
use crate::events::{subscribe_events, MacOSEventType};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::info;

/// Start the mouse tracker using macOS native API.
pub fn start_tracker(
    config: MouseTrackerConfig,
    event_tx: Sender<MouseTrackerEvent>,
    _cmd_rx: Receiver<MouseTrackerCommand>,
    running: Arc<AtomicBool>,
) {
    info!("Mouse tracker thread started (macOS native, using global listener)");

    let throttle_duration = Duration::from_millis(config.throttle_ms);
    
    // Subscribe to the global event listener
    let subscription = subscribe_events();
    
    let mut last_emit_time = Instant::now();
    
    while running.load(Ordering::SeqCst) {
        match subscription.recv_timeout(Duration::from_millis(100)) {
            Ok(event) => {
                if let MacOSEventType::MouseMove { x, y } = event.event_type {
                    let now = Instant::now();
                    if now.duration_since(last_emit_time) >= throttle_duration {
                        last_emit_time = now;
                        let _ = event_tx.try_send(MouseTrackerEvent::PositionUpdate {
                            x: x as i32,
                            y: y as i32,
                        });
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Just check running flag
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                break;
            }
        }
    }

    running.store(false, Ordering::SeqCst);
    info!("Mouse tracker thread exiting");
}

