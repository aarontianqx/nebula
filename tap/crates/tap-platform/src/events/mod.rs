//! Global event listening module.
//!
//! This module provides platform-specific implementations for global input event listening.
//!
//! - On macOS: Uses native Core Graphics CGEventTap API (singleton pattern)
//! - On Windows/Linux: Uses rdev crate with singleton pattern to avoid multiple listener issues

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(not(target_os = "macos"))]
mod rdev;

#[cfg(not(target_os = "macos"))]
pub use rdev::*;
