//! OCR configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// OCR service configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrConfig {
    /// Whether OCR is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,

    /// Base URL of the OCR service
    #[serde(default = "default_base_url")]
    pub base_url: String,

    /// Request timeout in seconds
    #[serde(default = "default_timeout_sec")]
    pub timeout_sec: u64,

    /// Health check interval in seconds
    #[serde(default = "default_health_interval_sec")]
    pub health_interval_sec: u64,

    /// Health check timeout in seconds
    #[serde(default = "default_health_timeout_sec")]
    pub health_timeout_sec: u64,
}

fn default_enabled() -> bool {
    true
}

fn default_base_url() -> String {
    "http://localhost:8000".to_string()
}

fn default_timeout_sec() -> u64 {
    30
}

fn default_health_interval_sec() -> u64 {
    5
}

fn default_health_timeout_sec() -> u64 {
    3
}

impl Default for OcrConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            base_url: default_base_url(),
            timeout_sec: default_timeout_sec(),
            health_interval_sec: default_health_interval_sec(),
            health_timeout_sec: default_health_timeout_sec(),
        }
    }
}

impl OcrConfig {
    /// Get request timeout as Duration
    pub fn timeout(&self) -> Duration {
        Duration::from_secs(self.timeout_sec)
    }

    /// Get health check interval as Duration
    pub fn health_interval(&self) -> Duration {
        Duration::from_secs(self.health_interval_sec)
    }

    /// Get health check timeout as Duration
    pub fn health_timeout(&self) -> Duration {
        Duration::from_secs(self.health_timeout_sec)
    }
}
