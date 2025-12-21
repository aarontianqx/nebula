//! Variable and counter storage for Phase 3.
//!
//! Provides a simple key-value store for:
//! - Counters (integers that can be incremented/decremented)
//! - Variables (string values for future use)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Storage for variables and counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VariableStore {
    /// Integer counters.
    counters: HashMap<String, i32>,
    /// String variables (for future use).
    variables: HashMap<String, String>,
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

    // === Variable operations (for future use) ===

    /// Get a variable value.
    pub fn get_variable(&self, key: &str) -> Option<&str> {
        self.variables.get(key).map(|s| s.as_str())
    }

    /// Set a variable value.
    pub fn set_variable(&mut self, key: &str, value: &str) {
        self.variables.insert(key.to_string(), value.to_string());
    }

    /// Remove a variable.
    pub fn remove_variable(&mut self, key: &str) {
        self.variables.remove(key);
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
        assert_eq!(store.get_variable("key"), Some("value"));

        store.remove_variable("key");
        assert!(store.get_variable("key").is_none());
    }
}

