//! Input injection implementations.

use crate::{PlatformError, PlatformResult};
use enigo::{
    Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings,
};
use std::sync::Mutex;
use tap_core::{Action, ActionExecutorAdapter, MouseButton};
use tracing::{debug, warn};

/// Trait for injecting mouse/keyboard actions into the OS.
pub trait InputInjector: Send + Sync {
    fn inject(&self, action: &Action) -> PlatformResult<()>;
}

/// Minimal no-op injector for early UI development / testing.
pub struct NoopInjector;

impl InputInjector for NoopInjector {
    fn inject(&self, action: &Action) -> PlatformResult<()> {
        debug!(?action, "NoopInjector: would inject action");
        Ok(())
    }
}

/// Real input injector using `enigo` crate.
pub struct EnigoInjector {
    enigo: Mutex<Enigo>,
}

impl EnigoInjector {
    /// Create a new EnigoInjector.
    pub fn new() -> PlatformResult<Self> {
        let settings = Settings::default();
        let enigo = Enigo::new(&settings).map_err(|e| {
            PlatformError::InjectionFailed(format!("failed to create Enigo: {e}"))
        })?;
        Ok(Self {
            enigo: Mutex::new(enigo),
        })
    }
}

impl Default for EnigoInjector {
    fn default() -> Self {
        Self::new().expect("failed to create EnigoInjector")
    }
}

impl InputInjector for EnigoInjector {
    fn inject(&self, action: &Action) -> PlatformResult<()> {
        let mut enigo = self.enigo.lock().unwrap();

        match action {
            Action::Click { x, y, button } => {
                debug!(x, y, ?button, "injecting click");
                enigo
                    .move_mouse(*x, *y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                let btn = mouse_button_to_enigo(*button);
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::DoubleClick { x, y, button } => {
                debug!(x, y, ?button, "injecting double click");
                enigo
                    .move_mouse(*x, *y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                let btn = mouse_button_to_enigo(*button);
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                enigo
                    .button(btn, Direction::Click)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::Drag {
                from,
                to,
                duration_ms: _,
            } => {
                debug!(?from, ?to, "injecting drag");
                // Move to start
                enigo
                    .move_mouse(from.x, from.y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                // Press
                enigo
                    .button(Button::Left, Direction::Press)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                // Move to end
                enigo
                    .move_mouse(to.x, to.y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                // Release
                enigo
                    .button(Button::Left, Direction::Release)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::Scroll { delta_x, delta_y } => {
                debug!(delta_x, delta_y, "injecting scroll");
                if *delta_y != 0 {
                    enigo
                        .scroll(*delta_y, Axis::Vertical)
                        .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                }
                if *delta_x != 0 {
                    enigo
                        .scroll(*delta_x, Axis::Horizontal)
                        .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                }
            }

            Action::KeyTap { key } => {
                debug!(key, "injecting key tap");
                let k = parse_key(key)?;
                enigo
                    .key(k, Direction::Click)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::KeyDown { key } => {
                debug!(key, "injecting key down");
                let k = parse_key(key)?;
                enigo
                    .key(k, Direction::Press)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::KeyUp { key } => {
                debug!(key, "injecting key up");
                let k = parse_key(key)?;
                enigo
                    .key(k, Direction::Release)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::TextInput { text } => {
                debug!(text, "injecting text input");
                enigo
                    .text(text)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::MouseDown { x, y, button } => {
                debug!(x, y, ?button, "injecting mouse down");
                enigo
                    .move_mouse(*x, *y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                let btn = mouse_button_to_enigo(*button);
                enigo
                    .button(btn, Direction::Press)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::MouseUp { x, y, button } => {
                debug!(x, y, ?button, "injecting mouse up");
                enigo
                    .move_mouse(*x, *y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
                let btn = mouse_button_to_enigo(*button);
                enigo
                    .button(btn, Direction::Release)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::MouseMove { x, y } => {
                debug!(x, y, "injecting mouse move");
                enigo
                    .move_mouse(*x, *y, Coordinate::Abs)
                    .map_err(|e| PlatformError::InjectionFailed(e.to_string()))?;
            }

            Action::Wait { ms } => {
                debug!(ms, "wait action - handled by executor, not injector");
                // Wait is handled by the execution engine, not the injector
            }
        }

        Ok(())
    }
}

fn mouse_button_to_enigo(button: MouseButton) -> Button {
    match button {
        MouseButton::Left => Button::Left,
        MouseButton::Right => Button::Right,
        MouseButton::Middle => Button::Middle,
    }
}

/// Parse a key string into an enigo Key.
/// Supports common key names and single characters.
fn parse_key(key: &str) -> PlatformResult<enigo::Key> {
    use enigo::Key;

    // Handle single character keys
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        return Ok(Key::Unicode(c));
    }

    // Handle named keys (case-insensitive)
    let key_lower = key.to_lowercase();
    let parsed = match key_lower.as_str() {
        // Function keys
        "f1" => Key::F1,
        "f2" => Key::F2,
        "f3" => Key::F3,
        "f4" => Key::F4,
        "f5" => Key::F5,
        "f6" => Key::F6,
        "f7" => Key::F7,
        "f8" => Key::F8,
        "f9" => Key::F9,
        "f10" => Key::F10,
        "f11" => Key::F11,
        "f12" => Key::F12,

        // Modifiers
        "shift" | "lshift" => Key::Shift,
        "rshift" => Key::RShift,
        "control" | "ctrl" | "lctrl" | "lcontrol" => Key::Control,
        "rctrl" | "rcontrol" => Key::RControl,
        "alt" | "lalt" => Key::Alt,
        "ralt" => Key::Alt, // enigo 0.3 doesn't distinguish left/right Alt
        "meta" | "win" | "super" | "cmd" | "command" => Key::Meta,

        // Navigation
        "up" | "uparrow" => Key::UpArrow,
        "down" | "downarrow" => Key::DownArrow,
        "left" | "leftarrow" => Key::LeftArrow,
        "right" | "rightarrow" => Key::RightArrow,
        "home" => Key::Home,
        "end" => Key::End,
        "pageup" | "pgup" => Key::PageUp,
        "pagedown" | "pgdn" => Key::PageDown,

        // Editing
        "backspace" | "back" => Key::Backspace,
        "delete" | "del" => Key::Delete,
        "insert" | "ins" => Key::Insert,
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "space" | " " => Key::Space,

        // Misc
        "capslock" | "caps" => Key::CapsLock,
        "printscreen" | "prtsc" => Key::PrintScr,
        // "scrolllock" not available in enigo 0.3
        "pause" => Key::Pause,
        "numlock" => Key::Numlock,

        _ => {
            warn!(key, "unknown key, treating as unicode sequence");
            // For unknown keys, if it's a single word, treat first char as unicode
            if let Some(c) = key.chars().next() {
                Key::Unicode(c)
            } else {
                return Err(PlatformError::InvalidKey(key.to_string()));
            }
        }
    };

    Ok(parsed)
}

// Implement ActionExecutorAdapter so EnigoInjector can be used with Player.
impl ActionExecutorAdapter for EnigoInjector {
    fn inject(&self, action: &Action) -> Result<(), String> {
        InputInjector::inject(self, action).map_err(|e| e.to_string())
    }
}

impl ActionExecutorAdapter for NoopInjector {
    fn inject(&self, action: &Action) -> Result<(), String> {
        InputInjector::inject(self, action).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_key_single_char() {
        let k = parse_key("a").unwrap();
        assert!(matches!(k, enigo::Key::Unicode('a')));
    }

    #[test]
    fn test_parse_key_named() {
        let k = parse_key("Space").unwrap();
        assert!(matches!(k, enigo::Key::Space));

        let k = parse_key("ENTER").unwrap();
        assert!(matches!(k, enigo::Key::Return));

        let k = parse_key("ctrl").unwrap();
        assert!(matches!(k, enigo::Key::Control));
    }

    #[test]
    fn test_parse_key_function() {
        let k = parse_key("F1").unwrap();
        assert!(matches!(k, enigo::Key::F1));
    }
}

