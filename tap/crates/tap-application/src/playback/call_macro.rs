//! What a `call_macro` needs besides the callee itself: a call stack that stops cycles and runaway
//! depth, the arguments evaluated in the caller's scope, and the callee's own scope.

use std::collections::HashMap;

use tap_core::{resolve_typed_value, DslValue, ExpressionEngine, MacroDocument};
use tap_core::{VariableStore, VariableValue};

const MAX_CALL_DEPTH: usize = 10;

#[derive(Default)]
pub(super) struct CallStack {
    macros: Vec<String>,
}

impl CallStack {
    pub(super) fn push(&mut self, macro_name: &str) -> Result<(), CallMacroError> {
        if self.macros.iter().any(|name| name == macro_name) {
            return Err(CallMacroError::CircularCall {
                macro_name: macro_name.to_string(),
                call_stack: self.macros.clone(),
            });
        }
        if self.macros.len() >= MAX_CALL_DEPTH {
            return Err(CallMacroError::MaxDepthExceeded {
                depth: self.macros.len(),
                max: MAX_CALL_DEPTH,
            });
        }
        self.macros.push(macro_name.to_string());
        Ok(())
    }

    pub(super) fn pop(&mut self) {
        self.macros.pop();
    }
}

/// Prepare arguments for a sub-macro call.
///
/// Converts DslValue arguments to VariableValue, evaluating `{{ ... }}` templates against the
/// caller's scope. An argument that is exactly one template keeps its type (`{{ n * 2 }}` is a
/// number); text mixing literals and templates becomes a string. A template that cannot be
/// evaluated is an error naming the argument, not a silent `0`.
pub(super) fn prepare_args(
    args: &HashMap<String, DslValue>,
    parent_vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<HashMap<String, VariableValue>, CallMacroError> {
    let mut result = HashMap::new();

    for (key, value) in args {
        let resolved = match value {
            DslValue::Int(n) => VariableValue::Number(*n as f64),
            DslValue::Float(f) => VariableValue::Number(*f),
            DslValue::Bool(b) => VariableValue::Boolean(*b),
            DslValue::String(s) => resolve_typed_value(s, parent_vars, engine)
                .map_err(|e| CallMacroError::VariableError(format!("argument `{key}`: {e}")))?,
        };
        result.insert(key.clone(), resolved);
    }

    Ok(result)
}

/// The callee sees the caller's scope, then its own declared defaults, then the call's arguments
/// (later ones win).
pub(super) fn child_scope(
    parent: &VariableStore,
    callee: &MacroDocument,
    args: HashMap<String, VariableValue>,
) -> VariableStore {
    let mut child = parent.clone();
    child.init_from_definitions(&callee.variables);
    child.set_variables(args);
    child
}

#[derive(Debug, Clone, thiserror::Error)]
pub(super) enum CallMacroError {
    #[error("Circular macro call detected: {macro_name}. Call stack: {call_stack:?}")]
    CircularCall {
        macro_name: String,
        call_stack: Vec<String>,
    },
    #[error("Maximum call depth exceeded: {depth}/{max}")]
    MaxDepthExceeded { depth: usize, max: usize },
    #[error("Variable error in sub-macro: {0}")]
    VariableError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_stack() {
        let mut ctx = CallStack::default();

        assert!(ctx.push("macro1").is_ok());
        assert!(ctx.push("macro2").is_ok());
        assert_eq!(ctx.macros.len(), 2);

        // Circular call should fail
        assert!(ctx.push("macro1").is_err());

        ctx.pop();
        assert_eq!(ctx.macros.len(), 1);
    }

    #[test]
    fn test_max_depth() {
        let mut ctx = CallStack::default();

        for i in 0..MAX_CALL_DEPTH {
            assert!(ctx.push(&format!("macro{}", i)).is_ok());
        }

        // Should fail at max depth
        assert!(ctx.push("one_more").is_err());
    }

    #[test]
    fn test_prepare_args() {
        let mut parent = VariableStore::new();
        parent.set_variable("x", 100);
        parent.set_counter("count", 5);

        let mut args = HashMap::new();
        args.insert("a".to_string(), DslValue::Int(42));
        args.insert("b".to_string(), DslValue::String("{{ x }}".to_string()));
        args.insert("c".to_string(), DslValue::String("{{ count }}".to_string()));

        let result = prepare_args(&args, &parent, &ExpressionEngine::new()).unwrap();

        assert!(matches!(result.get("a"), Some(VariableValue::Number(n)) if *n == 42.0));
        assert!(matches!(result.get("b"), Some(VariableValue::Number(n)) if *n == 100.0));
        assert!(matches!(result.get("c"), Some(VariableValue::Number(n)) if *n == 5.0));
    }

    #[test]
    fn args_may_be_expressions_over_the_callers_scope() {
        let mut parent = VariableStore::new();
        parent.set_variable("x", 100);
        parent.set_variable("name", "Ada");
        parent.set_counter("count", 5);

        let mut args = HashMap::new();
        args.insert("n".to_string(), DslValue::String("{{ x * 2 }}".to_string()));
        args.insert(
            "m".to_string(),
            DslValue::String("{{ count + 1 }}".to_string()),
        );
        args.insert(
            "big".to_string(),
            DslValue::String("{{ x > 50 }}".to_string()),
        );
        args.insert(
            "who".to_string(),
            DslValue::String("hi {{ name }}!".to_string()),
        );
        args.insert("flag".to_string(), DslValue::Bool(true));

        let result = prepare_args(&args, &parent, &ExpressionEngine::new()).unwrap();

        assert!(matches!(result.get("n"), Some(VariableValue::Number(n)) if *n == 200.0));
        assert!(matches!(result.get("m"), Some(VariableValue::Number(n)) if *n == 6.0));
        assert!(matches!(
            result.get("big"),
            Some(VariableValue::Boolean(true))
        ));
        assert!(matches!(result.get("who"), Some(VariableValue::String(s)) if s == "hi Ada!"));
        assert!(matches!(
            result.get("flag"),
            Some(VariableValue::Boolean(true))
        ));
    }

    #[test]
    fn an_argument_that_cannot_be_resolved_is_an_error_naming_it() {
        let parent = VariableStore::new();
        let mut args = HashMap::new();
        args.insert("n".to_string(), DslValue::String("{{ 1 +* }}".to_string()));

        let err = prepare_args(&args, &parent, &ExpressionEngine::new()).unwrap_err();

        assert!(matches!(err, CallMacroError::VariableError(_)));
        assert!(err.to_string().contains("argument `n`"), "{err}");
    }
}
