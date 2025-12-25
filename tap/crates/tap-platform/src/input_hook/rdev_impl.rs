//! rdev-based implementation for Windows/Linux input hooking.
//!
//! Uses rdev::listen() directly. Note: rdev::listen() blocks forever,
//! so we spawn it in its own thread and communicate via channels.

use super::{InputEventType, MouseButtonType, RawInputEvent};
use crossbeam_channel::{bounded, Receiver, Sender};
use rdev::{listen, Event, EventType};
use std::thread;
use std::time::Instant;
use tracing::{debug, error, info, warn};

/// Start the input hook using rdev.
pub fn start_hook(event_tx: Sender<RawInputEvent>, stop_rx: Receiver<()>) {
    info!("Input hook thread started (rdev direct)");
    let start_time = Instant::now();

    // We need to spawn rdev::listen in a separate thread because it blocks.
    // We'll use a channel to forward events.
    let (rdev_tx, rdev_rx) = bounded::<RawInputEvent>(2048);

    // Clone for the rdev thread
    let rdev_tx_clone = rdev_tx.clone();

    // Spawn the rdev listener thread
    let rdev_thread = thread::spawn(move || {
        debug!("rdev::listen thread starting");
        let callback = move |event: Event| {
            let timestamp_ms = start_time.elapsed().as_millis() as u64;

            let input_event = match event.event_type {
                EventType::MouseMove { x, y } => Some(InputEventType::MouseMove {
                    x: x as i32,
                    y: y as i32,
                }),
                EventType::ButtonPress(button) => Some(InputEventType::MouseDown {
                    x: 0,
                    y: 0,
                    button: button.into(),
                }),
                EventType::ButtonRelease(button) => Some(InputEventType::MouseUp {
                    x: 0,
                    y: 0,
                    button: button.into(),
                }),
                EventType::Wheel { delta_x, delta_y } => Some(InputEventType::Scroll {
                    delta_x: delta_x as i32,
                    delta_y: delta_y as i32,
                }),
                EventType::KeyPress(key) => {
                    let key_str = format_key(key);
                    debug!(key = %key_str, "KeyPress detected");
                    Some(InputEventType::KeyDown { key: key_str })
                }
                EventType::KeyRelease(key) => {
                    let key_str = format_key(key);
                    debug!(key = %key_str, "KeyRelease detected");
                    Some(InputEventType::KeyUp { key: key_str })
                }
            };

            if let Some(event_type) = input_event {
                let raw_event = RawInputEvent {
                    timestamp_ms,
                    event: event_type,
                };
                // Use try_send to avoid blocking
                if let Err(e) = rdev_tx_clone.try_send(raw_event) {
                    warn!("Failed to send input event: {}", e);
                }
            }
        };

        if let Err(error) = listen(callback) {
            error!(?error, "rdev listen error");
        }
        debug!("rdev::listen thread exiting");
    });

    // Main loop: forward events and check for stop signal
    loop {
        // Check for stop signal
        if stop_rx.try_recv().is_ok() {
            info!("Input hook received stop signal");
            break;
        }

        // Forward any events from rdev
        while let Ok(event) = rdev_rx.try_recv() {
            if let Err(e) = event_tx.try_send(event) {
                warn!("Failed to forward input event: {}", e);
            }
        }

        // Small sleep to avoid busy loop
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // Note: We can't cleanly stop rdev::listen(), it will keep running.
    // The thread will be orphaned but this is acceptable for our use case.
    drop(rdev_thread);

    info!("Input hook thread exiting");
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
