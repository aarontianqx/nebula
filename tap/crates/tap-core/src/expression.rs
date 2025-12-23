//! Expression evaluation using Rhai scripting engine.
//!
//! Provides:
//! - Safe expression evaluation in a sandboxed environment
//! - Variable/counter access within expressions
//! - Support for {{ expr }} syntax in DSL values

use rhai::{Dynamic, Engine, Scope};
use std::sync::Arc;

use crate::variables::VariableStore;

/// Expression evaluator using Rhai.
pub struct ExpressionEngine {
    engine: Engine,
}

impl ExpressionEngine {
    /// Create a new expression engine with sandboxed settings.
    pub fn new() -> Self {
        let mut engine = Engine::new();

        // Sandbox settings: disable dangerous operations
        engine.set_max_expr_depths(64, 64);
        engine.set_max_call_levels(32);
        engine.set_max_operations(10000);
        engine.set_max_modules(0); // No external modules
        engine.set_max_string_size(4096);
        engine.set_max_array_size(1000);
        engine.set_max_map_size(100);

        // Disable file system and network access by not registering those modules
        // Rhai by default doesn't have these, so we're safe

        Self { engine }
    }

    /// Evaluate an expression with the given variable store.
    pub fn evaluate(
        &self,
        expr: &str,
        variables: &VariableStore,
    ) -> Result<ExpressionResult, ExpressionError> {
        let mut scope = Scope::new();

        // Add all variables to scope
        for (key, value) in variables.all_variables() {
            match value {
                crate::variables::VariableValue::String(s) => {
                    scope.push(key.clone(), s.clone());
                }
                crate::variables::VariableValue::Number(n) => {
                    // Use i64 for integers, f64 for floats
                    if n.fract() == 0.0 {
                        scope.push(key.clone(), *n as i64);
                    } else {
                        scope.push(key.clone(), *n);
                    }
                }
                crate::variables::VariableValue::Boolean(b) => {
                    scope.push(key.clone(), *b);
                }
            }
        }

        // Add all counters to scope
        for (key, value) in variables.all_counters() {
            scope.push(key.clone(), value as i64);
        }

        // Evaluate the expression
        let result = self
            .engine
            .eval_with_scope::<Dynamic>(&mut scope, expr)
            .map_err(|e| ExpressionError::EvaluationError(e.to_string()))?;

        Ok(ExpressionResult::from_dynamic(result))
    }

    /// Evaluate an expression and return as i32.
    pub fn evaluate_to_i32(
        &self,
        expr: &str,
        variables: &VariableStore,
    ) -> Result<i32, ExpressionError> {
        let result = self.evaluate(expr, variables)?;
        result.as_i32().ok_or_else(|| {
            ExpressionError::TypeMismatch {
                expected: "integer".to_string(),
                got: result.type_name().to_string(),
            }
        })
    }

    /// Evaluate an expression and return as string.
    pub fn evaluate_to_string(
        &self,
        expr: &str,
        variables: &VariableStore,
    ) -> Result<String, ExpressionError> {
        let result = self.evaluate(expr, variables)?;
        Ok(result.as_string())
    }

    /// Evaluate an expression and return as bool.
    pub fn evaluate_to_bool(
        &self,
        expr: &str,
        variables: &VariableStore,
    ) -> Result<bool, ExpressionError> {
        let result = self.evaluate(expr, variables)?;
        result.as_bool().ok_or_else(|| {
            ExpressionError::TypeMismatch {
                expected: "boolean".to_string(),
                got: result.type_name().to_string(),
            }
        })
    }
}

impl Default for ExpressionEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of expression evaluation.
#[derive(Debug, Clone)]
pub enum ExpressionResult {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Unit,
}

impl ExpressionResult {
    /// Convert from Rhai Dynamic.
    pub fn from_dynamic(d: Dynamic) -> Self {
        if d.is::<i64>() {
            ExpressionResult::Integer(d.cast::<i64>())
        } else if d.is::<f64>() {
            ExpressionResult::Float(d.cast::<f64>())
        } else if d.is::<bool>() {
            ExpressionResult::Boolean(d.cast::<bool>())
        } else if d.is::<String>() {
            ExpressionResult::String(d.cast::<String>())
        } else if d.is_unit() {
            ExpressionResult::Unit
        } else {
            // Convert other types to string
            ExpressionResult::String(d.to_string())
        }
    }

    /// Get the type name.
    pub fn type_name(&self) -> &str {
        match self {
            ExpressionResult::Integer(_) => "integer",
            ExpressionResult::Float(_) => "float",
            ExpressionResult::String(_) => "string",
            ExpressionResult::Boolean(_) => "boolean",
            ExpressionResult::Unit => "unit",
        }
    }

    /// Convert to i32.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            ExpressionResult::Integer(n) => Some(*n as i32),
            ExpressionResult::Float(f) => Some(*f as i32),
            ExpressionResult::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Convert to f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ExpressionResult::Integer(n) => Some(*n as f64),
            ExpressionResult::Float(f) => Some(*f),
            ExpressionResult::String(s) => s.parse().ok(),
            _ => None,
        }
    }

    /// Convert to string.
    pub fn as_string(&self) -> String {
        match self {
            ExpressionResult::Integer(n) => n.to_string(),
            ExpressionResult::Float(f) => f.to_string(),
            ExpressionResult::String(s) => s.clone(),
            ExpressionResult::Boolean(b) => b.to_string(),
            ExpressionResult::Unit => "()".to_string(),
        }
    }

    /// Convert to bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ExpressionResult::Boolean(b) => Some(*b),
            ExpressionResult::Integer(n) => Some(*n != 0),
            ExpressionResult::Float(f) => Some(*f != 0.0),
            ExpressionResult::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            ExpressionResult::Unit => Some(false),
        }
    }
}

/// Expression evaluation errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ExpressionError {
    #[error("Expression evaluation error: {0}")]
    EvaluationError(String),
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },
    #[error("Invalid expression syntax: {0}")]
    SyntaxError(String),
}

/// Resolve expressions in a string.
/// Expressions are enclosed in {{ }}.
/// Simple variable references ({{ var }}) are resolved directly.
/// Complex expressions ({{ var + 1 }}) are evaluated using Rhai.
pub fn resolve_expressions(
    s: &str,
    variables: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<String, ExpressionError> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next(); // consume second '{'

            // Find the closing }}
            let mut expr = String::new();
            let mut found_close = false;

            while let Some(c2) = chars.next() {
                if c2 == '}' && chars.peek() == Some(&'}') {
                    chars.next(); // consume second '}'
                    found_close = true;
                    break;
                }
                expr.push(c2);
            }

            if !found_close {
                return Err(ExpressionError::SyntaxError(format!(
                    "Unclosed expression: {}",
                    expr
                )));
            }

            let expr = expr.trim();

            // Check if it's a simple variable reference
            if is_simple_identifier(expr) {
                // Try variable first, then counter
                if let Some(value) = variables.get_variable(expr) {
                    result.push_str(&value.as_string());
                } else {
                    let counter = variables.get_counter(expr);
                    result.push_str(&counter.to_string());
                }
            } else {
                // Evaluate as expression
                let eval_result = engine.evaluate(expr, variables)?;
                result.push_str(&eval_result.as_string());
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}

/// Check if a string is a simple identifier (alphanumeric + underscore).
fn is_simple_identifier(s: &str) -> bool {
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

/// Thread-safe expression engine handle.
pub type ExpressionEngineHandle = Arc<ExpressionEngine>;

/// Create a new thread-safe expression engine.
pub fn create_expression_engine() -> ExpressionEngineHandle {
    Arc::new(ExpressionEngine::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_arithmetic() {
        let engine = ExpressionEngine::new();
        let vars = VariableStore::new();

        let result = engine.evaluate("1 + 2", &vars).unwrap();
        assert_eq!(result.as_i32(), Some(3));

        let result = engine.evaluate("10 * 5", &vars).unwrap();
        assert_eq!(result.as_i32(), Some(50));
    }

    #[test]
    fn test_variable_access() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 10);
        vars.set_variable("y", 20);

        let result = engine.evaluate("x + y", &vars).unwrap();
        assert_eq!(result.as_i32(), Some(30));
    }

    #[test]
    fn test_counter_access() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_counter("count", 5);

        let result = engine.evaluate("count * 2", &vars).unwrap();
        assert_eq!(result.as_i32(), Some(10));
    }

    #[test]
    fn test_string_concat() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("name", "World");

        let result = engine.evaluate_to_string("\"Hello, \" + name", &vars).unwrap();
        assert_eq!(result, "Hello, World");
    }

    #[test]
    fn test_boolean_expression() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 10);

        let result = engine.evaluate_to_bool("x > 5", &vars).unwrap();
        assert!(result);

        let result = engine.evaluate_to_bool("x < 5", &vars).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_resolve_expressions() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 100);
        vars.set_counter("count", 5);

        let result = resolve_expressions("Click at {{ x }}, count: {{ count + 1 }}", &vars, &engine)
            .unwrap();
        assert_eq!(result, "Click at 100, count: 6");
    }

    #[test]
    fn test_simple_identifier() {
        assert!(is_simple_identifier("my_var"));
        assert!(is_simple_identifier("x"));
        assert!(is_simple_identifier("_private"));
        assert!(!is_simple_identifier("x + 1"));
        assert!(!is_simple_identifier("123"));
        assert!(!is_simple_identifier(""));
    }
}

