use serde::Deserialize;

/// Keyboard passthrough gesture configuration
#[derive(Debug, Clone, Deserialize)]
pub struct KeyboardConfig {
    /// Long press detection threshold in milliseconds
    pub long_press_threshold_ms: u64,
    /// Long press repeat click interval in milliseconds
    pub repeat_interval_ms: u64,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            long_press_threshold_ms: 300,
            repeat_interval_ms: 100,
        }
    }
}
