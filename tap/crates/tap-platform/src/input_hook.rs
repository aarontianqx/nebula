//! Global input event hook using rdev.
//!
//! This module captures all keyboard and mouse events for recording.

use crossbeam_channel::{bounded, Receiver, Sender};
use rdev::{listen, Event, EventType};
use std::thread::{self, JoinHandle};
use std::time::Instant;
use tracing::{error, info, warn};

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
        // Note: rdev::listen blocks, so we can't reliably join the thread
        // The thread will exit when the process exits
    }
}

/// Start capturing global input events.
///
/// Returns a handle that can be used to receive events and stop the hook.
pub fn start_input_hook() -> InputHookHandle {
    let (event_tx, event_rx) = bounded(1024);
    let (stop_tx, stop_rx) = bounded(1);

    let thread = thread::spawn(move || {
        info!("Input hook thread started");
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
                    // Get mouse position from the event name field (workaround)
                    // rdev doesn't provide position on button events on some platforms
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
                    key: format_key(key),
                }),
                EventType::KeyRelease(key) => Some(InputEventType::KeyUp {
                    key: format_key(key),
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
    });

    InputHookHandle {
        event_rx,
        stop_tx,
        thread: Some(thread),
    }
}

/// Format an rdev key to a string.
fn format_key(key: rdev::Key) -> String {
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

