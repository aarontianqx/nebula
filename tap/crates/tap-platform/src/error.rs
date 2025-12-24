//! Common error types for tap-platform.

use thiserror::Error;

/// Platform-level errors.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("not implemented")]
    NotImplemented,
    #[error("injection failed: {0}")]
    InjectionFailed(String),
    #[error("invalid key: {0}")]
    InvalidKey(String),
}

/// Result type for platform operations.
pub type PlatformResult<T> = Result<T, PlatformError>;

