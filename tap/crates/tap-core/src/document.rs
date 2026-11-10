//! The macro document: what is stored on disk, edited in the UI and played by the engine, and its
//! YAML form.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::DslAction;

pub const DSL_VERSION: &str = "1.0";

/// A macro: the one model the editor edits, the engine plays and the disk stores, so a save/load
/// or an edit never loses anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MacroDocument {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,

    /// Declared variables with their defaults; a run's overrides win over those.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub variables: HashMap<String, VariableDefinition>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_window: Option<DslTargetWindow>,

    pub timeline: Vec<DslTimedAction>,

    #[serde(default)]
    pub run: DslRunConfig,
}

fn default_version() -> String {
    DSL_VERSION.to_string()
}

/// A declared variable: its type and default.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VariableDefinition {
    #[serde(rename = "type")]
    pub var_type: VariableType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VariableType {
    String,
    Number,
    Boolean,
}

/// What the coordinates of a document are measured from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoordinateSpace {
    /// The screen: coordinates are positions in the OS injection space.
    #[default]
    Screen,
    /// The target window's top-left corner: the window can be moved or the display changed
    /// without re-capturing. Applies to every action with a position and to `pixel_color`.
    Window,
}

impl CoordinateSpace {
    /// For `skip_serializing_if`: only a non-default choice is written out.
    pub fn is_screen(&self) -> bool {
        *self == CoordinateSpace::Screen
    }
}

/// The window a macro is bound to.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DslTargetWindow {
    /// Partial, case-insensitive match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Partial, case-insensitive match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub process: Option<String>,
    #[serde(default = "default_pause")]
    pub pause_when_unfocused: bool,
    #[serde(default, skip_serializing_if = "CoordinateSpace::is_screen")]
    pub coordinates: CoordinateSpace,
}

fn default_pause() -> bool {
    true
}

/// One step: an action and when it starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DslTimedAction {
    /// Milliseconds since the start of a pass.
    pub at_ms: u64,
    pub action: DslAction,
    /// Disabled steps are skipped when playing.
    #[serde(default = "default_enabled", skip_serializing_if = "is_true")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

fn default_enabled() -> bool {
    true
}

fn is_true(b: &bool) -> bool {
    *b
}

impl DslTimedAction {
    pub fn new(at_ms: u64, action: DslAction) -> Self {
        Self {
            at_ms,
            action,
            enabled: true,
            note: None,
        }
    }
}

/// How a macro is played.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DslRunConfig {
    /// Number of passes; 0 repeats forever.
    #[serde(default = "default_repeat")]
    pub repeat: u32,
    /// Wait before the first step, e.g. to switch to the target window.
    #[serde(default)]
    pub start_delay_ms: u64,
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default, skip_serializing_if = "Humanize::is_off")]
    pub humanize: Humanize,
}

/// The largest position jitter `run.humanize.position_px` may ask for. Beyond this a click no
/// longer lands on what was recorded, which is a mistake rather than a humanizing effect.
pub const MAX_HUMANIZE_POSITION_PX: u32 = 100;
/// The largest step-timing jitter `run.humanize.delay_ms` may ask for.
pub const MAX_HUMANIZE_DELAY_MS: u32 = 10_000;

/// Random variation for playback, so repeated runs do not land on exactly the same pixels at
/// exactly the same milliseconds.
///
/// - `position_px`: every click, double click, mouse move and drag end point lands up to this many
///   pixels off on each axis. A press and its release share one offset, so a click is never
///   turned into a tiny drag.
/// - `delay_ms`: when a step starts is moved by up to this many milliseconds, earlier or later.
///   Steps still run in order and explicit `wait` actions are exact.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Humanize {
    #[serde(default)]
    pub position_px: u32,
    #[serde(default)]
    pub delay_ms: u32,
}

impl Humanize {
    /// Whether no variation is configured (the default, and what is left out of the document).
    pub fn is_off(&self) -> bool {
        self.position_px == 0 && self.delay_ms == 0
    }
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
            humanize: Humanize::default(),
        }
    }
}

pub fn document_to_yaml(document: &MacroDocument) -> Result<String, YamlError> {
    serde_yaml::to_string(document).map_err(|e| YamlError::Serialize(e.to_string()))
}

pub fn parse_yaml(yaml: &str) -> Result<MacroDocument, YamlError> {
    serde_yaml::from_str(yaml).map_err(|e| YamlError::Parse(e.to_string()))
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum YamlError {
    #[error("YAML serialization error: {0}")]
    Serialize(String),
    #[error("YAML parsing error: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DslCondition, DslMouseButton, DslValue};

    #[test]
    fn yaml_roundtrip_preserves_metadata_and_variables() {
        let mut variables = HashMap::new();
        variables.insert(
            "count".to_string(),
            VariableDefinition {
                var_type: VariableType::Number,
                default: Some(serde_json::json!(3)),
                description: Some("iteration count".to_string()),
            },
        );

        let doc = MacroDocument {
            name: "Param Macro".to_string(),
            description: Some("uses variables".to_string()),
            version: DSL_VERSION.to_string(),
            author: Some("qa".to_string()),
            tags: vec!["smoke".to_string()],
            variables,
            target_window: None,
            // A coordinate that is an unresolved variable reference must survive.
            timeline: vec![DslTimedAction {
                at_ms: 0,
                action: DslAction::Click {
                    x: DslValue::String("{{ count }}".to_string()),
                    y: DslValue::Int(10),
                    button: DslMouseButton::Right,
                },
                enabled: false,
                note: None,
            }],
            run: DslRunConfig {
                repeat: 0,
                start_delay_ms: 250,
                speed: 1.5,
                humanize: Humanize {
                    position_px: 4,
                    delay_ms: 30,
                },
            },
        };

        let yaml = document_to_yaml(&doc).unwrap();
        let parsed = parse_yaml(&yaml).unwrap();
        assert_eq!(parsed, doc, "YAML round-trip must be lossless");
    }

    #[test]
    fn conditions_roundtrip_including_not() {
        // Regression: `not` wrapping another tagged condition must still
        // serialize (YAML cannot stack two tags on one node, so `Not` is a
        // struct variant). Exercise and/or/not nesting plus `conditional`.
        let doc = MacroDocument {
            name: "Conditions".to_string(),
            description: None,
            version: DSL_VERSION.to_string(),
            author: None,
            tags: vec![],
            variables: HashMap::new(),
            target_window: None,
            run: DslRunConfig::default(),
            timeline: vec![
                DslTimedAction {
                    at_ms: 0,
                    action: DslAction::WaitUntil {
                        condition: DslCondition::And(vec![
                            DslCondition::WindowFocused {
                                title: Some("Notepad".to_string()),
                                process: None,
                            },
                            DslCondition::Not {
                                condition: Box::new(DslCondition::PixelColor {
                                    x: 10,
                                    y: 20,
                                    color: "#00FF00".to_string(),
                                    tolerance: 10,
                                }),
                            },
                        ]),
                        timeout_ms: Some(5000),
                        poll_interval_ms: 100,
                    },
                    enabled: true,
                    note: None,
                },
                DslTimedAction {
                    at_ms: 1,
                    action: DslAction::Conditional {
                        condition: DslCondition::Counter {
                            key: "n".to_string(),
                            op: "<".to_string(),
                            value: 10,
                        },
                        then_action: Box::new(DslAction::Exit),
                        else_action: None,
                    },
                    enabled: true,
                    note: None,
                },
            ],
        };

        let yaml = document_to_yaml(&doc).expect("conditions must serialize");
        let parsed = parse_yaml(&yaml).unwrap();
        assert_eq!(parsed, doc, "condition round-trip must be lossless");
    }

    #[test]
    fn coordinates_default_to_the_screen_and_are_only_written_when_not() {
        let yaml = "name: m\ntarget_window:\n  process: game.exe\ntimeline:\n- at_ms: 0\n  action: !wait\n    ms: 1\n";
        let doc = parse_yaml(yaml).unwrap();
        let target = doc.target_window.clone().unwrap();
        assert_eq!(target.coordinates, CoordinateSpace::Screen);
        assert!(
            !document_to_yaml(&doc).unwrap().contains("coordinates"),
            "the default is not serialized, so existing macros are unchanged"
        );
    }

    #[test]
    fn window_coordinates_roundtrip_through_yaml_and_json() {
        let yaml = "name: m\ntarget_window:\n  title: Game\n  coordinates: window\ntimeline:\n- at_ms: 0\n  action: !wait\n    ms: 1\n";
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(
            doc.target_window.as_ref().unwrap().coordinates,
            CoordinateSpace::Window
        );

        let again = parse_yaml(&document_to_yaml(&doc).unwrap()).unwrap();
        assert_eq!(again, doc);

        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(json["target_window"]["coordinates"], "window");
        let from_json: MacroDocument = serde_json::from_value(json).unwrap();
        assert_eq!(from_json, doc);
    }

    #[test]
    fn humanize_is_off_by_default_and_only_written_when_on() {
        let doc =
            parse_yaml("name: m\ntimeline:\n- at_ms: 0\n  action: !wait\n    ms: 1\n").unwrap();
        assert!(doc.run.humanize.is_off());
        assert!(
            !document_to_yaml(&doc).unwrap().contains("humanize"),
            "the default is not serialized, so existing macros are unchanged"
        );
        assert!(!serde_json::to_string(&doc).unwrap().contains("humanize"));
    }

    #[test]
    fn humanize_fields_default_independently_and_roundtrip() {
        let yaml = "name: m\nrun:\n  humanize:\n    position_px: 3\ntimeline:\n- at_ms: 0\n  action: !wait\n    ms: 1\n";
        let doc = parse_yaml(yaml).unwrap();
        assert_eq!(
            doc.run.humanize,
            Humanize {
                position_px: 3,
                delay_ms: 0
            }
        );

        let again = parse_yaml(&document_to_yaml(&doc).unwrap()).unwrap();
        assert_eq!(again, doc);

        let json = serde_json::to_value(&doc).unwrap();
        assert_eq!(
            json["run"]["humanize"],
            serde_json::json!({ "position_px": 3, "delay_ms": 0 })
        );
    }
}
