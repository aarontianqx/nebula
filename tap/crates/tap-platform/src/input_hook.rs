//! Global input event hook for recording.
//!
//! This module captures all keyboard and mouse events for recording.
//!
//! On macOS, we use a custom implementation to avoid rdev's thread-safety issues
//! with keyboard character resolution.

use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tracing::{info, warn};

#[cfg(not(target_os = "macos"))]
use std::time::Instant;

#[cfg(not(target_os = "macos"))]
use tracing::error;

// Platform-specific imports
#[cfg(not(target_os = "macos"))]
use rdev::{listen, Event, EventType};

/// A raw input event captured by the hook.
#[derive(Debug, Clone)]
pub struct RawInputEvent {
    /// Milliseconds since recording started.
    pub timestamp_ms: u64,
    /// The event type.
    pub event: InputEventType,
}

/// Types of input events we capture.
#[derive(Debug, Clone)]
pub enum InputEventType {
    /// Mouse moved to position.
    MouseMove { x: i32, y: i32 },
    /// Mouse button pressed.
    MouseDown { x: i32, y: i32, button: MouseButtonType },
    /// Mouse button released.
    MouseUp { x: i32, y: i32, button: MouseButtonType },
    /// Mouse wheel scrolled.
    Scroll { delta_x: i32, delta_y: i32 },
    /// Key pressed.
    KeyDown { key: String },
    /// Key released.
    KeyUp { key: String },
}

/// Mouse button types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
    Unknown,
}

#[cfg(not(target_os = "macos"))]
impl From<rdev::Button> for MouseButtonType {
    fn from(button: rdev::Button) -> Self {
        match button {
            rdev::Button::Left => MouseButtonType::Left,
            rdev::Button::Right => MouseButtonType::Right,
            rdev::Button::Middle => MouseButtonType::Middle,
            _ => MouseButtonType::Unknown,
        }
    }
}

/// Handle to control the input hook.
pub struct InputHookHandle {
    event_rx: Receiver<RawInputEvent>,
    stop_tx: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl InputHookHandle {
    /// Try to receive an event (non-blocking).
    pub fn try_recv(&self) -> Option<RawInputEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Receive all pending events.
    pub fn drain(&self) -> Vec<RawInputEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Signal the hook to stop.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    /// Check if the hook thread is still running.
    pub fn is_running(&self) -> bool {
        self.thread.as_ref().map_or(false, |t| !t.is_finished())
    }
}

impl Drop for InputHookHandle {
    fn drop(&mut self) {
        self.stop();
        // Take the thread handle but don't join it - the listener blocks
        // and will exit when the subscription is dropped
        let _ = self.thread.take();
    }
}

/// Start capturing global input events.
///
/// Returns a handle that can be used to receive events and stop the hook.
///
/// On Windows/Linux: Uses rdev.
/// On macOS: Uses native Core Graphics API to avoid thread-safety issues.
pub fn start_input_hook() -> InputHookHandle {
    let (event_tx, event_rx) = bounded(1024);
    let (stop_tx, stop_rx) = bounded(1);

    #[cfg(not(target_os = "macos"))]
    let thread = thread::spawn(move || {
        start_hook_rdev(event_tx, stop_rx);
    });

    #[cfg(target_os = "macos")]
    let thread = thread::spawn(move || {
        start_hook_macos(event_tx, stop_rx);
    });

    InputHookHandle {
        event_rx,
        stop_tx,
        thread: Some(thread),
    }
}

/// rdev-based implementation for Windows/Linux.
#[cfg(not(target_os = "macos"))]
fn start_hook_rdev(event_tx: Sender<RawInputEvent>, stop_rx: Receiver<()>) {
    info!("Input hook thread started (rdev)");
    let start_time = Instant::now();

    let callback = move |event: Event| {
        // Check for stop signal
        if stop_rx.try_recv().is_ok() {
            return;
        }

        let timestamp_ms = start_time.elapsed().as_millis() as u64;

        let input_event = match event.event_type {
            EventType::MouseMove { x, y } => Some(InputEventType::MouseMove {
                x: x as i32,
                y: y as i32,
            }),
            EventType::ButtonPress(button) => {
                Some(InputEventType::MouseDown {
                    x: 0, // Will be filled by recorder using last known position
                    y: 0,
                    button: button.into(),
                })
            }
            EventType::ButtonRelease(button) => Some(InputEventType::MouseUp {
                x: 0,
                y: 0,
                button: button.into(),
            }),
            EventType::Wheel { delta_x, delta_y } => Some(InputEventType::Scroll {
                delta_x: delta_x as i32,
                delta_y: delta_y as i32,
            }),
            EventType::KeyPress(key) => Some(InputEventType::KeyDown {
                key: format_key_rdev(key),
            }),
            EventType::KeyRelease(key) => Some(InputEventType::KeyUp {
                key: format_key_rdev(key),
            }),
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
    };

    if let Err(error) = listen(callback) {
        error!(?error, "Input hook error");
    }

    info!("Input hook thread exiting");
}

/// Native Core Graphics implementation for macOS.
/// Uses the singleton global event listener pattern - subscription is automatically
/// cleaned up when this function returns (subscription dropped).
#[cfg(target_os = "macos")]
fn start_hook_macos(event_tx: Sender<RawInputEvent>, stop_rx: Receiver<()>) {
    use crate::macos_events::{keycode_to_name, subscribe_events, MacOSEventType};
    use std::sync::atomic::{AtomicU64, Ordering};

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

/// Format an rdev key to a string.
#[cfg(not(target_os = "macos"))]
fn format_key_rdev(key: rdev::Key) -> String {
    match key {
        rdev::Key::Alt => "Alt".into(),
        rdev::Key::AltGr => "AltGr".into(),
        rdev::Key::Backspace => "Backspace".into(),
        rdev::Key::CapsLock => "CapsLock".into(),
        rdev::Key::ControlLeft => "ControlLeft".into(),
        rdev::Key::ControlRight => "ControlRight".into(),
        rdev::Key::Delete => "Delete".into(),
        rdev::Key::DownArrow => "Down".into(),
        rdev::Key::End => "End".into(),
        rdev::Key::Escape => "Escape".into(),
        rdev::Key::F1 => "F1".into(),
        rdev::Key::F2 => "F2".into(),
        rdev::Key::F3 => "F3".into(),
        rdev::Key::F4 => "F4".into(),
        rdev::Key::F5 => "F5".into(),
        rdev::Key::F6 => "F6".into(),
        rdev::Key::F7 => "F7".into(),
        rdev::Key::F8 => "F8".into(),
        rdev::Key::F9 => "F9".into(),
        rdev::Key::F10 => "F10".into(),
        rdev::Key::F11 => "F11".into(),
        rdev::Key::F12 => "F12".into(),
        rdev::Key::Home => "Home".into(),
        rdev::Key::LeftArrow => "Left".into(),
        rdev::Key::MetaLeft => "MetaLeft".into(),
        rdev::Key::MetaRight => "MetaRight".into(),
        rdev::Key::PageDown => "PageDown".into(),
        rdev::Key::PageUp => "PageUp".into(),
        rdev::Key::Return => "Return".into(),
        rdev::Key::RightArrow => "Right".into(),
        rdev::Key::ShiftLeft => "ShiftLeft".into(),
        rdev::Key::ShiftRight => "ShiftRight".into(),
        rdev::Key::Space => "Space".into(),
        rdev::Key::Tab => "Tab".into(),
        rdev::Key::UpArrow => "Up".into(),
        rdev::Key::PrintScreen => "PrintScreen".into(),
        rdev::Key::ScrollLock => "ScrollLock".into(),
        rdev::Key::Pause => "Pause".into(),
        rdev::Key::NumLock => "NumLock".into(),
        rdev::Key::BackQuote => "`".into(),
        rdev::Key::Num1 => "1".into(),
        rdev::Key::Num2 => "2".into(),
        rdev::Key::Num3 => "3".into(),
        rdev::Key::Num4 => "4".into(),
        rdev::Key::Num5 => "5".into(),
        rdev::Key::Num6 => "6".into(),
        rdev::Key::Num7 => "7".into(),
        rdev::Key::Num8 => "8".into(),
        rdev::Key::Num9 => "9".into(),
        rdev::Key::Num0 => "0".into(),
        rdev::Key::Minus => "-".into(),
        rdev::Key::Equal => "=".into(),
        rdev::Key::KeyQ => "q".into(),
        rdev::Key::KeyW => "w".into(),
        rdev::Key::KeyE => "e".into(),
        rdev::Key::KeyR => "r".into(),
        rdev::Key::KeyT => "t".into(),
        rdev::Key::KeyY => "y".into(),
        rdev::Key::KeyU => "u".into(),
        rdev::Key::KeyI => "i".into(),
        rdev::Key::KeyO => "o".into(),
        rdev::Key::KeyP => "p".into(),
        rdev::Key::LeftBracket => "[".into(),
        rdev::Key::RightBracket => "]".into(),
        rdev::Key::KeyA => "a".into(),
        rdev::Key::KeyS => "s".into(),
        rdev::Key::KeyD => "d".into(),
        rdev::Key::KeyF => "f".into(),
        rdev::Key::KeyG => "g".into(),
        rdev::Key::KeyH => "h".into(),
        rdev::Key::KeyJ => "j".into(),
        rdev::Key::KeyK => "k".into(),
        rdev::Key::KeyL => "l".into(),
        rdev::Key::SemiColon => ";".into(),
        rdev::Key::Quote => "'".into(),
        rdev::Key::BackSlash => "\\".into(),
        rdev::Key::IntlBackslash => "\\".into(),
        rdev::Key::KeyZ => "z".into(),
        rdev::Key::KeyX => "x".into(),
        rdev::Key::KeyC => "c".into(),
        rdev::Key::KeyV => "v".into(),
        rdev::Key::KeyB => "b".into(),
        rdev::Key::KeyN => "n".into(),
        rdev::Key::KeyM => "m".into(),
        rdev::Key::Comma => ",".into(),
        rdev::Key::Dot => ".".into(),
        rdev::Key::Slash => "/".into(),
        rdev::Key::Insert => "Insert".into(),
        rdev::Key::KpReturn => "KpReturn".into(),
        rdev::Key::KpMinus => "KpMinus".into(),
        rdev::Key::KpPlus => "KpPlus".into(),
        rdev::Key::KpMultiply => "KpMultiply".into(),
        rdev::Key::KpDivide => "KpDivide".into(),
        rdev::Key::Kp0 => "Kp0".into(),
        rdev::Key::Kp1 => "Kp1".into(),
        rdev::Key::Kp2 => "Kp2".into(),
        rdev::Key::Kp3 => "Kp3".into(),
        rdev::Key::Kp4 => "Kp4".into(),
        rdev::Key::Kp5 => "Kp5".into(),
        rdev::Key::Kp6 => "Kp6".into(),
        rdev::Key::Kp7 => "Kp7".into(),
        rdev::Key::Kp8 => "Kp8".into(),
        rdev::Key::Kp9 => "Kp9".into(),
        rdev::Key::KpDelete => "KpDelete".into(),
        rdev::Key::Function => "Function".into(),
        rdev::Key::Unknown(code) => format!("Unknown({})", code),
    }
}
