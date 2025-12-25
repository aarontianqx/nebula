//! Global input event hook for recording.
//!
//! This module captures all keyboard and mouse events for recording.
//!
//! Platform implementations:
//! - Windows: Uses native SetWindowsHookEx API (`windows_native.rs`)
//! - macOS: Uses native Core Graphics API (`macos.rs`)
//! - Linux: Uses rdev crate (`rdev_impl.rs`)

use crossbeam_channel::{bounded, Receiver, Sender};
use std::thread::{self, JoinHandle};

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

