//! tap-core: domain model + scheduling primitives.
//!
//! Design goal: keep this crate UI-agnostic and platform-agnostic.
//! Platform specific I/O (hook/inject) lives in `tap-platform`.

mod condition;
mod engine;
mod recorder;
mod storage;
mod variables;

pub use condition::{
    CompareOp, Condition, ConditionColor, ConditionEvaluator, ConditionResult, WaitUntilConfig,
};
pub use engine::{
    ActionExecutor, ActionExecutorAdapter, EngineCommand, EngineEvent, EngineState,
    InjectorExecutor, PlatformConditionProvider, Player, PlayerHandle,
};
pub use recorder::{
    BufferedEvent, MouseButtonRaw, RawEventType, Recorder, RecorderConfig, RecorderEvent,
    RecorderState,
};
pub use storage::{
    delete_profile, ensure_profiles_dir, get_app_data_dir, get_profiles_dir, list_profiles,
    load_last_used, load_profile, save_last_used, save_profile, StorageError, StorageResult,
};
pub use variables::VariableStore;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub name: String,
    pub timeline: Timeline,
    pub run: RunConfig,
    /// Target window binding (Phase 3).
    #[serde(default)]
    pub target_window: Option<TargetWindow>,
}

/// Target window binding for a profile.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TargetWindow {
    /// Window title pattern (partial match).
    pub title: Option<String>,
    /// Process name pattern (partial match).
    pub process: Option<String>,
    /// Whether to pause when target window is not focused.
    #[serde(default = "default_pause_when_unfocused")]
    pub pause_when_unfocused: bool,
}

fn default_pause_when_unfocused() -> bool {
    true
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
            target_window: None,
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
    Drag { from: Point, to: Point, duration_ms: u64 },
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

    // === Phase 3: Condition & Variable Actions ===

    /// Wait until a condition is satisfied or timeout.
    WaitUntil {
        condition: Condition,
        timeout_ms: Option<u64>,
        poll_interval_ms: u64,
    },
    /// Conditional execution: if condition then action, else optional action.
    Conditional {
        condition: Condition,
        then_action: Box<Action>,
        else_action: Option<Box<Action>>,
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


