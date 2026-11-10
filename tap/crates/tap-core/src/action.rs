//! The concrete form of an action: every value is a literal, ready to inject or evaluate.
//!
//! A macro stores [`DslAction`](crate::DslAction)s, which may hold `{{ }}` templates; the Resolve
//! stage turns each one into an [`Action`] just before it runs.

use serde::{Deserialize, Serialize};

use crate::Condition;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    /// Single click at position.
    Click { x: i32, y: i32, button: MouseButton },
    /// Double click at position.
    DoubleClick { x: i32, y: i32, button: MouseButton },
    /// Mouse button down (for drag operations).
    MouseDown { x: i32, y: i32, button: MouseButton },
    /// Mouse button up (for drag operations).
    MouseUp { x: i32, y: i32, button: MouseButton },
    /// Move mouse to position.
    MouseMove { x: i32, y: i32 },
    /// Drag from one point to another.
    Drag {
        from: Point,
        to: Point,
        duration_ms: u64,
    },
    /// Scroll wheel.
    Scroll { delta_x: i32, delta_y: i32 },
    /// Press and release a key.
    KeyTap { key: String },
    /// Key down (for key combos).
    KeyDown { key: String },
    /// Key up (for key combos).
    KeyUp { key: String },
    /// Type text string.
    TextInput { text: String },
    /// Wait/delay.
    Wait { ms: u64 },

    /// Wait until a condition is satisfied or timeout.
    WaitUntil {
        condition: Condition,
        timeout_ms: Option<u64>,
        poll_interval_ms: u64,
    },
    /// Set a counter to a value.
    SetCounter { key: String, value: i32 },
    /// Increment a counter by 1.
    IncrCounter { key: String },
    /// Decrement a counter by 1.
    DecrCounter { key: String },
    /// Reset a counter to 0.
    ResetCounter { key: String },
    /// Exit/stop the macro (used for exit conditions).
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}
