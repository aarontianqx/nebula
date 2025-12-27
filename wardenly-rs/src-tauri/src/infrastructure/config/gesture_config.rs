use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Gesture recognition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GestureConfig {
    pub keyboard_passthrough: KeyboardPassthroughConfig,
}

/// Keyboard passthrough configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeyboardPassthroughConfig {
    /// Time before a press is considered a long press (milliseconds)
    pub long_press_threshold_ms: u64,

    /// Interval between repeated clicks during long press (milliseconds)
    pub repeat_interval_ms: u64,

    /// Debounce window to prevent duplicate events (milliseconds)
    pub debounce_window_ms: u64,
}

impl Default for GestureConfig {
    fn default() -> Self {
        Self {
            keyboard_passthrough: KeyboardPassthroughConfig::default(),
        }
    }
}

impl Default for KeyboardPassthroughConfig {
    fn default() -> Self {
        Self {
            long_press_threshold_ms: 300,
            repeat_interval_ms: 100,
            debounce_window_ms: 50,
        }
    }
}

impl KeyboardPassthroughConfig {
    /// Get long press threshold as Duration
    pub fn long_press_threshold(&self) -> Duration {
        Duration::from_millis(self.long_press_threshold_ms)
    }

    /// Get repeat interval as Duration
    pub fn repeat_interval(&self) -> Duration {
        Duration::from_millis(self.repeat_interval_ms)
    }

    /// Get debounce window as Duration
    pub fn debounce_window(&self) -> Duration {
        Duration::from_millis(self.debounce_window_ms)
    }
}

