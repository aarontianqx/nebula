use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Script represents an automation script with metadata and execution steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    /// Unique identifier for this script
    pub name: String,

    /// Human-readable explanation of what the script does
    #[serde(default)]
    pub description: Option<String>,

    /// Script version for compatibility tracking
    #[serde(default)]
    pub version: Option<String>,

    /// Script creator
    #[serde(default)]
    pub author: Option<String>,

    /// Ordered execution steps
    pub steps: Vec<Step>,
}

/// Step represents a single step in script execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    /// Scene name this step expects to match
    #[serde(rename = "scene")]
    pub expected_scene: String,

    /// Timeout for waiting for the expected scene
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,

    /// Actions to perform when the scene matches
    #[serde(default)]
    pub actions: Vec<Action>,

    /// Optional loop behavior for this step
    #[serde(rename = "loop")]
    pub loop_config: Option<LoopConfig>,

    /// Optional OCR-based resource checking
    #[serde(default)]
    pub ocr_rule: Option<OcrRule>,
}

/// Action represents a single action within a step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Action {
    /// Action type (click, wait, drag, etc.)
    #[serde(rename = "type")]
    pub action_type: ActionType,

    /// Coordinates for the action
    #[serde(default)]
    pub points: Vec<Point>,

    /// Time for the action (e.g., wait duration)
    #[serde(default, with = "humantime_serde")]
    pub duration: Option<Duration>,

    /// Key for counter operations (incr/decr)
    #[serde(default)]
    pub key: Option<String>,

    /// Condition for conditional actions (quit)
    #[serde(default)]
    pub condition: Option<Condition>,
}

/// ActionType represents the type of action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Click,
    Wait,
    Drag,
    Quit,
    Incr,
    Decr,
    CheckScene,
}

/// Point represents coordinates for actions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Condition defines a condition check for script control.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    /// Comparison operator (eq, gt, lt, neq, gte, lte)
    pub op: String,

    /// Counter key to check
    pub key: String,

    /// Value to compare against
    pub value: i32,
}

impl Condition {
    /// Evaluate the condition against the current counter values
    pub fn evaluate(&self, counters: &HashMap<String, i32>) -> bool {
        let value = counters.get(&self.key).copied().unwrap_or(0);

        match self.op.as_str() {
            "eq" => value == self.value,
            "neq" => value != self.value,
            "gt" => value > self.value,
            "gte" => value >= self.value,
            "lt" => value < self.value,
            "lte" => value <= self.value,
            _ => false,
        }
    }
}

/// Loop defines how a sequence of actions should be repeated.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoopConfig {
    /// Index of the first action in the loop (0-based)
    #[serde(rename = "startIndex", default)]
    pub start_index: usize,

    /// Index of the last action in the loop (0-based)
    #[serde(rename = "endIndex", default)]
    pub end_index: usize,

    /// Number of iterations (-1 for infinite)
    #[serde(default = "default_loop_count")]
    pub count: i32,

    /// Scene name that stops the loop when matched
    #[serde(default)]
    pub until: Option<String>,

    /// Time between loop iterations
    #[serde(default, with = "humantime_serde")]
    pub interval: Option<Duration>,
}

fn default_loop_count() -> i32 {
    -1
}

impl LoopConfig {
    /// Returns true if the loop runs indefinitely
    pub fn is_infinite(&self) -> bool {
        self.count < 0
    }

    /// Returns true if the loop has a scene-based stop condition
    pub fn has_until_condition(&self) -> bool {
        self.until.is_some()
    }

    /// Validate loop indices for the given number of actions
    pub fn validate_indices(&self, action_count: usize) -> Result<(), String> {
        if self.start_index > self.end_index {
            return Err(format!(
                "loop startIndex ({}) cannot be greater than endIndex ({})",
                self.start_index, self.end_index
            ));
        }
        if self.end_index >= action_count {
            return Err(format!(
                "loop endIndex ({}) exceeds action count ({})",
                self.end_index, action_count
            ));
        }
        Ok(())
    }
}

/// OCR rule for resource-based script control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRule {
    /// Rule name (e.g., "quit_when_exhausted")
    pub name: String,

    /// Region of interest for OCR
    pub roi: OcrRegion,

    /// Threshold for the quit condition
    #[serde(default)]
    pub threshold: i32,
}

/// Region of interest for OCR
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRegion {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Helper module for humantime duration parsing
mod humantime_serde {
    use serde::{de, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Option<Duration>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match duration {
            Some(d) => {
                let s = humantime::format_duration(*d).to_string();
                serializer.serialize_some(&s)
            }
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Duration>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        match opt {
            Some(s) => {
                let duration = humantime::parse_duration(&s).map_err(de::Error::custom)?;
                Ok(Some(duration))
            }
            None => Ok(None),
        }
    }
}

/// Script info for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptInfo {
    pub name: String,
    pub description: Option<String>,
}

impl From<&Script> for ScriptInfo {
    fn from(script: &Script) -> Self {
        Self {
            name: script.name.clone(),
            description: script.description.clone(),
        }
    }
}

