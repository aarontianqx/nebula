//! Global input event hook for recording.
//!
//! This module captures all keyboard and mouse events for recording.
//!
//! Platform implementations:
//! - Windows: Uses native SetWindowsHookEx API (`windows_native.rs`)
//! - macOS: Uses native Core Graphics API (`macos.rs`)
//! - Linux: Uses rdev crate (`rdev_impl.rs`)

use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[cfg(target_os = "windows")]
mod windows_native;

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "linux")]
mod rdev_impl;

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
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButtonType,
    },
    /// Mouse button released.
    MouseUp {
        x: i32,
        y: i32,
        button: MouseButtonType,
    },
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

/// Handle to control the input hook.
pub struct InputHookHandle {
    event_rx: Receiver<RawInputEvent>,
    stop_tx: Sender<()>,
    thread: Option<JoinHandle<()>>,
}

impl InputHookHandle {
    /// Wait up to `timeout` for the next event. `Disconnected` means the hook has ended.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<RawInputEvent, RecvTimeoutError> {
        self.event_rx.recv_timeout(timeout)
    }

    /// Signal the hook to stop.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
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
/// Platform implementations:
/// - Windows: Uses native SetWindowsHookEx API
/// - macOS: Uses native Core Graphics API
/// - Linux: Uses rdev crate
pub fn start_input_hook() -> InputHookHandle {
    let (event_tx, event_rx) = bounded(1024);
    let (stop_tx, stop_rx) = bounded(1);

    #[cfg(target_os = "windows")]
    let thread = thread::spawn(move || {
        windows_native::start_hook(event_tx, stop_rx);
    });

    #[cfg(target_os = "macos")]
    let thread = thread::spawn(move || {
        macos::start_hook(event_tx, stop_rx);
    });

    #[cfg(target_os = "linux")]
    let thread = thread::spawn(move || {
        rdev_impl::start_hook(event_tx, stop_rx);
    });

    InputHookHandle {
        event_rx,
        stop_tx,
        thread: Some(thread),
    }
}
