//! The run scope: counters and variables that `{{ }}` templates and conditions read.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::{VariableDefinition, VariableType};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableStore {
    counters: HashMap<String, i32>,
    variables: HashMap<String, VariableValue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VariableValue {
    String(String),
    Number(f64),
    Boolean(bool),
}

impl VariableValue {
    pub fn as_string(&self) -> String {
        match self {
            VariableValue::String(s) => s.clone(),
            VariableValue::Number(n) if n.fract() == 0.0 => format!("{}", *n as i64),
            VariableValue::Number(n) => format!("{}", n),
            VariableValue::Boolean(b) => b.to_string(),
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
    pub fn new() -> Self {
        Self::default()
    }

    /// A counter that was never set reads as 0.
    pub fn get_counter(&self, key: &str) -> i32 {
        self.counters.get(key).copied().unwrap_or(0)
    }

    pub fn set_counter(&mut self, key: &str, value: i32) {
        self.counters.insert(key.to_string(), value);
    }

    /// Returns the new value.
    pub fn incr_counter(&mut self, key: &str) -> i32 {
        let value = self.get_counter(key) + 1;
        self.counters.insert(key.to_string(), value);
        value
    }

    /// Returns the new value.
    pub fn decr_counter(&mut self, key: &str) -> i32 {
        let value = self.get_counter(key) - 1;
        self.counters.insert(key.to_string(), value);
        value
    }

    pub fn reset_counter(&mut self, key: &str) {
        self.counters.insert(key.to_string(), 0);
    }

    pub fn all_counters(&self) -> HashMap<String, i32> {
        self.counters.clone()
    }

    pub fn get_variable(&self, key: &str) -> Option<&VariableValue> {
        self.variables.get(key)
    }

    pub fn set_variable<V: Into<VariableValue>>(&mut self, key: &str, value: V) {
        self.variables.insert(key.to_string(), value.into());
    }

    pub fn all_variables(&self) -> &HashMap<String, VariableValue> {
        &self.variables
    }

    /// Seed every declared variable with its default (or the type's zero value).
    pub fn init_from_definitions(&mut self, definitions: &HashMap<String, VariableDefinition>) {
        for (name, def) in definitions {
            let value = match &def.default {
                Some(json_val) => match def.var_type {
                    VariableType::String => {
                        VariableValue::String(json_val.as_str().unwrap_or("").to_string())
                    }
                    VariableType::Number => VariableValue::Number(json_val.as_f64().unwrap_or(0.0)),
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

    pub fn set_variables(&mut self, vars: HashMap<String, VariableValue>) {
        self.variables.extend(vars);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_at_zero_and_follow_every_operation() {
        let mut store = VariableStore::new();
        assert_eq!(store.get_counter("test"), 0);

        store.set_counter("test", 10);
        assert_eq!(store.incr_counter("test"), 11);
        assert_eq!(store.decr_counter("test"), 10);

        store.reset_counter("test");
        assert_eq!(store.get_counter("test"), 0);
    }

    #[test]
    fn variables_keep_their_type_and_print_without_trailing_zeros() {
        let mut store = VariableStore::new();
        assert!(store.get_variable("key").is_none());

        store.set_variable("str", "hello");
        store.set_variable("num", 42);
        store.set_variable("float", 1.5);
        store.set_variable("bool", true);

        let printed = |key: &str| store.get_variable(key).unwrap().as_string();
        assert_eq!(printed("str"), "hello");
        assert_eq!(printed("num"), "42");
        assert_eq!(printed("float"), "1.5");
        assert_eq!(printed("bool"), "true");
    }
}
