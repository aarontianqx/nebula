//! Sandboxed `{{ expr }}` evaluation (Rhai) over the run scope.

use rhai::{Dynamic, Engine, Scope};
use std::sync::Arc;

use crate::variables::{VariableStore, VariableValue};

pub struct ExpressionEngine {
    engine: Engine,
}

impl ExpressionEngine {
    /// Rhai ships without file or network access; the limits keep a runaway expression from
    /// stalling the run.
    pub fn new() -> Self {
        let mut engine = Engine::new();
        engine.set_max_expr_depths(64, 64);
        engine.set_max_call_levels(32);
        engine.set_max_operations(10000);
        engine.set_max_modules(0);
        engine.set_max_string_size(4096);
        engine.set_max_array_size(1000);
        engine.set_max_map_size(100);
        Self { engine }
    }

    pub fn evaluate(
        &self,
        expr: &str,
        variables: &VariableStore,
    ) -> Result<ExpressionResult, ExpressionError> {
        let mut scope = Scope::new();

        for (key, value) in variables.all_variables() {
            match value {
                VariableValue::String(s) => scope.push(key.clone(), s.clone()),
                // Whole numbers become integers so `x % 2` and `x / 2` behave as written.
                VariableValue::Number(n) if n.fract() == 0.0 => scope.push(key.clone(), *n as i64),
                VariableValue::Number(n) => scope.push(key.clone(), *n),
                VariableValue::Boolean(b) => scope.push(key.clone(), *b),
            };
        }
        for (key, value) in variables.all_counters() {
            scope.push(key, value as i64);
        }

        let result = self
            .engine
            .eval_with_scope::<Dynamic>(&mut scope, expr)
            .map_err(|e| ExpressionError::EvaluationError(e.to_string()))?;

        Ok(ExpressionResult::from_dynamic(result))
    }
}

impl Default for ExpressionEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum ExpressionResult {
    Integer(i64),
    Float(f64),
    String(String),
    Boolean(bool),
    Unit,
}

impl ExpressionResult {
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
            ExpressionResult::String(d.to_string())
        }
    }

    pub fn as_string(&self) -> String {
        match self {
            ExpressionResult::Integer(n) => n.to_string(),
            ExpressionResult::Float(f) => f.to_string(),
            ExpressionResult::String(s) => s.clone(),
            ExpressionResult::Boolean(b) => b.to_string(),
            ExpressionResult::Unit => "()".to_string(),
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

/// Resolve a template to a *typed* value.
///
/// A string that is exactly one `{{ ... }}` keeps the type of what it refers to or evaluates to
/// (a number stays a number, a boolean a boolean): `call_macro` arguments become the callee's
/// variables, where the type matters. Anything else (plain text, or text mixed with several
/// `{{ ... }}`) is resolved to a string with [`resolve_expressions`].
///
/// As in [`resolve_expressions`], a lone identifier refers to a variable first and a counter
/// second; everything else is a Rhai expression.
pub fn resolve_typed_value(
    s: &str,
    variables: &VariableStore,
    engine: &ExpressionEngine,
) -> Result<VariableValue, ExpressionError> {
    let Some(expr) = single_template(s) else {
        return resolve_expressions(s, variables, engine).map(VariableValue::String);
    };

    if is_simple_identifier(expr) {
        return Ok(match variables.get_variable(expr) {
            Some(value) => value.clone(),
            None => VariableValue::Number(f64::from(variables.get_counter(expr))),
        });
    }

    match engine.evaluate(expr, variables)? {
        ExpressionResult::Integer(n) => Ok(VariableValue::Number(n as f64)),
        ExpressionResult::Float(f) => Ok(VariableValue::Number(f)),
        ExpressionResult::Boolean(b) => Ok(VariableValue::Boolean(b)),
        ExpressionResult::String(text) => Ok(VariableValue::String(text)),
        ExpressionResult::Unit => Err(ExpressionError::TypeMismatch {
            expected: "a value".to_string(),
            got: "unit".to_string(),
        }),
    }
}

/// The trimmed inner text when `s` is exactly one `{{ ... }}` template.
fn single_template(s: &str) -> Option<&str> {
    let rest = s.trim().strip_prefix("{{")?;
    let inner = rest.strip_suffix("}}")?;
    // `{{ a }} and {{ b }}` also starts with `{{` and ends with `}}`.
    if inner.contains("{{") || inner.contains("}}") {
        return None;
    }
    Some(inner.trim())
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

        assert_eq!(engine.evaluate("1 + 2", &vars).unwrap().as_string(), "3");
        assert_eq!(engine.evaluate("10 * 5", &vars).unwrap().as_string(), "50");
    }

    #[test]
    fn test_variable_access() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 10);
        vars.set_variable("y", 20);

        assert_eq!(engine.evaluate("x + y", &vars).unwrap().as_string(), "30");
    }

    #[test]
    fn test_counter_access() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_counter("count", 5);

        assert_eq!(
            engine.evaluate("count * 2", &vars).unwrap().as_string(),
            "10"
        );
    }

    #[test]
    fn test_string_concat() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("name", "World");

        let result = engine.evaluate("\"Hello, \" + name", &vars).unwrap();
        assert_eq!(result.as_string(), "Hello, World");
    }

    #[test]
    fn test_boolean_expression() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 10);

        assert!(matches!(
            engine.evaluate("x > 5", &vars).unwrap(),
            ExpressionResult::Boolean(true)
        ));
        assert!(matches!(
            engine.evaluate("x < 5", &vars).unwrap(),
            ExpressionResult::Boolean(false)
        ));
    }

    #[test]
    fn test_resolve_expressions() {
        let engine = ExpressionEngine::new();
        let mut vars = VariableStore::new();
        vars.set_variable("x", 100);
        vars.set_counter("count", 5);

        let result =
            resolve_expressions("Click at {{ x }}, count: {{ count + 1 }}", &vars, &engine)
                .unwrap();
        assert_eq!(result, "Click at 100, count: 6");
    }

    fn typed_scope() -> (ExpressionEngine, VariableStore) {
        let mut vars = VariableStore::new();
        vars.set_variable("x", 100);
        vars.set_variable("name", "Ada");
        vars.set_variable("flag", true);
        vars.set_counter("count", 5);
        (ExpressionEngine::new(), vars)
    }

    #[test]
    fn typed_value_keeps_the_type_of_a_lone_template() {
        let (engine, vars) = typed_scope();
        let typed = |s: &str| resolve_typed_value(s, &vars, &engine).unwrap();

        assert!(matches!(typed("{{ x }}"), VariableValue::Number(n) if n == 100.0));
        assert!(matches!(typed("{{ flag }}"), VariableValue::Boolean(true)));
        assert!(matches!(typed("{{ name }}"), VariableValue::String(s) if s == "Ada"));
        // A lone identifier that is no variable is a counter.
        assert!(matches!(typed("{{ count }}"), VariableValue::Number(n) if n == 5.0));
        assert!(matches!(typed("  {{x}}  "), VariableValue::Number(n) if n == 100.0));
    }

    #[test]
    fn typed_value_evaluates_expressions() {
        let (engine, vars) = typed_scope();
        let typed = |s: &str| resolve_typed_value(s, &vars, &engine).unwrap();

        assert!(matches!(typed("{{ x * 2 }}"), VariableValue::Number(n) if n == 200.0));
        assert!(matches!(typed("{{ count + 1.5 }}"), VariableValue::Number(n) if n == 6.5));
        assert!(matches!(typed("{{ x > 5 }}"), VariableValue::Boolean(true)));
        assert!(matches!(typed("{{ \"hi \" + name }}"), VariableValue::String(s) if s == "hi Ada"));
    }

    #[test]
    fn typed_value_turns_everything_else_into_text() {
        let (engine, vars) = typed_scope();
        let typed = |s: &str| resolve_typed_value(s, &vars, &engine).unwrap();

        assert!(matches!(typed("plain"), VariableValue::String(s) if s == "plain"));
        assert!(matches!(typed("x = {{ x }}"), VariableValue::String(s) if s == "x = 100"));
        // Two templates are text, not one expression spanning both.
        assert!(
            matches!(typed("{{ x }} and {{ count }}"), VariableValue::String(s) if s == "100 and 5")
        );
    }

    #[test]
    fn typed_value_reports_what_cannot_be_resolved() {
        let (engine, vars) = typed_scope();

        assert!(resolve_typed_value("{{ x +* }}", &vars, &engine).is_err());
        assert!(resolve_typed_value("{{ x", &vars, &engine).is_err());
        // An expression with no value is an error, not an empty string.
        assert!(resolve_typed_value("{{ () }}", &vars, &engine).is_err());
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
