//! macOS native implementation for input hooking.
//!
//! Uses the singleton global event listener pattern - subscription is automatically
//! cleaned up when this function returns (subscription dropped).

use super::{InputEventType, MouseButtonType, RawInputEvent};
use crate::events::{keycode_to_name, subscribe_events, MacOSEventType};
use crossbeam_channel::{Receiver, Sender};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tracing::{info, warn};

/// Start the input hook using macOS native API.
pub fn start_hook(event_tx: Sender<RawInputEvent>, stop_rx: Receiver<()>) {
    info!("Input hook thread started (macOS native, using global listener)");
    
    // Record start time for relative timestamps (using same time source as MacOSEvent)
    let start_timestamp_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    // Track last flags for detecting key up/down on FlagsChanged
    static LAST_FLAGS: AtomicU64 = AtomicU64::new(0);
    
    // Subscribe to the global event listener
    let subscription = subscribe_events();

    loop {
        // Check for stop signal
        if stop_rx.try_recv().is_ok() {
            info!("Input hook received stop signal");
            break;
        }
        
        // Try to receive an event with timeout
        match subscription.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                // Use the original event timestamp for accurate timing
                // Convert from absolute timestamp to relative (since recording started)
                let timestamp_ms = event.timestamp_ms.saturating_sub(start_timestamp_ms);

                let input_event = match event.event_type {
                    MacOSEventType::MouseMove { x, y } => Some(InputEventType::MouseMove {
                        x: x as i32,
                        y: y as i32,
                    }),
                    MacOSEventType::MouseDown { x, y, button } => {
                        let btn = match button {
                            0 => MouseButtonType::Left,
                            1 => MouseButtonType::Right,
                            _ => MouseButtonType::Middle,
                        };
                        Some(InputEventType::MouseDown {
                            x: x as i32,
                            y: y as i32,
                            button: btn,
                        })
                    }
                    MacOSEventType::MouseUp { x, y, button } => {
                        let btn = match button {
                            0 => MouseButtonType::Left,
                            1 => MouseButtonType::Right,
                            _ => MouseButtonType::Middle,
                        };
                        Some(InputEventType::MouseUp {
                            x: x as i32,
                            y: y as i32,
                            button: btn,
                        })
                    }
                    MacOSEventType::Scroll { delta_x, delta_y } => Some(InputEventType::Scroll {
                        delta_x: delta_x as i32,
                        delta_y: delta_y as i32,
                    }),
                    MacOSEventType::KeyDown { keycode } => Some(InputEventType::KeyDown {
                        key: keycode_to_name(keycode),
                    }),
                    MacOSEventType::KeyUp { keycode } => Some(InputEventType::KeyUp {
                        key: keycode_to_name(keycode),
                    }),
                    MacOSEventType::FlagsChanged { keycode, flags } => {
                        let old_flags = LAST_FLAGS.swap(flags, Ordering::SeqCst);
                        let key = keycode_to_name(keycode);
                        if flags > old_flags {
                            Some(InputEventType::KeyDown { key })
                        } else {
                            Some(InputEventType::KeyUp { key })
                        }
                    }
                };

                if let Some(event_type) = input_event {
                    let raw_event = RawInputEvent {
                        timestamp_ms,
                        event: event_type,
                    };
                    if let Err(e) = event_tx.try_send(raw_event) {
                        warn!("Failed to send input event: {}", e);
                    }
                }
            }
            Err(crossbeam_channel::RecvTimeoutError::Timeout) => {
                // Continue loop, will check stop signal
            }
            Err(crossbeam_channel::RecvTimeoutError::Disconnected) => {
                warn!("Event subscription disconnected");
                break;
            }
        }
    }
    
    // Subscription is dropped here, automatically unsubscribing from global listener
    info!("Input hook thread exiting");
}

