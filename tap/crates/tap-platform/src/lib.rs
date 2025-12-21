//! tap-platform: platform-specific I/O boundary for tap.
//!
//! This crate defines traits for hooking global input and injecting actions,
//! and provides OS-specific implementations (Win/mac) via `enigo`.

mod injector;

pub use injector::{EnigoInjector, InputInjector, NoopInjector};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("not implemented")]
    NotImplemented,
    #[error("injection failed: {0}")]
    InjectionFailed(String),
    #[error("invalid key: {0}")]
    InvalidKey(String),
}

pub type PlatformResult<T> = Result<T, PlatformError>;
