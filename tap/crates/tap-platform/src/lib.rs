//! tap-platform: platform-specific I/O boundary for tap.
//!
//! This crate defines traits for hooking global input and injecting actions,
//! and will provide OS-specific implementations (Win/mac) later.

use tap_core::Action;

#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("not implemented")]
    NotImplemented,
}

pub type PlatformResult<T> = Result<T, PlatformError>;

/// Inject mouse/keyboard actions into the OS.
pub trait InputInjector: Send + Sync {
    fn inject(&self, action: &Action) -> PlatformResult<()>;
}

/// Minimal no-op injector for early UI development.
pub struct NoopInjector;

impl InputInjector for NoopInjector {
    fn inject(&self, _action: &Action) -> PlatformResult<()> {
        Err(PlatformError::NotImplemented)
    }
}


