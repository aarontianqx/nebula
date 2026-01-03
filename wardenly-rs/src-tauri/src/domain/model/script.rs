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

    /// Optional OCR-based resource checking
    #[serde(default, rename = "ocrRule")]
    pub ocr_rule: Option<OcrRule>,
}

/// Action represents a single action within a step.
/// Now implemented as a tagged enum to support nested Loop actions.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    /// Click at specified coordinates
    Click {
        #[serde(default)]
        points: Vec<Point>,
    },

    /// Wait for a duration
    Wait {
        #[serde(default, with = "humantime_serde")]
        duration: Option<Duration>,
    },

    /// Drag between points or along a path
    Drag {
        #[serde(default)]
        points: Vec<Point>,
    },

    /// Quit the script, optionally with a condition
    Quit {
        #[serde(default)]
        condition: Option<Condition>,
    },

    /// Increment a counter
    Incr {
        key: String,
    },

    /// Decrement a counter
    Decr {
        key: String,
    },

    /// Check scene (trigger OCR rule evaluation at this point)
    CheckScene,

    /// Loop action - contains nested actions to repeat
    Loop {
        /// Number of iterations (-1 for infinite)
        #[serde(default = "default_infinite")]
        count: i32,

        /// Time between loop iterations
        #[serde(default, with = "humantime_serde")]
        interval: Option<Duration>,

        /// Scene name that stops the loop when matched
        #[serde(default)]
        until: Option<String>,

        /// Nested actions to execute in the loop
        actions: Vec<Action>,
    },
}

fn default_infinite() -> i32 {
    -1
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

/// OCR mode for different recognition types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrMode {
    /// Recognize usage ratio (e.g., "1/10")
    Ratio,
}

impl Default for OcrMode {
    fn default() -> Self {
        OcrMode::Ratio
    }
}

/// Action to take when OCR condition is met
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OcrAction {
    /// Quit with ResourceExhausted status
    QuitExhausted,
    /// Quit with Completed status
    Quit,
    /// Skip current step
    Skip,
}

impl Default for OcrAction {
    fn default() -> Self {
        OcrAction::QuitExhausted
    }
}

/// OCR rule for resource-based script control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrRule {
    /// OCR recognition mode
    #[serde(default)]
    pub mode: OcrMode,

    /// Region of interest for OCR
    pub roi: OcrRegion,

    /// Condition expression (e.g., "used > 7 || used > total")
    /// Variables: used = denominator, total = numerator
    pub condition: String,

    /// Action to take when condition is met
    #[serde(default)]
    pub action: OcrAction,
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
