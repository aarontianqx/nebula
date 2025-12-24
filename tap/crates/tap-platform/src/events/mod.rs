//! Global event listening module.
//!
//! This module provides platform-specific implementations for global input event listening.
//!
//! - On macOS: Uses native Core Graphics CGEventTap API (singleton pattern)
//! - On Windows/Linux: Uses rdev crate (planned)

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

// Re-export common types (defined in the macOS module for now)
// When we add rdev_impl, we'll move common types here

