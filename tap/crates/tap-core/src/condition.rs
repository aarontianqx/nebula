//! Condition types and evaluation for Phase 3 conditional execution.
//!
//! Provides:
//! - Condition enum for different condition types
//! - ConditionEvaluator for evaluating conditions
//! - ConditionalAction for actions with conditions

use serde::{Deserialize, Serialize};

/// Comparison operators for counter conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    /// Equal
    Eq,
    /// Not equal
    Ne,
    /// Greater than
    Gt,
    /// Less than
    Lt,
    /// Greater than or equal
    Gte,
    /// Less than or equal
    Lte,
}

impl CompareOp {
    /// Evaluate the comparison.
    pub fn evaluate(&self, left: i32, right: i32) -> bool {
        match self {
            CompareOp::Eq => left == right,
            CompareOp::Ne => left != right,
            CompareOp::Gt => left > right,
            CompareOp::Lt => left < right,
            CompareOp::Gte => left >= right,
            CompareOp::Lte => left <= right,
        }
    }
}

/// RGB color for pixel conditions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl ConditionColor {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Calculate the difference between two colors.
    pub fn difference(&self, other: &ConditionColor) -> u32 {
        let dr = (self.r as i32 - other.r as i32).unsigned_abs();
        let dg = (self.g as i32 - other.g as i32).unsigned_abs();
        let db = (self.b as i32 - other.b as i32).unsigned_abs();
        dr + dg + db
    }

    /// Check if this color matches another within a tolerance.
    pub fn matches(&self, other: &ConditionColor, tolerance: u8) -> bool {
        self.difference(other) <= tolerance as u32
    }
}

impl Default for ConditionColor {
    fn default() -> Self {
        Self { r: 0, g: 0, b: 0 }
    }
}

/// A condition that can be evaluated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Condition {
    /// Check if a window with the given title/process is focused.
    WindowFocused {
        title: Option<String>,
        process: Option<String>,
    },
    /// Check if a window with the given title/process exists.
    WindowExists {
        title: Option<String>,
        process: Option<String>,
    },
    /// Check if the pixel at (x, y) matches the expected color within tolerance.
    PixelColor {
        x: i32,
        y: i32,
        color: ConditionColor,
        tolerance: u8,
    },
    /// Check a counter value against a comparison.
    Counter {
        key: String,
        op: CompareOp,
        value: i32,
    },
    /// Always true.
    Always,
    /// Always false.
    Never,
    /// Logical AND of multiple conditions.
    And(Vec<Condition>),
    /// Logical OR of multiple conditions.
    Or(Vec<Condition>),
    /// Logical NOT of a condition.
    Not(Box<Condition>),
}

impl Default for Condition {
    fn default() -> Self {
        Condition::Always
    }
}

/// Result of condition evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionResult {
    /// Condition is satisfied.
    Satisfied,
    /// Condition is not satisfied.
    NotSatisfied,
    /// Evaluation failed (e.g., window not found, pixel read error).
    Error,
}

impl ConditionResult {
    pub fn is_satisfied(&self) -> bool {
        matches!(self, ConditionResult::Satisfied)
    }
}

/// Trait for evaluating conditions.
/// This is implemented by the engine which has access to platform APIs and variables.
pub trait ConditionEvaluator {
    /// Check if a window is focused.
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool;

    /// Check if a window exists.
    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool;

    /// Get the pixel color at the given coordinates.
    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor>;

    /// Get a counter value.
    fn get_counter(&self, key: &str) -> i32;

    /// Evaluate a condition.
    fn evaluate(&self, condition: &Condition) -> ConditionResult {
        match condition {
            Condition::WindowFocused { title, process } => {
                if self.is_window_focused(title.as_deref(), process.as_deref()) {
                    ConditionResult::Satisfied
                } else {
                    ConditionResult::NotSatisfied
                }
            }
            Condition::WindowExists { title, process } => {
                if self.window_exists(title.as_deref(), process.as_deref()) {
                    ConditionResult::Satisfied
                } else {
                    ConditionResult::NotSatisfied
                }
            }
            Condition::PixelColor {
                x,
                y,
                color,
                tolerance,
            } => {
                if let Some(actual) = self.get_pixel_color(*x, *y) {
                    if color.matches(&actual, *tolerance) {
                        ConditionResult::Satisfied
                    } else {
                        ConditionResult::NotSatisfied
                    }
                } else {
                    ConditionResult::Error
                }
            }
            Condition::Counter { key, op, value } => {
                let actual = self.get_counter(key);
                if op.evaluate(actual, *value) {
                    ConditionResult::Satisfied
                } else {
                    ConditionResult::NotSatisfied
                }
            }
            Condition::Always => ConditionResult::Satisfied,
            Condition::Never => ConditionResult::NotSatisfied,
            Condition::And(conditions) => {
                for c in conditions {
                    match self.evaluate(c) {
                        ConditionResult::Satisfied => continue,
                        other => return other,
                    }
                }
                ConditionResult::Satisfied
            }
            Condition::Or(conditions) => {
                for c in conditions {
                    match self.evaluate(c) {
                        ConditionResult::Satisfied => return ConditionResult::Satisfied,
                        ConditionResult::Error => return ConditionResult::Error,
                        ConditionResult::NotSatisfied => continue,
                    }
                }
                ConditionResult::NotSatisfied
            }
            Condition::Not(c) => match self.evaluate(c) {
                ConditionResult::Satisfied => ConditionResult::NotSatisfied,
                ConditionResult::NotSatisfied => ConditionResult::Satisfied,
                ConditionResult::Error => ConditionResult::Error,
            },
        }
    }
}

/// Configuration for wait-until behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaitUntilConfig {
    /// The condition to wait for.
    pub condition: Condition,
    /// Maximum time to wait in milliseconds. None means wait forever.
    pub timeout_ms: Option<u64>,
    /// Polling interval in milliseconds.
    pub poll_interval_ms: u64,
}

impl Default for WaitUntilConfig {
    fn default() -> Self {
        Self {
            condition: Condition::Always,
            timeout_ms: Some(30000), // 30 seconds default timeout
            poll_interval_ms: 100,   // 100ms polling interval
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestEvaluator {
        counters: std::collections::HashMap<String, i32>,
    }

    impl ConditionEvaluator for TestEvaluator {
        fn is_window_focused(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
            true
        }

        fn window_exists(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
            true
        }

        fn get_pixel_color(&self, _x: i32, _y: i32) -> Option<ConditionColor> {
            Some(ConditionColor::new(255, 0, 0))
        }

        fn get_counter(&self, key: &str) -> i32 {
            self.counters.get(key).copied().unwrap_or(0)
        }
    }

    #[test]
    fn test_counter_condition() {
        let mut eval = TestEvaluator {
            counters: std::collections::HashMap::new(),
        };
        eval.counters.insert("test".to_string(), 5);

        let cond = Condition::Counter {
            key: "test".to_string(),
            op: CompareOp::Lt,
            value: 10,
        };
        assert!(eval.evaluate(&cond).is_satisfied());

        let cond = Condition::Counter {
            key: "test".to_string(),
            op: CompareOp::Gt,
            value: 10,
        };
        assert!(!eval.evaluate(&cond).is_satisfied());
    }

    #[test]
    fn test_and_or_not() {
        let eval = TestEvaluator {
            counters: std::collections::HashMap::new(),
        };

        // AND: Always && Never = Never
        let cond = Condition::And(vec![Condition::Always, Condition::Never]);
        assert!(!eval.evaluate(&cond).is_satisfied());

        // OR: Always || Never = Always
        let cond = Condition::Or(vec![Condition::Always, Condition::Never]);
        assert!(eval.evaluate(&cond).is_satisfied());

        // NOT: !Never = Always
        let cond = Condition::Not(Box::new(Condition::Never));
        assert!(eval.evaluate(&cond).is_satisfied());
    }
}

