//! Resolve stage: turn a parameterized [`DslAction`] into a concrete [`Action`].
//!
//! This is where `{{ var }}` / `{{ expr }}` placeholders become literal values,
//! using the run scope ([`VariableStore`]) and the sandboxed Rhai expression
//! engine. Only fields that actually contain `{{` are evaluated; everything else
//! passes through untouched.

use tap_core::{
    resolve_expressions, Action, Condition, DslAction, DslValue, ExpressionEngine, MacroDocument,
    MouseButton, Point, Profile, RunConfig, TargetWindow, TimedAction, Timeline, VariableStore,
};

/// Errors raised while resolving a parameterized action into a concrete one.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("failed to evaluate expression for `{field}`: {reason}")]
    Expression { field: String, reason: String },
    #[error("`{field}` did not resolve to an integer (got `{got}`)")]
    NotAnInteger { field: String, got: String },
    #[error("invalid condition: {0}")]
    Condition(String),
    #[error("`call_macro` is expanded by the engine and has no primitive form")]
    CallMacro,
}

/// Resolve a [`DslValue`] used as an integer coordinate/amount.
fn resolve_i32(
    value: &DslValue,
    field: &str,
    vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<i32, ResolveError> {
    match value {
        DslValue::Int(n) => Ok(*n as i32),
        DslValue::Float(f) => Ok(*f as i32),
        DslValue::Bool(_) => Err(ResolveError::NotAnInteger {
            field: field.to_string(),
            got: "boolean".to_string(),
        }),
        DslValue::String(s) => {
            let resolved =
                resolve_expressions(s, vars, engine).map_err(|e| ResolveError::Expression {
                    field: field.to_string(),
                    reason: e.to_string(),
                })?;
            let trimmed = resolved.trim();
            if let Ok(n) = trimmed.parse::<i32>() {
                Ok(n)
            } else if let Ok(f) = trimmed.parse::<f64>() {
                Ok(f as i32)
            } else {
                Err(ResolveError::NotAnInteger {
                    field: field.to_string(),
                    got: resolved,
                })
            }
        }
    }
}

/// Resolve a [`DslValue`] used as text (variable refs allowed, type preserved).
fn resolve_text(
    value: &DslValue,
    field: &str,
    vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<String, ResolveError> {
    match value {
        DslValue::String(s) => {
            resolve_expressions(s, vars, engine).map_err(|e| ResolveError::Expression {
                field: field.to_string(),
                reason: e.to_string(),
            })
        }
        other => Ok(other.as_string()),
    }
}

fn resolve_condition(dsl: &tap_core::DslCondition) -> Result<Condition, ResolveError> {
    Condition::try_from(dsl).map_err(|e| ResolveError::Condition(e.to_string()))
}

/// Resolve a parameterized [`DslAction`] into a concrete [`Action`] using `vars`.
///
/// Returns [`ResolveError::CallMacro`] for `call_macro`, which the engine expands
/// inline rather than turning into a primitive action.
pub fn resolve_action(
    dsl: &DslAction,
    vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<Action, ResolveError> {
    let action = match dsl {
        DslAction::Click { x, y, button } => Action::Click {
            x: resolve_i32(x, "click.x", vars, engine)?,
            y: resolve_i32(y, "click.y", vars, engine)?,
            button: MouseButton::from(*button),
        },
        DslAction::DoubleClick { x, y, button } => Action::DoubleClick {
            x: resolve_i32(x, "double_click.x", vars, engine)?,
            y: resolve_i32(y, "double_click.y", vars, engine)?,
            button: MouseButton::from(*button),
        },
        DslAction::MouseDown { x, y, button } => Action::MouseDown {
            x: resolve_i32(x, "mouse_down.x", vars, engine)?,
            y: resolve_i32(y, "mouse_down.y", vars, engine)?,
            button: MouseButton::from(*button),
        },
        DslAction::MouseUp { x, y, button } => Action::MouseUp {
            x: resolve_i32(x, "mouse_up.x", vars, engine)?,
            y: resolve_i32(y, "mouse_up.y", vars, engine)?,
            button: MouseButton::from(*button),
        },
        DslAction::MouseMove { x, y } => Action::MouseMove {
            x: resolve_i32(x, "mouse_move.x", vars, engine)?,
            y: resolve_i32(y, "mouse_move.y", vars, engine)?,
        },
        DslAction::Drag {
            from_x,
            from_y,
            to_x,
            to_y,
            duration_ms,
        } => Action::Drag {
            from: Point {
                x: resolve_i32(from_x, "drag.from_x", vars, engine)?,
                y: resolve_i32(from_y, "drag.from_y", vars, engine)?,
            },
            to: Point {
                x: resolve_i32(to_x, "drag.to_x", vars, engine)?,
                y: resolve_i32(to_y, "drag.to_y", vars, engine)?,
            },
            duration_ms: *duration_ms,
        },
        DslAction::Scroll { delta_x, delta_y } => Action::Scroll {
            delta_x: resolve_i32(delta_x, "scroll.delta_x", vars, engine)?,
            delta_y: resolve_i32(delta_y, "scroll.delta_y", vars, engine)?,
        },
        DslAction::KeyTap { key } => Action::KeyTap { key: key.clone() },
        DslAction::KeyDown { key } => Action::KeyDown { key: key.clone() },
        DslAction::KeyUp { key } => Action::KeyUp { key: key.clone() },
        DslAction::TextInput { text } => Action::TextInput {
            text: resolve_text(text, "text_input.text", vars, engine)?,
        },
        DslAction::Wait { ms } => Action::Wait { ms: *ms },
        DslAction::WaitUntil {
            condition,
            timeout_ms,
            poll_interval_ms,
        } => Action::WaitUntil {
            condition: resolve_condition(condition)?,
            timeout_ms: *timeout_ms,
            poll_interval_ms: *poll_interval_ms,
        },
        DslAction::Conditional {
            condition,
            then_action,
            else_action,
        } => Action::Conditional {
            condition: resolve_condition(condition)?,
            then_action: Box::new(resolve_action(then_action, vars, engine)?),
            else_action: else_action
                .as_ref()
                .map(|a| resolve_action(a, vars, engine))
                .transpose()?
                .map(Box::new),
        },
        DslAction::SetCounter { key, value } => Action::SetCounter {
            key: key.clone(),
            value: resolve_i32(value, "set_counter.value", vars, engine)?,
        },
        DslAction::IncrCounter { key } => Action::IncrCounter { key: key.clone() },
        DslAction::DecrCounter { key } => Action::DecrCounter { key: key.clone() },
        DslAction::ResetCounter { key } => Action::ResetCounter { key: key.clone() },
        DslAction::Exit => Action::Exit,
        DslAction::CallMacro { .. } => return Err(ResolveError::CallMacro),
    };
    Ok(action)
}

/// Build a *resolved* [`Profile`] projection of a document for display/IPC.
///
/// This is a lenient, best-effort view: steps whose parameters cannot be
/// resolved with the supplied scope (including `call_macro`, which has no
/// primitive form) are omitted. The canonical [`MacroDocument`] is unaffected
/// and still carries the original parameterized steps for execution.
pub fn document_to_profile_view(
    doc: &MacroDocument,
    vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Profile {
    let actions = doc
        .timeline
        .iter()
        .filter_map(|ta| {
            resolve_action(&ta.action, vars, engine)
                .ok()
                .map(|action| TimedAction {
                    at_ms: ta.at_ms,
                    action,
                    enabled: ta.enabled,
                    note: ta.note.clone(),
                })
        })
        .collect();

    Profile {
        name: doc.name.clone(),
        timeline: Timeline { actions },
        run: RunConfig::from(&doc.run),
        target_window: doc.target_window.as_ref().map(|tw| TargetWindow {
            title: tw.title.clone(),
            process: tw.process.clone(),
            pause_when_unfocused: tw.pause_when_unfocused,
        }),
    }
}
