//! Schema validation for DSL profiles.
//!
//! Provides validation logic to ensure imported YAML profiles are valid
//! before converting them to internal Profile format.

use crate::dsl::{
    CoordinateSpace, DslAction, DslCondition, DslRunConfig, DslTimedAction, DslValue,
    MacroDocument, MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX,
};
use serde::{Deserialize, Serialize};

/// Validation error with context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationError {
    /// Path to the problematic field (e.g., "timeline[0].action.x").
    pub path: String,
    /// Error message.
    pub message: String,
    /// Line number in YAML (if available).
    pub line: Option<usize>,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            write!(f, "Line {}: {} - {}", line, self.path, self.message)
        } else {
            write!(f, "{} - {}", self.path, self.message)
        }
    }
}

/// Validation result.
pub type ValidationResult = Result<(), Vec<ValidationError>>;

/// Validate a [`MacroDocument`].
pub fn validate_profile(profile: &MacroDocument) -> ValidationResult {
    let mut errors = Vec::new();

    // Validate name
    if profile.name.trim().is_empty() {
        errors.push(ValidationError {
            path: "name".to_string(),
            message: "Profile name cannot be empty".to_string(),
            line: None,
        });
    }

    // Validate timeline
    if profile.timeline.is_empty() {
        errors.push(ValidationError {
            path: "timeline".to_string(),
            message: "Timeline must have at least one action".to_string(),
            line: None,
        });
    }

    // Validate each action
    for (i, timed_action) in profile.timeline.iter().enumerate() {
        validate_timed_action(timed_action, &format!("timeline[{}]", i), &mut errors);
    }

    // Window-relative coordinates need a window to be relative to.
    if let Some(target) = &profile.target_window {
        let names = |s: &Option<String>| s.as_deref().is_some_and(|s| !s.trim().is_empty());
        if target.coordinates == CoordinateSpace::Window
            && !names(&target.title)
            && !names(&target.process)
        {
            errors.push(ValidationError {
                path: "target_window.coordinates".to_string(),
                message: "window-relative coordinates need the target window's title or process"
                    .to_string(),
                line: None,
            });
        }
    }

    // Validate run config
    validate_run_config(&profile.run, "run", &mut errors);

    // Validate variables
    for name in profile.variables.keys() {
        if name.trim().is_empty() {
            errors.push(ValidationError {
                path: format!("variables.{}", name),
                message: "Variable name cannot be empty".to_string(),
                line: None,
            });
        }
        // Variable names should be valid identifiers
        if !is_valid_identifier(name) {
            errors.push(ValidationError {
                path: format!("variables.{}", name),
                message: "Variable name must be a valid identifier (alphanumeric and underscore)"
                    .to_string(),
                line: None,
            });
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn validate_timed_action(ta: &DslTimedAction, path: &str, errors: &mut Vec<ValidationError>) {
    validate_action(&ta.action, &format!("{}.action", path), errors);
}

fn validate_action(action: &DslAction, path: &str, errors: &mut Vec<ValidationError>) {
    match action {
        DslAction::Click { x, y, button: _ } => {
            validate_coordinate(x, &format!("{}.x", path), errors);
            validate_coordinate(y, &format!("{}.y", path), errors);
        }
        DslAction::DoubleClick { x, y, button: _ } => {
            validate_coordinate(x, &format!("{}.x", path), errors);
            validate_coordinate(y, &format!("{}.y", path), errors);
        }
        DslAction::MouseDown { x, y, button: _ } => {
            validate_coordinate(x, &format!("{}.x", path), errors);
            validate_coordinate(y, &format!("{}.y", path), errors);
        }
        DslAction::MouseUp { x, y, button: _ } => {
            validate_coordinate(x, &format!("{}.x", path), errors);
            validate_coordinate(y, &format!("{}.y", path), errors);
        }
        DslAction::MouseMove { x, y } => {
            validate_coordinate(x, &format!("{}.x", path), errors);
            validate_coordinate(y, &format!("{}.y", path), errors);
        }
        DslAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            duration_ms: _,
        } => {
            validate_coordinate(from_x, &format!("{}.from_x", path), errors);
            validate_coordinate(from_y, &format!("{}.from_y", path), errors);
            validate_coordinate(to_x, &format!("{}.to_x", path), errors);
            validate_coordinate(to_y, &format!("{}.to_y", path), errors);
        }
        DslAction::Scroll { delta_x, delta_y } => {
            validate_scroll_delta(delta_x, &format!("{}.delta_x", path), errors);
            validate_scroll_delta(delta_y, &format!("{}.delta_y", path), errors);
        }
        DslAction::KeyTap { key } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Key cannot be empty".to_string(),
                    line: None,
                });
            }
        }
        DslAction::KeyDown { key } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Key cannot be empty".to_string(),
                    line: None,
                });
            }
        }
        DslAction::KeyUp { key } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Key cannot be empty".to_string(),
                    line: None,
                });
            }
        }
        DslAction::TextInput { text: _ } => {
            // Text can be empty (for clearing)
        }
        DslAction::Wait { ms: _ } => {
            // Any ms value is valid
        }
        DslAction::WaitUntil {
            condition,
            timeout_ms: _,
            poll_interval_ms,
        } => {
            validate_condition(condition, &format!("{}.condition", path), errors);
            if *poll_interval_ms == 0 {
                errors.push(ValidationError {
                    path: format!("{}.poll_interval_ms", path),
                    message: "Poll interval must be greater than 0".to_string(),
                    line: None,
                });
            }
        }
        DslAction::Conditional {
            condition,
            then_action,
            else_action,
        } => {
            validate_condition(condition, &format!("{}.condition", path), errors);
            validate_nested_action(then_action, &format!("{}.then_action", path), errors);
            if let Some(else_act) = else_action {
                validate_nested_action(else_act, &format!("{}.else_action", path), errors);
            }
        }
        DslAction::SetCounter { key, value: _ } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Counter key cannot be empty".to_string(),
                    line: None,
                });
            }
        }
        DslAction::IncrCounter { key }
        | DslAction::DecrCounter { key }
        | DslAction::ResetCounter { key } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Counter key cannot be empty".to_string(),
                    line: None,
                });
            }
        }
        DslAction::Exit => {
            // No validation needed
        }
        DslAction::CallMacro { name, args: _ } => {
            if name.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.name", path),
                    message: "Macro name cannot be empty".to_string(),
                    line: None,
                });
            }
        }
    }
}

/// Validate an action nested in a conditional.
///
/// The engine expands `call_macro` only for steps of a timeline; nested in an `if` it has no
/// primitive form and would fail halfway through a run. Reject it up front instead.
fn validate_nested_action(action: &DslAction, path: &str, errors: &mut Vec<ValidationError>) {
    if matches!(action, DslAction::CallMacro { .. }) {
        errors.push(ValidationError {
            path: path.to_string(),
            message: "call_macro must be a timeline step; it cannot be nested inside an if"
                .to_string(),
            line: None,
        });
        return;
    }
    validate_action(action, path, errors);
}

fn validate_condition(cond: &DslCondition, path: &str, errors: &mut Vec<ValidationError>) {
    match cond {
        DslCondition::WindowFocused { title, process } => {
            if title.is_none() && process.is_none() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "WindowFocused requires either title or process".to_string(),
                    line: None,
                });
            }
        }
        DslCondition::WindowExists { title, process } => {
            if title.is_none() && process.is_none() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "WindowExists requires either title or process".to_string(),
                    line: None,
                });
            }
        }
        DslCondition::PixelColor {
            x: _,
            y: _,
            color,
            tolerance: _,
        } => {
            if !is_valid_hex_color(color) {
                errors.push(ValidationError {
                    path: format!("{}.color", path),
                    message: "Color must be in #RRGGBB format".to_string(),
                    line: None,
                });
            }
        }
        DslCondition::Counter { key, op, value: _ } => {
            if key.trim().is_empty() {
                errors.push(ValidationError {
                    path: format!("{}.key", path),
                    message: "Counter key cannot be empty".to_string(),
                    line: None,
                });
            }
            if !is_valid_compare_op(op) {
                errors.push(ValidationError {
                    path: format!("{}.op", path),
                    message: "Invalid comparison operator. Use: ==, !=, >, <, >=, <=".to_string(),
                    line: None,
                });
            }
        }
        DslCondition::Always | DslCondition::Never => {
            // No validation needed
        }
        DslCondition::And(conditions) => {
            for (i, c) in conditions.iter().enumerate() {
                validate_condition(c, &format!("{}[{}]", path, i), errors);
            }
        }
        DslCondition::Or(conditions) => {
            for (i, c) in conditions.iter().enumerate() {
                validate_condition(c, &format!("{}[{}]", path, i), errors);
            }
        }
        DslCondition::Not { condition } => {
            validate_condition(condition, path, errors);
        }
    }
}

fn validate_run_config(rc: &DslRunConfig, path: &str, errors: &mut Vec<ValidationError>) {
    if rc.speed <= 0.0 {
        errors.push(ValidationError {
            path: format!("{}.speed", path),
            message: "Speed must be greater than 0".to_string(),
            line: None,
        });
    }
    if rc.speed > 100.0 {
        errors.push(ValidationError {
            path: format!("{}.speed", path),
            message: "Speed cannot exceed 100x".to_string(),
            line: None,
        });
    }
    if rc.humanize.position_px > MAX_HUMANIZE_POSITION_PX {
        errors.push(ValidationError {
            path: format!("{}.humanize.position_px", path),
            message: format!("Position jitter cannot exceed {MAX_HUMANIZE_POSITION_PX} px"),
            line: None,
        });
    }
    if rc.humanize.delay_ms > MAX_HUMANIZE_DELAY_MS {
        errors.push(ValidationError {
            path: format!("{}.humanize.delay_ms", path),
            message: format!("Delay jitter cannot exceed {MAX_HUMANIZE_DELAY_MS} ms"),
            line: None,
        });
    }
}

fn validate_coordinate(value: &DslValue, path: &str, errors: &mut Vec<ValidationError>) {
    // If it's a variable reference, we can't validate the value at parse time
    if value.is_variable_ref() {
        return;
    }

    match value {
        DslValue::Int(n) => {
            if *n < -100000 || *n > 100000 {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Coordinate value out of reasonable range".to_string(),
                    line: None,
                });
            }
        }
        DslValue::Float(f) => {
            if *f < -100000.0 || *f > 100000.0 {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Coordinate value out of reasonable range".to_string(),
                    line: None,
                });
            }
        }
        DslValue::String(s) => {
            // Try to parse as number
            if s.parse::<i64>().is_err() && s.parse::<f64>().is_err() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Coordinate must be a number or variable reference".to_string(),
                    line: None,
                });
            }
        }
        DslValue::Bool(_) => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: "Coordinate cannot be a boolean".to_string(),
                line: None,
            });
        }
    }
}

fn validate_scroll_delta(value: &DslValue, path: &str, errors: &mut Vec<ValidationError>) {
    if value.is_variable_ref() {
        return;
    }

    match value {
        DslValue::Int(n) => {
            if *n < -10000 || *n > 10000 {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Scroll delta out of reasonable range".to_string(),
                    line: None,
                });
            }
        }
        DslValue::Float(f) => {
            if *f < -10000.0 || *f > 10000.0 {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Scroll delta out of reasonable range".to_string(),
                    line: None,
                });
            }
        }
        DslValue::String(s) => {
            if s.parse::<i64>().is_err() && s.parse::<f64>().is_err() {
                errors.push(ValidationError {
                    path: path.to_string(),
                    message: "Scroll delta must be a number or variable reference".to_string(),
                    line: None,
                });
            }
        }
        DslValue::Bool(_) => {
            errors.push(ValidationError {
                path: path.to_string(),
                message: "Scroll delta cannot be a boolean".to_string(),
                line: None,
            });
        }
    }
}

fn is_valid_identifier(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    if !first.is_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_alphanumeric() || c == '_')
}

fn is_valid_hex_color(s: &str) -> bool {
    if !s.starts_with('#') {
        return false;
    }
    let hex = &s[1..];
    hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

fn is_valid_compare_op(op: &str) -> bool {
    matches!(op, "==" | "!=" | ">" | "<" | ">=" | "<=")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl::{DslMouseButton, DslRunConfig, DslTimedAction, DslValue, MacroDocument};

    fn minimal_profile() -> MacroDocument {
        MacroDocument {
            name: "Test".to_string(),
            description: None,
            version: "1.0".to_string(),
            author: None,
            tags: Vec::new(),
            variables: std::collections::HashMap::new(),
            target_window: None,
            timeline: vec![DslTimedAction {
                at_ms: 0,
                action: DslAction::Click {
                    x: DslValue::Int(100),
                    y: DslValue::Int(200),
                    button: DslMouseButton::Left,
                },
                enabled: true,
                note: None,
            }],
            run: DslRunConfig::default(),
        }
    }

    #[test]
    fn test_valid_profile() {
        let profile = minimal_profile();
        assert!(validate_profile(&profile).is_ok());
    }

    #[test]
    fn test_empty_name() {
        let mut profile = minimal_profile();
        profile.name = "".to_string();
        let result = validate_profile(&profile);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| e.path == "name"));
    }

    fn window_target(
        title: Option<&str>,
        process: Option<&str>,
        coordinates: CoordinateSpace,
    ) -> crate::dsl::DslTargetWindow {
        crate::dsl::DslTargetWindow {
            title: title.map(String::from),
            process: process.map(String::from),
            pause_when_unfocused: true,
            coordinates,
        }
    }

    #[test]
    fn window_relative_coordinates_need_a_window_to_be_relative_to() {
        let mut profile = minimal_profile();

        profile.target_window = Some(window_target(None, None, CoordinateSpace::Window));
        let errors = validate_profile(&profile).unwrap_err();
        assert!(errors.iter().any(|e| e.path == "target_window.coordinates"));

        // A blank name is no name.
        profile.target_window = Some(window_target(Some("  "), Some(""), CoordinateSpace::Window));
        assert!(validate_profile(&profile).is_err());

        profile.target_window = Some(window_target(Some("Game"), None, CoordinateSpace::Window));
        assert!(validate_profile(&profile).is_ok());
        profile.target_window = Some(window_target(
            None,
            Some("game.exe"),
            CoordinateSpace::Window,
        ));
        assert!(validate_profile(&profile).is_ok());
    }

    #[test]
    fn screen_coordinates_never_need_a_window_name() {
        let mut profile = minimal_profile();
        profile.target_window = Some(window_target(None, None, CoordinateSpace::Screen));
        assert!(validate_profile(&profile).is_ok());
    }

    #[test]
    fn humanize_is_limited_to_what_still_hits_the_target() {
        let mut profile = minimal_profile();
        profile.run.humanize.position_px = MAX_HUMANIZE_POSITION_PX;
        profile.run.humanize.delay_ms = MAX_HUMANIZE_DELAY_MS;
        assert!(validate_profile(&profile).is_ok());

        profile.run.humanize.position_px = MAX_HUMANIZE_POSITION_PX + 1;
        let errors = validate_profile(&profile).unwrap_err();
        assert!(errors.iter().any(|e| e.path == "run.humanize.position_px"));

        profile.run.humanize.position_px = 0;
        profile.run.humanize.delay_ms = MAX_HUMANIZE_DELAY_MS + 1;
        let errors = validate_profile(&profile).unwrap_err();
        assert!(errors.iter().any(|e| e.path == "run.humanize.delay_ms"));
    }

    #[test]
    fn test_empty_timeline() {
        let mut profile = minimal_profile();
        profile.timeline.clear();
        let result = validate_profile(&profile);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_speed() {
        let mut profile = minimal_profile();
        profile.run.speed = 0.0;
        let result = validate_profile(&profile);
        assert!(result.is_err());
    }

    fn conditional(then_action: DslAction, else_action: Option<DslAction>) -> MacroDocument {
        let mut profile = minimal_profile();
        profile.timeline[0].action = DslAction::Conditional {
            condition: DslCondition::Always,
            then_action: Box::new(then_action),
            else_action: else_action.map(Box::new),
        };
        profile
    }

    fn call(name: &str) -> DslAction {
        DslAction::CallMacro {
            name: name.to_string(),
            args: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn call_macro_is_valid_as_a_timeline_step() {
        let mut profile = minimal_profile();
        profile.timeline[0].action = call("login");
        assert!(validate_profile(&profile).is_ok());
    }

    #[test]
    fn call_macro_nested_in_an_if_is_rejected_in_either_branch() {
        let in_then = conditional(call("login"), None);
        let errors = validate_profile(&in_then).unwrap_err();
        assert!(errors.iter().any(
            |e| e.path == "timeline[0].action.then_action" && e.message.contains("call_macro")
        ));

        let in_else = conditional(DslAction::Exit, Some(call("login")));
        let errors = validate_profile(&in_else).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.path == "timeline[0].action.else_action"));
    }

    #[test]
    fn call_macro_nested_deeper_in_ifs_is_still_rejected() {
        let inner = DslAction::Conditional {
            condition: DslCondition::Always,
            then_action: Box::new(call("login")),
            else_action: None,
        };
        let errors = validate_profile(&conditional(inner, None)).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| e.path == "timeline[0].action.then_action.then_action"));
    }

    #[test]
    fn test_valid_hex_color() {
        assert!(is_valid_hex_color("#FF00FF"));
        assert!(is_valid_hex_color("#000000"));
        assert!(!is_valid_hex_color("FF00FF"));
        assert!(!is_valid_hex_color("#FFF"));
        assert!(!is_valid_hex_color("#GGGGGG"));
    }

    #[test]
    fn test_valid_identifier() {
        assert!(is_valid_identifier("my_var"));
        assert!(is_valid_identifier("_private"));
        assert!(is_valid_identifier("var123"));
        assert!(!is_valid_identifier("123var"));
        assert!(!is_valid_identifier("my-var"));
        assert!(!is_valid_identifier(""));
    }
}
