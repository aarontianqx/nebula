//! Sub-macro calling functionality for Phase 4.
//!
//! Provides:
//! - Loading and executing sub-macros
//! - Call stack tracking to prevent circular calls
//! - Variable context sharing between parent and child macros

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::storage::load_profile;
use tap_core::{resolve_typed_value, DslValue, ExpressionEngine, MacroDocument};
use tap_core::{Profile, VariableStore, VariableValue};

/// Maximum call stack depth to prevent infinite recursion.
pub const MAX_CALL_DEPTH: usize = 10;

/// Sub-macro execution context.
pub struct SubMacroContext {
    /// Current call stack (macro names).
    call_stack: Vec<String>,
    /// Cached loaded profiles.
    profile_cache: HashMap<String, Profile>,
}

impl SubMacroContext {
    /// Create a new sub-macro context.
    pub fn new() -> Self {
        Self {
            call_stack: Vec::new(),
            profile_cache: HashMap::new(),
        }
    }

    /// Check if calling a macro would create a circular reference.
    pub fn would_be_circular(&self, macro_name: &str) -> bool {
        self.call_stack.iter().any(|name| name == macro_name)
    }

    /// Check if the call stack has reached maximum depth.
    pub fn is_max_depth(&self) -> bool {
        self.call_stack.len() >= MAX_CALL_DEPTH
    }

    /// Push a macro onto the call stack.
    pub fn push(&mut self, macro_name: &str) -> Result<(), SubMacroError> {
        if self.would_be_circular(macro_name) {
            return Err(SubMacroError::CircularCall {
                macro_name: macro_name.to_string(),
                call_stack: self.call_stack.clone(),
            });
        }
        if self.is_max_depth() {
            return Err(SubMacroError::MaxDepthExceeded {
                depth: self.call_stack.len(),
                max: MAX_CALL_DEPTH,
            });
        }
        self.call_stack.push(macro_name.to_string());
        Ok(())
    }

    /// Pop a macro from the call stack.
    pub fn pop(&mut self) {
        self.call_stack.pop();
    }

    /// Get the current call stack.
    pub fn call_stack(&self) -> &[String] {
        &self.call_stack
    }

    /// Get the current call depth.
    pub fn depth(&self) -> usize {
        self.call_stack.len()
    }

    /// Load a profile by name (with caching).
    pub fn load_profile(&mut self, name: &str) -> Result<&Profile, SubMacroError> {
        if !self.profile_cache.contains_key(name) {
            let profile = load_profile(name).map_err(|e| SubMacroError::LoadError {
                macro_name: name.to_string(),
                error: e.to_string(),
            })?;
            self.profile_cache.insert(name.to_string(), profile);
        }
        Ok(self.profile_cache.get(name).unwrap())
    }

    /// Clear the profile cache.
    pub fn clear_cache(&mut self) {
        self.profile_cache.clear();
    }
}

impl Default for SubMacroContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe sub-macro context handle.
pub type SubMacroContextHandle = Arc<Mutex<SubMacroContext>>;

/// Create a new thread-safe sub-macro context.
pub fn create_submacro_context() -> SubMacroContextHandle {
    Arc::new(Mutex::new(SubMacroContext::new()))
}

/// Prepare arguments for a sub-macro call.
///
/// Converts DslValue arguments to VariableValue, evaluating `{{ ... }}` templates against the
/// caller's scope. An argument that is exactly one template keeps its type (`{{ n * 2 }}` is a
/// number); text mixing literals and templates becomes a string. A template that cannot be
/// evaluated is an error naming the argument, not a silent `0`.
pub fn prepare_submacro_args(
    args: &HashMap<String, DslValue>,
    parent_vars: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<HashMap<String, VariableValue>, SubMacroError> {
    let mut result = HashMap::new();

    for (key, value) in args {
        let resolved = match value {
            DslValue::Int(n) => VariableValue::Number(*n as f64),
            DslValue::Float(f) => VariableValue::Number(*f),
            DslValue::Bool(b) => VariableValue::Boolean(*b),
            DslValue::String(s) => resolve_typed_value(s, parent_vars, engine)
                .map_err(|e| SubMacroError::VariableError(format!("argument `{key}`: {e}")))?,
        };
        result.insert(key.clone(), resolved);
    }

    Ok(result)
}

/// Create a child variable store for sub-macro execution.
/// The child inherits the parent's variables but can override them with args.
pub fn create_child_variable_store(
    parent: &VariableStore,
    profile: &MacroDocument,
    args: HashMap<String, VariableValue>,
) -> VariableStore {
    let mut child = parent.clone();

    // Initialize with profile's variable definitions
    child.init_from_definitions(&profile.variables);

    // Override with provided arguments
    child.set_variables(args);

    child
}

/// Sub-macro related errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SubMacroError {
    #[error("Circular macro call detected: {macro_name}. Call stack: {call_stack:?}")]
    CircularCall {
        macro_name: String,
        call_stack: Vec<String>,
    },
    #[error("Maximum call depth exceeded: {depth}/{max}")]
    MaxDepthExceeded { depth: usize, max: usize },
    #[error("Failed to load macro '{macro_name}': {error}")]
    LoadError { macro_name: String, error: String },
    #[error("Macro not found: {0}")]
    NotFound(String),
    #[error("Variable error in sub-macro: {0}")]
    VariableError(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_call_stack() {
        let mut ctx = SubMacroContext::new();

        assert!(ctx.push("macro1").is_ok());
        assert!(ctx.push("macro2").is_ok());
        assert_eq!(ctx.depth(), 2);

        // Circular call should fail
        assert!(ctx.push("macro1").is_err());

        ctx.pop();
        assert_eq!(ctx.depth(), 1);
    }

    #[test]
    fn test_max_depth() {
        let mut ctx = SubMacroContext::new();

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

        let result = prepare_submacro_args(&args, &parent, &ExpressionEngine::new()).unwrap();

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

        let result = prepare_submacro_args(&args, &parent, &ExpressionEngine::new()).unwrap();

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

        let err = prepare_submacro_args(&args, &parent, &ExpressionEngine::new()).unwrap_err();

        assert!(matches!(err, SubMacroError::VariableError(_)));
        assert!(err.to_string().contains("argument `n`"), "{err}");
    }
}
