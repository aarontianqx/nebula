//! Variable and counter storage for Phase 3 & 4.
//!
//! Provides a simple key-value store for:
//! - Counters (integers that can be incremented/decremented)
//! - Variables (string/number/boolean values)
//! - Variable resolution (replacing {{ var }} references)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::dsl::{DslValue, VariableDefinition, VariableType};

/// Storage for variables and counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableStore {
    /// Integer counters.
    counters: HashMap<String, i32>,
    /// String variables.
    variables: HashMap<String, VariableValue>,
}

/// A typed variable value.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VariableValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl VariableValue {
    /// Convert to string representation.
    pub fn as_string(&self) -> String {
        match self {
            VariableValue::String(s) => s.clone(),
            VariableValue::Number(n) => {
                // Format without trailing zeros for integers
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            VariableValue::Boolean(b) => b.to_string(),
        }
    }

    /// Try to convert to i32.
    pub fn as_i32(&self) -> Option<i32> {
        match self {
            VariableValue::Number(n) => Some(*n as i32),
            VariableValue::String(s) => s.parse().ok(),
            VariableValue::Boolean(_) => None,
        }
    }

    /// Try to convert to f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            VariableValue::Number(n) => Some(*n),
            VariableValue::String(s) => s.parse().ok(),
            VariableValue::Boolean(_) => None,
        }
    }

    /// Try to convert to bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            VariableValue::Boolean(b) => Some(*b),
            VariableValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            VariableValue::Number(n) => Some(*n != 0.0),
        }
    }
}

impl From<&str> for VariableValue {
    fn from(s: &str) -> Self {
        VariableValue::String(s.to_string())
    }
}

impl From<String> for VariableValue {
    fn from(s: String) -> Self {
        VariableValue::String(s)
    }
}

impl From<i32> for VariableValue {
    fn from(n: i32) -> Self {
        VariableValue::Number(n as f64)
    }
}

impl From<f64> for VariableValue {
    fn from(n: f64) -> Self {
        VariableValue::Number(n)
    }
}

impl From<bool> for VariableValue {
    fn from(b: bool) -> Self {
        VariableValue::Boolean(b)
    }
}

impl VariableStore {
    /// Create a new empty variable store.
    pub fn new() -> Self {
        Self::default()
    }

    // === Counter operations ===

    /// Get a counter value. Returns 0 if the counter doesn't exist.
    pub fn get_counter(&self, key: &str) -> i32 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    /// Set a counter to a specific value.
    pub fn set_counter(&mut self, key: &str, value: i32) {
        self.counters.insert(key.to_string(), value);
    }

    /// Increment a counter by 1. Returns the new value.
    pub fn incr_counter(&mut self, key: &str) -> i32 {
        let value = self.get_counter(key) + 1;
        self.counters.insert(key.to_string(), value);
        value
    }

    /// Decrement a counter by 1. Returns the new value.
    pub fn decr_counter(&mut self, key: &str) -> i32 {
        let value = self.get_counter(key) - 1;
        self.counters.insert(key.to_string(), value);
        value
    }

    /// Add a value to a counter. Returns the new value.
    pub fn add_counter(&mut self, key: &str, delta: i32) -> i32 {
        let value = self.get_counter(key) + delta;
        self.counters.insert(key.to_string(), value);
        value
    }

    /// Reset a counter to 0.
    pub fn reset_counter(&mut self, key: &str) {
        self.counters.insert(key.to_string(), 0);
    }

    /// Remove a counter.
    pub fn remove_counter(&mut self, key: &str) {
        self.counters.remove(key);
    }

    /// Get all counter keys.
    pub fn counter_keys(&self) -> Vec<&str> {
        self.counters.keys().map(|s| s.as_str()).collect()
    }

    /// Get all counters as a snapshot.
    pub fn all_counters(&self) -> HashMap<String, i32> {
        self.counters.clone()
    }

    // === Variable operations ===

    /// Get a variable value.
    pub fn get_variable(&self, key: &str) -> Option<&VariableValue> {
        self.variables.get(key)
    }

    /// Get a variable as string.
    pub fn get_variable_string(&self, key: &str) -> Option<String> {
        self.variables.get(key).map(|v| v.as_string())
    }

    /// Set a variable value.
    pub fn set_variable<V: Into<VariableValue>>(&mut self, key: &str, value: V) {
        self.variables.insert(key.to_string(), value.into());
    }

    /// Remove a variable.
    pub fn remove_variable(&mut self, key: &str) {
        self.variables.remove(key);
    }

    /// Get all variable keys.
    pub fn variable_keys(&self) -> Vec<&str> {
        self.variables.keys().map(|s| s.as_str()).collect()
    }

    /// Get all variables as a snapshot.
    pub fn all_variables(&self) -> &HashMap<String, VariableValue> {
        &self.variables
    }

    // === Bulk operations ===

    /// Clear all counters and variables.
    pub fn clear(&mut self) {
        self.counters.clear();
        self.variables.clear();
    }

    /// Reset all counters to 0 but keep the keys.
    pub fn reset_all_counters(&mut self) {
        for value in self.counters.values_mut() {
            *value = 0;
        }
    }

    /// Initialize variables from definitions with default values.
    pub fn init_from_definitions(&mut self, definitions: &HashMap<String, VariableDefinition>) {
        for (name, def) in definitions {
            let value = match &def.default {
                Some(json_val) => match def.var_type {
                    VariableType::String => {
                        VariableValue::String(json_val.as_str().unwrap_or("").to_string())
                    }
                    VariableType::Number => {
                        VariableValue::Number(json_val.as_f64().unwrap_or(0.0))
                    }
                    VariableType::Boolean => {
                        VariableValue::Boolean(json_val.as_bool().unwrap_or(false))
                    }
                },
                None => match def.var_type {
                    VariableType::String => VariableValue::String(String::new()),
                    VariableType::Number => VariableValue::Number(0.0),
                    VariableType::Boolean => VariableValue::Boolean(false),
                },
            };
            self.variables.insert(name.clone(), value);
        }
    }

    /// Set multiple variables from a map.
    pub fn set_variables(&mut self, vars: HashMap<String, VariableValue>) {
        for (key, value) in vars {
            self.variables.insert(key, value);
        }
    }
}

// ============================================================================
// Variable Resolver
// ============================================================================

/// Resolves variable references in strings and DslValues.
pub struct VariableResolver<'a> {
    store: &'a VariableStore,
}

impl<'a> VariableResolver<'a> {
    /// Create a new resolver with the given variable store.
    pub fn new(store: &'a VariableStore) -> Self {
        Self { store }
    }

    /// Resolve variable references in a string.
    /// Variables are referenced as {{ var_name }}.
    pub fn resolve_string(&self, s: &str) -> Result<String, VariableError> {
        let mut result = String::new();
        let mut chars = s.chars().peekable();

        while let Some(c) = chars.next() {
            if c == '{' && chars.peek() == Some(&'{') {
                chars.next(); // consume second '{'
                
                // Find the closing }}
                let mut var_name = String::new();
                let mut found_close = false;
                
                while let Some(c2) = chars.next() {
                    if c2 == '}' && chars.peek() == Some(&'}') {
                        chars.next(); // consume second '}'
                        found_close = true;
                        break;
                    }
                    var_name.push(c2);
                }

                if !found_close {
                    return Err(VariableError::UnclosedReference(var_name));
                }

                let var_name = var_name.trim();
                
                // Try to get from variables first, then counters
                if let Some(value) = self.store.get_variable(var_name) {
                    result.push_str(&value.as_string());
                } else {
                    // Check if it's a counter
                    let counter_val = self.store.get_counter(var_name);
                    if counter_val != 0 || self.store.counters.contains_key(var_name) {
                        result.push_str(&counter_val.to_string());
                    } else {
                        return Err(VariableError::UndefinedVariable(var_name.to_string()));
                    }
                }
            } else {
                result.push(c);
            }
        }

        Ok(result)
    }

    /// Resolve a DslValue to its final value.
    pub fn resolve_dsl_value(&self, value: &DslValue) -> Result<DslValue, VariableError> {
        match value {
            DslValue::String(s) if s.contains("{{") && s.contains("}}") => {
                let resolved = self.resolve_string(s)?;
                // Try to parse as number
                if let Ok(n) = resolved.parse::<i64>() {
                    Ok(DslValue::Int(n))
                } else if let Ok(n) = resolved.parse::<f64>() {
                    Ok(DslValue::Float(n))
                } else if resolved == "true" {
                    Ok(DslValue::Bool(true))
                } else if resolved == "false" {
                    Ok(DslValue::Bool(false))
                } else {
                    Ok(DslValue::String(resolved))
                }
            }
            _ => Ok(value.clone()),
        }
    }

    /// Resolve a DslValue to i32.
    pub fn resolve_to_i32(&self, value: &DslValue) -> Result<i32, VariableError> {
        let resolved = self.resolve_dsl_value(value)?;
        resolved.as_i32().ok_or_else(|| {
            VariableError::TypeMismatch {
                expected: "number".to_string(),
                got: format!("{:?}", resolved),
            }
        })
    }

    /// Resolve a DslValue to string.
    pub fn resolve_to_string(&self, value: &DslValue) -> Result<String, VariableError> {
        let resolved = self.resolve_dsl_value(value)?;
        Ok(resolved.as_string())
    }
}

/// Variable resolution errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum VariableError {
    #[error("Undefined variable: {0}")]
    UndefinedVariable(String),
    #[error("Unclosed variable reference: {0}")]
    UnclosedReference(String),
    #[error("Type mismatch: expected {expected}, got {got}")]
    TypeMismatch { expected: String, got: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_operations() {
        let mut store = VariableStore::new();

        // Initial value should be 0
        assert_eq!(store.get_counter("test"), 0);

        // Set and get
        store.set_counter("test", 10);
        assert_eq!(store.get_counter("test"), 10);

        // Increment
        assert_eq!(store.incr_counter("test"), 11);
        assert_eq!(store.get_counter("test"), 11);

        // Decrement
        assert_eq!(store.decr_counter("test"), 10);

        // Add
        assert_eq!(store.add_counter("test", 5), 15);

        // Reset
        store.reset_counter("test");
        assert_eq!(store.get_counter("test"), 0);
    }

    #[test]
    fn test_variable_operations() {
        let mut store = VariableStore::new();

        assert!(store.get_variable("key").is_none());

        store.set_variable("key", "value");
        assert!(store.get_variable("key").is_some());
        assert_eq!(store.get_variable_string("key"), Some("value".to_string()));

        store.remove_variable("key");
        assert!(store.get_variable("key").is_none());
    }

    #[test]
    fn test_variable_value_types() {
        let mut store = VariableStore::new();

        store.set_variable("str", "hello");
        store.set_variable("num", 42);
        store.set_variable("float", 3.14);
        store.set_variable("bool", true);

        assert_eq!(store.get_variable_string("str"), Some("hello".to_string()));
        assert_eq!(store.get_variable_string("num"), Some("42".to_string()));
        assert_eq!(store.get_variable_string("bool"), Some("true".to_string()));
    }

    #[test]
    fn test_variable_resolver_simple() {
        let mut store = VariableStore::new();
        store.set_variable("name", "Alice");
        store.set_variable("age", 25);

        let resolver = VariableResolver::new(&store);

        assert_eq!(
            resolver.resolve_string("Hello, {{ name }}!").unwrap(),
            "Hello, Alice!"
        );
        assert_eq!(
            resolver.resolve_string("Age: {{ age }}").unwrap(),
            "Age: 25"
        );
    }

    #[test]
    fn test_variable_resolver_counter() {
        let mut store = VariableStore::new();
        store.set_counter("count", 10);

        let resolver = VariableResolver::new(&store);

        assert_eq!(
            resolver.resolve_string("Count: {{ count }}").unwrap(),
            "Count: 10"
        );
    }

    #[test]
    fn test_variable_resolver_undefined() {
        let store = VariableStore::new();
        let resolver = VariableResolver::new(&store);

        let result = resolver.resolve_string("{{ undefined }}");
        assert!(result.is_err());
    }

    #[test]
    fn test_variable_resolver_dsl_value() {
        let mut store = VariableStore::new();
        store.set_variable("x", 100);

        let resolver = VariableResolver::new(&store);

        let value = DslValue::String("{{ x }}".to_string());
        let resolved = resolver.resolve_dsl_value(&value).unwrap();
        assert!(matches!(resolved, DslValue::Int(100)));
    }
}

