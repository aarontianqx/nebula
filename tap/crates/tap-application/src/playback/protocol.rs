//! What the player thread is told and what it reports back.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tap_core::{Action, MacroDocument, VariableValue};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    #[default]
    Idle,
    /// The start countdown is running.
    Arming,
    Running,
    Paused,
    /// A stop was requested; the player settles back to `Idle` right after.
    Stopped,
}

#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// Play `document`. Run-time overrides win over the variable defaults the document declares.
    Start {
        document: Box<MacroDocument>,
        runtime_vars: HashMap<String, VariableValue>,
    },
    Pause,
    Resume,
    Stop,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    StateChanged {
        old: EngineState,
        new: EngineState,
    },
    CountdownTick {
        remaining_secs: u32,
    },
    ActionStarting {
        index: usize,
        action: Action,
    },
    ActionCompleted {
        index: usize,
    },
    IterationCompleted {
        iteration: u32,
    },
    /// Every pass of a finite run is done.
    Completed,
    Error {
        message: String,
    },
    WaitingForCondition {
        condition: String,
    },
    ConditionSatisfied {
        condition: String,
    },
    ConditionTimeout {
        condition: String,
    },
    CounterChanged {
        key: String,
        value: i32,
    },
    /// The run is waiting for its target window (not focused, or not there).
    TargetWindowUnfocused {
        title: Option<String>,
        process: Option<String>,
    },
    TargetWindowFocused,
}
