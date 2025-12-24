//! Input injection implementations.
//!
//! On macOS, Enigo contains CGEventSource which is not Send, so we use a
//! dedicated injection thread and communicate via channels.

use crate::error::{PlatformError, PlatformResult};
use crossbeam_channel::{bounded, Sender};
use enigo::{Axis, Button, Coordinate, Direction, Enigo, Keyboard, Mouse, Settings};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use tap_core::{Action, ActionExecutorAdapter, MouseButton};
use tracing::{debug, error, warn};

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

/// Internal command for the injection thread.
struct InjectionCommand {
    action: Action,
    response_tx: Sender<PlatformResult<()>>,
}

/// Real input injector using `enigo` crate.
///
/// On macOS, Enigo uses CGEventSource which cannot be sent between threads.
/// We work around this by running Enigo in a dedicated thread and sending
/// commands via a channel.
pub struct EnigoInjector {
    cmd_tx: Sender<InjectionCommand>,
    // Keep thread handle alive; thread exits when sender is dropped
    _thread: Arc<JoinHandle<()>>,
}

impl EnigoInjector {
    /// Create a new EnigoInjector.
    pub fn new() -> PlatformResult<Self> {
        let (cmd_tx, cmd_rx) = bounded::<InjectionCommand>(64);

        // Spawn the injection thread
        let thread = thread::spawn(move || {
            // Create Enigo in this thread
            let settings = Settings::default();
            let mut enigo = match Enigo::new(&settings) {
                Ok(e) => e,
                Err(e) => {
                    error!("Failed to create Enigo in injection thread: {}", e);
                    return;
                }
            };

            debug!("Enigo injection thread started");

            // Process commands until the sender is dropped
            while let Ok(cmd) = cmd_rx.recv() {
                let result = execute_action(&mut enigo, &cmd.action);
                // Send response, ignore if receiver is gone
                let _ = cmd.response_tx.send(result);
            }

            debug!("Enigo injection thread exiting");
        });

        Ok(Self {
            cmd_tx,
            _thread: Arc::new(thread),
        })
    }
}

impl Default for EnigoInjector {
    fn default() -> Self {
        Self::new().expect("failed to create EnigoInjector")
    }
}

// EnigoInjector is Send + Sync because it only contains channel senders
// and an Arc<JoinHandle>, both of which are Send + Sync.
unsafe impl Send for EnigoInjector {}
unsafe impl Sync for EnigoInjector {}

impl InputInjector for EnigoInjector {
    fn inject(&self, action: &Action) -> PlatformResult<()> {
        // Create a response channel
        let (response_tx, response_rx) = bounded(1);

        // Send the command
        self.cmd_tx
            .send(InjectionCommand {
                action: action.clone(),
                response_tx,
            })
            .map_err(|e| PlatformError::InjectionFailed(format!("channel send failed: {}", e)))?;

        // Wait for response
        response_rx
            .recv()
            .map_err(|e| PlatformError::InjectionFailed(format!("channel recv failed: {}", e)))?
    }
}

/// Execute an action using the given Enigo instance.
fn execute_action(enigo: &mut Enigo, action: &Action) -> PlatformResult<()> {
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

        // Phase 3: These actions are handled by the execution engine, not the injector
        Action::WaitUntil { .. }
        | Action::Conditional { .. }
        | Action::SetCounter { .. }
        | Action::IncrCounter { .. }
        | Action::DecrCounter { .. }
        | Action::ResetCounter { .. }
        | Action::Exit => {
            debug!("control action - handled by executor, not injector");
            // These are control flow actions handled by the engine
        }
    }

    Ok(())
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
        "enter" | "return" => Key::Return,
        "tab" => Key::Tab,
        "escape" | "esc" => Key::Escape,
        "space" | " " => Key::Space,

        // Platform-specific keys (Windows only, use Help as fallback on macOS)
        #[cfg(target_os = "windows")]
        "insert" | "ins" => Key::Insert,
        #[cfg(not(target_os = "windows"))]
        "insert" | "ins" => {
            warn!("Insert key not available on this platform, using Help as fallback");
            Key::Help
        }

        // Misc
        "capslock" | "caps" => Key::CapsLock,

        #[cfg(target_os = "windows")]
        "printscreen" | "prtsc" => Key::PrintScr,
        #[cfg(not(target_os = "windows"))]
        "printscreen" | "prtsc" => {
            warn!("PrintScreen key not available on this platform");
            return Err(PlatformError::InvalidKey(
                "PrintScreen not supported on this platform".to_string(),
            ));
        }

        #[cfg(target_os = "windows")]
        "pause" => Key::Pause,
        #[cfg(not(target_os = "windows"))]
        "pause" => {
            warn!("Pause key not available on this platform");
            return Err(PlatformError::InvalidKey(
                "Pause not supported on this platform".to_string(),
            ));
        }

        #[cfg(target_os = "windows")]
        "numlock" => Key::Numlock,
        #[cfg(not(target_os = "windows"))]
        "numlock" => {
            warn!("NumLock key not available on this platform");
            return Err(PlatformError::InvalidKey(
                "NumLock not supported on this platform".to_string(),
            ));
        }

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
