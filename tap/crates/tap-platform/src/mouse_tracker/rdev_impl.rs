//! rdev-based implementation for Windows/Linux mouse tracking.

use super::{MouseTrackerConfig, MouseTrackerEvent};
use crossbeam_channel::{bounded, RecvTimeoutError, Sender};
use rdev::{listen, Event, EventType};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use tracing::{debug, error, info};

/// How often the loop looks up to see whether it was asked to stop.
const STOP_CHECK_INTERVAL: Duration = Duration::from_millis(100);

pub fn start_tracker(
    config: MouseTrackerConfig,
    event_tx: Sender<MouseTrackerEvent>,
    running: Arc<AtomicBool>,
) {
    info!("Mouse tracker thread started (rdev direct)");

    let throttle = Duration::from_millis(config.throttle_ms);
    let (pos_tx, pos_rx) = bounded::<(i32, i32)>(256);

    // `rdev::listen` never returns, so this thread cannot be stopped; it lives as long as the
    // process, which is also how long the tracker is wanted.
    thread::spawn(move || {
        let callback = move |event: Event| {
            if let EventType::MouseMove { x, y } = event.event_type {
                let _ = pos_tx.try_send((x as i32, y as i32));
            }
        };
        if let Err(error) = listen(callback) {
            error!(?error, "Mouse tracker rdev listen error");
        }
        debug!("Mouse tracker rdev::listen thread exiting");
    });

    let mut last_emit_time = Instant::now();
    while running.load(Ordering::SeqCst) {
        match pos_rx.recv_timeout(STOP_CHECK_INTERVAL) {
            Ok((x, y)) => {
                let now = Instant::now();
                if now.duration_since(last_emit_time) >= throttle {
                    last_emit_time = now;
                    let _ = event_tx.try_send(MouseTrackerEvent::PositionUpdate { x, y });
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }

    running.store(false, Ordering::SeqCst);
    info!("Mouse tracker thread exiting");
}
