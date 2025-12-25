//! Global event listening module.
//!
//! This module provides platform-specific implementations for global input event listening.
//!
//! ## Platform Implementations
//!
//! - **macOS**: Uses native Core Graphics CGEventTap API (singleton pattern)
//!   - Required because rdev has thread-safety issues with TSMGetInputSourceProperty
//!   - Provides `subscribe_events()` for a unified event stream
//!
//! - **Windows/Linux**: Does NOT use this module
//!   - The `input_hook` module uses platform-native APIs directly:
//!     - Windows: Raw Input API (`windows_native.rs`)
//!     - Linux: rdev (`rdev_impl.rs`)
//!   - The `mouse_tracker` module uses rdev directly

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::*;
