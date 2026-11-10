//! The macro vocabulary a document stores: actions, values and conditions, plus how a condition
//! becomes its runtime form.
//!
//! Strings in conditions (colors, comparison operators) are parsed only when the condition is
//! resolved, so a half-typed edit is stored as typed and surfaces as a validation finding rather
//! than being rejected.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::{Color, CompareOp, Condition, MouseButton};

/// An action as the document stores it: values may be `{{ }}` templates, resolved just before
/// the action runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

/// A literal, or a string holding a `{{ }}` template.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DslValue {
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// A string literal, or a template such as `{{ var_name }}`.
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
    pub fn is_variable_ref(&self) -> bool {
        match self {
            DslValue::String(s) => s.contains("{{") && s.contains("}}"),
            _ => false,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            DslValue::Int(n) => Some(*n as i32),
            DslValue::Float(f) => Some(*f as i32),
            DslValue::String(s) => s.parse().ok(),
            DslValue::Bool(_) => None,
        }
    }

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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DslMouseButton {
    #[default]
    Left,
    Right,
    Middle,
}

/// Condition in DSL format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    Counter { key: String, op: String, value: i32 },
    /// Always true.
    Always,
    /// Always false.
    Never,
    /// Logical AND.
    And(Vec<DslCondition>),
    /// Logical OR.
    Or(Vec<DslCondition>),
    /// Logical NOT.
    ///
    /// A struct variant (rather than a newtype) so the inner condition nests
    /// under a `condition:` key; YAML cannot stack two tags (`!not !window_...`)
    /// on one node, which would make the macro unserializable.
    Not { condition: Box<DslCondition> },
}

fn default_tolerance() -> u8 {
    10
}

impl DslAction {
    pub fn click(x: i32, y: i32, button: MouseButton) -> Self {
        DslAction::Click {
            x: x.into(),
            y: y.into(),
            button: button.into(),
        }
    }

    pub fn double_click(x: i32, y: i32, button: MouseButton) -> Self {
        DslAction::DoubleClick {
            x: x.into(),
            y: y.into(),
            button: button.into(),
        }
    }

    pub fn mouse_down(x: i32, y: i32, button: MouseButton) -> Self {
        DslAction::MouseDown {
            x: x.into(),
            y: y.into(),
            button: button.into(),
        }
    }

    pub fn mouse_up(x: i32, y: i32, button: MouseButton) -> Self {
        DslAction::MouseUp {
            x: x.into(),
            y: y.into(),
            button: button.into(),
        }
    }

    pub fn mouse_move(x: i32, y: i32) -> Self {
        DslAction::MouseMove {
            x: x.into(),
            y: y.into(),
        }
    }

    pub fn drag(from: (i32, i32), to: (i32, i32), duration_ms: u64) -> Self {
        DslAction::Drag {
            from_x: from.0.into(),
            from_y: from.1.into(),
            to_x: to.0.into(),
            to_y: to.1.into(),
            duration_ms,
        }
    }

    pub fn scroll(delta_x: i32, delta_y: i32) -> Self {
        DslAction::Scroll {
            delta_x: delta_x.into(),
            delta_y: delta_y.into(),
        }
    }
}

impl From<i32> for DslValue {
    fn from(n: i32) -> Self {
        DslValue::Int(n.into())
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
    type Error = InvalidCondition;

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
            DslCondition::Not { condition } => Ok(Condition::Not(Box::new(Condition::try_from(
                condition.as_ref(),
            )?))),
        }
    }
}

/// A condition whose strings cannot be turned into a runtime [`Condition`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct InvalidCondition(String);

/// Parse hex color string (#RRGGBB) to Color.
fn parse_hex_color(s: &str) -> Result<Color, InvalidCondition> {
    if !s.starts_with('#') || s.len() != 7 {
        return Err(InvalidCondition(format!(
            "Invalid color format '{}'. Expected #RRGGBB",
            s
        )));
    }
    let r = u8::from_str_radix(&s[1..3], 16)
        .map_err(|_| InvalidCondition(format!("Invalid red component in '{}'", s)))?;
    let g = u8::from_str_radix(&s[3..5], 16)
        .map_err(|_| InvalidCondition(format!("Invalid green component in '{}'", s)))?;
    let b = u8::from_str_radix(&s[5..7], 16)
        .map_err(|_| InvalidCondition(format!("Invalid blue component in '{}'", s)))?;
    Ok(Color::new(r, g, b))
}

/// Parse comparison operator string to CompareOp.
fn parse_compare_op(s: &str) -> Result<CompareOp, InvalidCondition> {
    match s {
        "==" | "eq" => Ok(CompareOp::Eq),
        "!=" | "ne" => Ok(CompareOp::Ne),
        ">" | "gt" => Ok(CompareOp::Gt),
        "<" | "lt" => Ok(CompareOp::Lt),
        ">=" | "gte" => Ok(CompareOp::Gte),
        "<=" | "lte" => Ok(CompareOp::Lte),
        _ => Err(InvalidCondition(format!(
            "Invalid comparison operator '{}'. Use: ==, !=, >, <, >=, <=",
            s
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn constructors_store_plain_integer_literals() {
        assert_eq!(
            DslAction::click(3, 4, MouseButton::Right),
            DslAction::Click {
                x: DslValue::Int(3),
                y: DslValue::Int(4),
                button: DslMouseButton::Right,
            }
        );
    }
}
