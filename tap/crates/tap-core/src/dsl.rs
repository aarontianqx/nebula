//! DSL (Domain Specific Language) module for YAML-based macro definitions.
//!
//! This module provides:
//! - YAML serialization/deserialization for profiles
//! - DSL schema types with metadata (version, author, description)
//! - Conversion between DSL and internal Profile types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{
    Action, CompareOp, Condition, ConditionColor, MouseButton, Point, Profile, Repeat, RunConfig,
    TargetWindow, TimedAction, Timeline,
};

/// DSL schema version.
pub const DSL_VERSION: &str = "1.0";

/// DSL representation of a macro profile.
/// This is the user-facing YAML format with metadata and human-readable structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslProfile {
    /// Name of the macro.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// DSL schema version.
    #[serde(default = "default_version")]
    pub version: String,
    /// Author name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,

    /// Parameterized variables (filled in before execution).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables: HashMap<String, VariableDefinition>,

    /// Target window binding (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_window: Option<DslTargetWindow>,

    /// Action timeline.
    pub timeline: Vec<DslTimedAction>,

    /// Execution configuration.
    #[serde(default)]
    pub run: DslRunConfig,
}

fn default_version() -> String {
    DSL_VERSION.to_string()
}

/// Variable definition for parameterized macros.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    /// Variable type: "string", "number", "boolean".
    #[serde(rename = "type")]
    pub var_type: VariableType,
    /// Default value (as string, will be parsed according to type).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    /// Human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Supported variable types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    String,
    Number,
    Boolean,
}

/// Target window binding in DSL format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DslTargetWindow {
    /// Window title pattern (partial match).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Process name pattern (partial match).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    /// Whether to pause when target window is not focused.
    #[serde(default = "default_pause")]
    pub pause_when_unfocused: bool,
}

fn default_pause() -> bool {
    true
}

/// Timed action in DSL format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslTimedAction {
    /// Milliseconds since the start of the timeline.
    pub at_ms: u64,
    /// The action to perform.
    pub action: DslAction,
    /// Whether this action is enabled.
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
    /// Optional note/comment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn default_enabled() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

/// Action in DSL format (more human-readable than internal format).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslAction {
    /// Single click.
    Click {
        x: DslValue,
        y: DslValue,
        #[serde(default)]
        button: DslMouseButton,
    },
    /// Double click.
    DoubleClick {
        x: DslValue,
        y: DslValue,
        #[serde(default)]
        button: DslMouseButton,
    },
    /// Mouse button down.
    MouseDown {
        x: DslValue,
        y: DslValue,
        #[serde(default)]
        button: DslMouseButton,
    },
    /// Mouse button up.
    MouseUp {
        x: DslValue,
        y: DslValue,
        #[serde(default)]
        button: DslMouseButton,
    },
    /// Move mouse.
    MouseMove { x: DslValue, y: DslValue },
    /// Drag from one point to another.
    Drag {
        from_x: DslValue,
        from_y: DslValue,
        to_x: DslValue,
        to_y: DslValue,
        #[serde(default = "default_drag_duration")]
        duration_ms: u64,
    },
    /// Scroll wheel.
    Scroll {
        #[serde(default)]
        delta_x: DslValue,
        delta_y: DslValue,
    },
    /// Press and release a key.
    KeyTap { key: String },
    /// Key down.
    KeyDown { key: String },
    /// Key up.
    KeyUp { key: String },
    /// Type text.
    TextInput { text: DslValue },
    /// Wait/delay.
    Wait { ms: u64 },

    // === Conditional actions ===
    /// Wait until condition is satisfied.
    WaitUntil {
        condition: DslCondition,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_ms: Option<u64>,
        #[serde(default = "default_poll_interval")]
        poll_interval_ms: u64,
    },
    /// Conditional execution.
    Conditional {
        condition: DslCondition,
        then_action: Box<DslAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        else_action: Option<Box<DslAction>>,
    },

    // === Counter actions ===
    /// Set counter value.
    SetCounter { key: String, value: DslValue },
    /// Increment counter.
    IncrCounter { key: String },
    /// Decrement counter.
    DecrCounter { key: String },
    /// Reset counter to 0.
    ResetCounter { key: String },

    /// Exit/stop macro.
    Exit,

    // === Phase 4: Sub-macro call ===
    /// Call another macro.
    #[serde(rename = "call_macro")]
    CallMacro {
        name: String,
        #[serde(default, skip_serializing_if = "HashMap::is_empty")]
        args: HashMap<String, DslValue>,
    },
}

fn default_drag_duration() -> u64 {
    500
}

fn default_poll_interval() -> u64 {
    100
}

/// A value that can be either a literal or a variable reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DslValue {
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// String literal or variable reference (e.g., "{{ var_name }}").
    String(String),
    /// Boolean literal.
    Bool(bool),
}

impl Default for DslValue {
    fn default() -> Self {
        DslValue::Int(0)
    }
}

impl DslValue {
    /// Check if this value contains a variable reference.
    pub fn is_variable_ref(&self) -> bool {
        match self {
            DslValue::String(s) => s.contains("{{") && s.contains("}}"),
            _ => false,
        }
    }

    /// Try to convert to i32.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            DslValue::Int(n) => Some(*n as i32),
            DslValue::Float(f) => Some(*f as i32),
            DslValue::String(s) => s.parse().ok(),
            DslValue::Bool(_) => None,
        }
    }

    /// Try to convert to string.
    pub fn as_string(&self) -> String {
        match self {
            DslValue::Int(n) => n.to_string(),
            DslValue::Float(f) => f.to_string(),
            DslValue::String(s) => s.clone(),
            DslValue::Bool(b) => b.to_string(),
        }
    }
}

/// Mouse button in DSL format.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DslMouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

/// Condition in DSL format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DslCondition {
    /// Check if window is focused.
    WindowFocused {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process: Option<String>,
    },
    /// Check if window exists.
    WindowExists {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        process: Option<String>,
    },
    /// Check pixel color.
    PixelColor {
        x: i32,
        y: i32,
        /// Color in "#RRGGBB" format.
        color: String,
        #[serde(default = "default_tolerance")]
        tolerance: u8,
    },
    /// Check counter value.
    Counter {
        key: String,
        op: String,
        value: i32,
    },
    /// Always true.
    Always,
    /// Always false.
    Never,
    /// Logical AND.
    And(Vec<DslCondition>),
    /// Logical OR.
    Or(Vec<DslCondition>),
    /// Logical NOT.
    Not(Box<DslCondition>),
}

fn default_tolerance() -> u8 {
    10
}

/// Run configuration in DSL format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslRunConfig {
    /// Number of times to repeat (0 = forever).
    #[serde(default = "default_repeat")]
    pub repeat: u32,
    /// Delay before first action in milliseconds.
    #[serde(default)]
    pub start_delay_ms: u64,
    /// Speed multiplier (1.0 = normal speed).
    #[serde(default = "default_speed")]
    pub speed: f32,
}

fn default_repeat() -> u32 {
    1
}

fn default_speed() -> f32 {
    1.0
}

impl Default for DslRunConfig {
    fn default() -> Self {
        Self {
            repeat: 1,
            start_delay_ms: 0,
            speed: 1.0,
        }
    }
}

// ============================================================================
// Conversion: Profile -> DslProfile (Export)
// ============================================================================

impl From<&Profile> for DslProfile {
    fn from(profile: &Profile) -> Self {
        DslProfile {
            name: profile.name.clone(),
            description: None,
            version: DSL_VERSION.to_string(),
            author: None,
            variables: HashMap::new(),
            target_window: profile.target_window.as_ref().map(|tw| DslTargetWindow {
                title: tw.title.clone(),
                process: tw.process.clone(),
                pause_when_unfocused: tw.pause_when_unfocused,
            }),
            timeline: profile
                .timeline
                .actions
                .iter()
                .map(DslTimedAction::from)
                .collect(),
            run: DslRunConfig::from(&profile.run),
        }
    }
}

impl From<&TimedAction> for DslTimedAction {
    fn from(ta: &TimedAction) -> Self {
        DslTimedAction {
            at_ms: ta.at_ms,
            action: DslAction::from(&ta.action),
            enabled: ta.enabled,
            note: ta.note.clone(),
        }
    }
}

impl From<&Action> for DslAction {
    fn from(action: &Action) -> Self {
        match action {
            Action::Click { x, y, button } => DslAction::Click {
                x: DslValue::Int(*x as i64),
                y: DslValue::Int(*y as i64),
                button: DslMouseButton::from(*button),
            },
            Action::DoubleClick { x, y, button } => DslAction::DoubleClick {
                x: DslValue::Int(*x as i64),
                y: DslValue::Int(*y as i64),
                button: DslMouseButton::from(*button),
            },
            Action::MouseDown { x, y, button } => DslAction::MouseDown {
                x: DslValue::Int(*x as i64),
                y: DslValue::Int(*y as i64),
                button: DslMouseButton::from(*button),
            },
            Action::MouseUp { x, y, button } => DslAction::MouseUp {
                x: DslValue::Int(*x as i64),
                y: DslValue::Int(*y as i64),
                button: DslMouseButton::from(*button),
            },
            Action::MouseMove { x, y } => DslAction::MouseMove {
                x: DslValue::Int(*x as i64),
                y: DslValue::Int(*y as i64),
            },
            Action::Drag { from, to, duration_ms } => DslAction::Drag {
                from_x: DslValue::Int(from.x as i64),
                from_y: DslValue::Int(from.y as i64),
                to_x: DslValue::Int(to.x as i64),
                to_y: DslValue::Int(to.y as i64),
                duration_ms: *duration_ms,
            },
            Action::Scroll { delta_x, delta_y } => DslAction::Scroll {
                delta_x: DslValue::Int(*delta_x as i64),
                delta_y: DslValue::Int(*delta_y as i64),
            },
            Action::KeyTap { key } => DslAction::KeyTap { key: key.clone() },
            Action::KeyDown { key } => DslAction::KeyDown { key: key.clone() },
            Action::KeyUp { key } => DslAction::KeyUp { key: key.clone() },
            Action::TextInput { text } => DslAction::TextInput {
                text: DslValue::String(text.clone()),
            },
            Action::Wait { ms } => DslAction::Wait { ms: *ms },
            Action::WaitUntil {
                condition,
                timeout_ms,
                poll_interval_ms,
            } => DslAction::WaitUntil {
                condition: DslCondition::from(condition),
                timeout_ms: *timeout_ms,
                poll_interval_ms: *poll_interval_ms,
            },
            Action::Conditional {
                condition,
                then_action,
                else_action,
            } => DslAction::Conditional {
                condition: DslCondition::from(condition),
                then_action: Box::new(DslAction::from(then_action.as_ref())),
                else_action: else_action
                    .as_ref()
                    .map(|a| Box::new(DslAction::from(a.as_ref()))),
            },
            Action::SetCounter { key, value } => DslAction::SetCounter {
                key: key.clone(),
                value: DslValue::Int(*value as i64),
            },
            Action::IncrCounter { key } => DslAction::IncrCounter { key: key.clone() },
            Action::DecrCounter { key } => DslAction::DecrCounter { key: key.clone() },
            Action::ResetCounter { key } => DslAction::ResetCounter { key: key.clone() },
            Action::Exit => DslAction::Exit,
        }
    }
}

impl From<MouseButton> for DslMouseButton {
    fn from(button: MouseButton) -> Self {
        match button {
            MouseButton::Left => DslMouseButton::Left,
            MouseButton::Right => DslMouseButton::Right,
            MouseButton::Middle => DslMouseButton::Middle,
        }
    }
}

impl From<&Condition> for DslCondition {
    fn from(cond: &Condition) -> Self {
        match cond {
            Condition::WindowFocused { title, process } => DslCondition::WindowFocused {
                title: title.clone(),
                process: process.clone(),
            },
            Condition::WindowExists { title, process } => DslCondition::WindowExists {
                title: title.clone(),
                process: process.clone(),
            },
            Condition::PixelColor {
                x,
                y,
                color,
                tolerance,
            } => DslCondition::PixelColor {
                x: *x,
                y: *y,
                color: format!("#{:02X}{:02X}{:02X}", color.r, color.g, color.b),
                tolerance: *tolerance,
            },
            Condition::Counter { key, op, value } => DslCondition::Counter {
                key: key.clone(),
                op: match op {
                    crate::CompareOp::Eq => "==".to_string(),
                    crate::CompareOp::Ne => "!=".to_string(),
                    crate::CompareOp::Gt => ">".to_string(),
                    crate::CompareOp::Lt => "<".to_string(),
                    crate::CompareOp::Gte => ">=".to_string(),
                    crate::CompareOp::Lte => "<=".to_string(),
                },
                value: *value,
            },
            Condition::Always => DslCondition::Always,
            Condition::Never => DslCondition::Never,
            Condition::And(conditions) => {
                DslCondition::And(conditions.iter().map(DslCondition::from).collect())
            }
            Condition::Or(conditions) => {
                DslCondition::Or(conditions.iter().map(DslCondition::from).collect())
            }
            Condition::Not(c) => DslCondition::Not(Box::new(DslCondition::from(c.as_ref()))),
        }
    }
}

impl From<&RunConfig> for DslRunConfig {
    fn from(rc: &RunConfig) -> Self {
        DslRunConfig {
            repeat: match rc.repeat {
                Repeat::Times(n) => n,
                Repeat::Forever => 0,
            },
            start_delay_ms: rc.start_delay_ms,
            speed: rc.speed,
        }
    }
}

// ============================================================================
// Export to YAML
// ============================================================================

/// Export a Profile to YAML string.
pub fn export_to_yaml(profile: &Profile) -> Result<String, DslError> {
    let dsl_profile = DslProfile::from(profile);
    serde_yaml::to_string(&dsl_profile).map_err(|e| DslError::SerializationError(e.to_string()))
}

/// Export a Profile to YAML with additional metadata.
pub fn export_to_yaml_with_metadata(
    profile: &Profile,
    description: Option<String>,
    author: Option<String>,
) -> Result<String, DslError> {
    let mut dsl_profile = DslProfile::from(profile);
    dsl_profile.description = description;
    dsl_profile.author = author;
    serde_yaml::to_string(&dsl_profile).map_err(|e| DslError::SerializationError(e.to_string()))
}

// ============================================================================
// Import from YAML
// ============================================================================

/// Parse YAML string to DslProfile.
pub fn parse_yaml(yaml: &str) -> Result<DslProfile, DslError> {
    serde_yaml::from_str(yaml).map_err(|e| DslError::ParseError(e.to_string()))
}

/// Import YAML string to Profile (with validation).
pub fn import_from_yaml(yaml: &str) -> Result<Profile, DslError> {
    let dsl_profile = parse_yaml(yaml)?;
    // Note: Full validation is done by schema.rs
    Profile::try_from(&dsl_profile)
}

// ============================================================================
// Conversion: DslProfile -> Profile (Import)
// ============================================================================

impl TryFrom<&DslProfile> for Profile {
    type Error = DslError;

    fn try_from(dsl: &DslProfile) -> Result<Self, Self::Error> {
        let timeline = Timeline {
            actions: dsl
                .timeline
                .iter()
                .map(TimedAction::try_from)
                .collect::<Result<Vec<_>, _>>()?,
        };

        let target_window = dsl.target_window.as_ref().map(|tw| TargetWindow {
            title: tw.title.clone(),
            process: tw.process.clone(),
            pause_when_unfocused: tw.pause_when_unfocused,
        });

        let run = RunConfig::from(&dsl.run);

        Ok(Profile {
            name: dsl.name.clone(),
            timeline,
            run,
            target_window,
        })
    }
}

impl TryFrom<&DslTimedAction> for TimedAction {
    type Error = DslError;

    fn try_from(dsl: &DslTimedAction) -> Result<Self, Self::Error> {
        Ok(TimedAction {
            at_ms: dsl.at_ms,
            action: Action::try_from(&dsl.action)?,
            enabled: dsl.enabled,
            note: dsl.note.clone(),
        })
    }
}

impl TryFrom<&DslAction> for Action {
    type Error = DslError;

    fn try_from(dsl: &DslAction) -> Result<Self, Self::Error> {
        match dsl {
            DslAction::Click { x, y, button } => Ok(Action::Click {
                x: x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("Click x must be a number".to_string())
                })?,
                y: y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("Click y must be a number".to_string())
                })?,
                button: MouseButton::from(*button),
            }),
            DslAction::DoubleClick { x, y, button } => Ok(Action::DoubleClick {
                x: x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("DoubleClick x must be a number".to_string())
                })?,
                y: y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("DoubleClick y must be a number".to_string())
                })?,
                button: MouseButton::from(*button),
            }),
            DslAction::MouseDown { x, y, button } => Ok(Action::MouseDown {
                x: x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseDown x must be a number".to_string())
                })?,
                y: y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseDown y must be a number".to_string())
                })?,
                button: MouseButton::from(*button),
            }),
            DslAction::MouseUp { x, y, button } => Ok(Action::MouseUp {
                x: x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseUp x must be a number".to_string())
                })?,
                y: y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseUp y must be a number".to_string())
                })?,
                button: MouseButton::from(*button),
            }),
            DslAction::MouseMove { x, y } => Ok(Action::MouseMove {
                x: x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseMove x must be a number".to_string())
                })?,
                y: y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("MouseMove y must be a number".to_string())
                })?,
            }),
            DslAction::Drag {
                from_x,
                from_y,
                to_x,
                to_y,
                duration_ms,
            } => Ok(Action::Drag {
                from: Point {
                    x: from_x.as_i32().ok_or_else(|| {
                        DslError::ValidationError("Drag from_x must be a number".to_string())
                    })?,
                    y: from_y.as_i32().ok_or_else(|| {
                        DslError::ValidationError("Drag from_y must be a number".to_string())
                    })?,
                },
                to: Point {
                    x: to_x.as_i32().ok_or_else(|| {
                        DslError::ValidationError("Drag to_x must be a number".to_string())
                    })?,
                    y: to_y.as_i32().ok_or_else(|| {
                        DslError::ValidationError("Drag to_y must be a number".to_string())
                    })?,
                },
                duration_ms: *duration_ms,
            }),
            DslAction::Scroll { delta_x, delta_y } => Ok(Action::Scroll {
                delta_x: delta_x.as_i32().ok_or_else(|| {
                    DslError::ValidationError("Scroll delta_x must be a number".to_string())
                })?,
                delta_y: delta_y.as_i32().ok_or_else(|| {
                    DslError::ValidationError("Scroll delta_y must be a number".to_string())
                })?,
            }),
            DslAction::KeyTap { key } => Ok(Action::KeyTap { key: key.clone() }),
            DslAction::KeyDown { key } => Ok(Action::KeyDown { key: key.clone() }),
            DslAction::KeyUp { key } => Ok(Action::KeyUp { key: key.clone() }),
            DslAction::TextInput { text } => Ok(Action::TextInput {
                text: text.as_string(),
            }),
            DslAction::Wait { ms } => Ok(Action::Wait { ms: *ms }),
            DslAction::WaitUntil {
                condition,
                timeout_ms,
                poll_interval_ms,
            } => Ok(Action::WaitUntil {
                condition: Condition::try_from(condition)?,
                timeout_ms: *timeout_ms,
                poll_interval_ms: *poll_interval_ms,
            }),
            DslAction::Conditional {
                condition,
                then_action,
                else_action,
            } => Ok(Action::Conditional {
                condition: Condition::try_from(condition)?,
                then_action: Box::new(Action::try_from(then_action.as_ref())?),
                else_action: else_action
                    .as_ref()
                    .map(|a| Action::try_from(a.as_ref()))
                    .transpose()?
                    .map(Box::new),
            }),
            DslAction::SetCounter { key, value } => Ok(Action::SetCounter {
                key: key.clone(),
                value: value.as_i32().ok_or_else(|| {
                    DslError::ValidationError("SetCounter value must be a number".to_string())
                })?,
            }),
            DslAction::IncrCounter { key } => Ok(Action::IncrCounter { key: key.clone() }),
            DslAction::DecrCounter { key } => Ok(Action::DecrCounter { key: key.clone() }),
            DslAction::ResetCounter { key } => Ok(Action::ResetCounter { key: key.clone() }),
            DslAction::Exit => Ok(Action::Exit),
            DslAction::CallMacro { name: _, args: _ } => {
                // CallMacro is handled at runtime, not converted to a base Action
                Err(DslError::ValidationError(
                    "CallMacro actions are handled at runtime".to_string(),
                ))
            }
        }
    }
}

impl From<DslMouseButton> for MouseButton {
    fn from(button: DslMouseButton) -> Self {
        match button {
            DslMouseButton::Left => MouseButton::Left,
            DslMouseButton::Right => MouseButton::Right,
            DslMouseButton::Middle => MouseButton::Middle,
        }
    }
}

impl TryFrom<&DslCondition> for Condition {
    type Error = DslError;

    fn try_from(dsl: &DslCondition) -> Result<Self, Self::Error> {
        match dsl {
            DslCondition::WindowFocused { title, process } => Ok(Condition::WindowFocused {
                title: title.clone(),
                process: process.clone(),
            }),
            DslCondition::WindowExists { title, process } => Ok(Condition::WindowExists {
                title: title.clone(),
                process: process.clone(),
            }),
            DslCondition::PixelColor {
                x,
                y,
                color,
                tolerance,
            } => {
                let parsed_color = parse_hex_color(color)?;
                Ok(Condition::PixelColor {
                    x: *x,
                    y: *y,
                    color: parsed_color,
                    tolerance: *tolerance,
                })
            }
            DslCondition::Counter { key, op, value } => Ok(Condition::Counter {
                key: key.clone(),
                op: parse_compare_op(op)?,
                value: *value,
            }),
            DslCondition::Always => Ok(Condition::Always),
            DslCondition::Never => Ok(Condition::Never),
            DslCondition::And(conditions) => Ok(Condition::And(
                conditions
                    .iter()
                    .map(Condition::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            DslCondition::Or(conditions) => Ok(Condition::Or(
                conditions
                    .iter()
                    .map(Condition::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            )),
            DslCondition::Not(c) => Ok(Condition::Not(Box::new(Condition::try_from(c.as_ref())?))),
        }
    }
}

impl From<&DslRunConfig> for RunConfig {
    fn from(dsl: &DslRunConfig) -> Self {
        RunConfig {
            repeat: if dsl.repeat == 0 {
                Repeat::Forever
            } else {
                Repeat::Times(dsl.repeat)
            },
            start_delay_ms: dsl.start_delay_ms,
            speed: dsl.speed,
        }
    }
}

/// Parse hex color string (#RRGGBB) to ConditionColor.
fn parse_hex_color(s: &str) -> Result<ConditionColor, DslError> {
    if !s.starts_with('#') || s.len() != 7 {
        return Err(DslError::ValidationError(format!(
            "Invalid color format '{}'. Expected #RRGGBB",
            s
        )));
    }
    let r = u8::from_str_radix(&s[1..3], 16)
        .map_err(|_| DslError::ValidationError(format!("Invalid red component in '{}'", s)))?;
    let g = u8::from_str_radix(&s[3..5], 16)
        .map_err(|_| DslError::ValidationError(format!("Invalid green component in '{}'", s)))?;
    let b = u8::from_str_radix(&s[5..7], 16)
        .map_err(|_| DslError::ValidationError(format!("Invalid blue component in '{}'", s)))?;
    Ok(ConditionColor::new(r, g, b))
}

/// Parse comparison operator string to CompareOp.
fn parse_compare_op(s: &str) -> Result<CompareOp, DslError> {
    match s {
        "==" | "eq" => Ok(CompareOp::Eq),
        "!=" | "ne" => Ok(CompareOp::Ne),
        ">" | "gt" => Ok(CompareOp::Gt),
        "<" | "lt" => Ok(CompareOp::Lt),
        ">=" | "gte" => Ok(CompareOp::Gte),
        "<=" | "lte" => Ok(CompareOp::Lte),
        _ => Err(DslError::ValidationError(format!(
            "Invalid comparison operator '{}'. Use: ==, !=, >, <, >=, <=",
            s
        ))),
    }
}

// ============================================================================
// Error types
// ============================================================================

/// DSL-related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DslError {
    #[error("YAML serialization error: {0}")]
    SerializationError(String),
    #[error("YAML parsing error: {0}")]
    ParseError(String),
    #[error("Validation error: {0}")]
    ValidationError(String),
    #[error("Variable error: {0}")]
    VariableError(String),
    #[error("Expression error: {0}")]
    ExpressionError(String),
}

impl Serialize for DslError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_default_profile() {
        let profile = Profile::default();
        let yaml = export_to_yaml(&profile).unwrap();
        assert!(yaml.contains("name: Default"));
        assert!(yaml.contains("timeline:"));
    }

    #[test]
    fn test_dsl_value_parsing() {
        let int_val = DslValue::Int(100);
        assert_eq!(int_val.as_i32(), Some(100));

        let str_val = DslValue::String("hello".to_string());
        assert_eq!(str_val.as_string(), "hello");

        let var_ref = DslValue::String("{{ my_var }}".to_string());
        assert!(var_ref.is_variable_ref());
    }

    #[test]
    fn test_roundtrip_export_import() {
        let profile = Profile::default();
        let yaml = export_to_yaml(&profile).unwrap();
        let imported = import_from_yaml(&yaml).unwrap();
        assert_eq!(profile.name, imported.name);
        assert_eq!(profile.timeline.actions.len(), imported.timeline.actions.len());
    }

    #[test]
    fn test_parse_hex_color() {
        let color = parse_hex_color("#FF00FF").unwrap();
        assert_eq!(color.r, 255);
        assert_eq!(color.g, 0);
        assert_eq!(color.b, 255);

        assert!(parse_hex_color("FF00FF").is_err());
        assert!(parse_hex_color("#FFF").is_err());
    }

    #[test]
    fn test_parse_compare_op() {
        assert_eq!(parse_compare_op("==").unwrap(), CompareOp::Eq);
        assert_eq!(parse_compare_op("!=").unwrap(), CompareOp::Ne);
        assert_eq!(parse_compare_op(">").unwrap(), CompareOp::Gt);
        assert_eq!(parse_compare_op("<").unwrap(), CompareOp::Lt);
        assert_eq!(parse_compare_op(">=").unwrap(), CompareOp::Gte);
        assert_eq!(parse_compare_op("<=").unwrap(), CompareOp::Lte);
        assert!(parse_compare_op("invalid").is_err());
    }
}

