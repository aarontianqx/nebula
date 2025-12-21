//! tap-core: domain model + scheduling primitives.
//!
//! Design goal: keep this crate UI-agnostic and platform-agnostic.
//! Platform specific I/O (hook/inject) lives in `tap-platform`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub timeline: Timeline,
    pub run: RunConfig,
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Default".into(),
            timeline: Timeline {
                actions: vec![
                    TimedAction::after_ms(0, Action::Click { x: 640, y: 360, button: MouseButton::Left }),
                    TimedAction::after_ms(500, Action::Wait { ms: 500 }),
                    TimedAction::after_ms(1200, Action::KeyTap { key: "Space".into() }),
                ],
            },
            run: RunConfig { repeat: Repeat::Forever, start_delay_ms: 0, speed: 1.0 },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunConfig {
    /// Delay before first action, used for "3..2..1" countdown / user switch to target window.
    pub start_delay_ms: u64,
    /// Speed multiplier for replay. 1.0 means real-time.
    pub speed: f32,
    pub repeat: Repeat,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Repeat {
    Times(u32),
    Forever,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub actions: Vec<TimedAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimedAction {
    /// Milliseconds since the start of the timeline.
    pub at_ms: u64,
    pub action: Action,
    /// Disabled actions are skipped during replay.
    pub enabled: bool,
    /// Free-form note for the user.
    pub note: Option<String>,
}

impl TimedAction {
    pub fn after_ms(at_ms: u64, action: Action) -> Self {
        Self { at_ms, action, enabled: true, note: None }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Action {
    Click { x: i32, y: i32, button: MouseButton },
    DoubleClick { x: i32, y: i32, button: MouseButton },
    Drag { from: Point, to: Point, duration_ms: u64 },
    Scroll { delta_x: i32, delta_y: i32 },
    KeyTap { key: String },
    KeyDown { key: String },
    KeyUp { key: String },
    TextInput { text: String },
    Wait { ms: u64 },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}


