//! tap-platform: platform-specific I/O boundary for tap.
//!
//! This crate provides:
//! - Input injection (mouse/keyboard simulation) via `enigo`
//! - Global input hook for recording via `rdev` (or native macOS API on macOS)
//! - Global mouse position tracking via `rdev` (or native macOS API on macOS)
//! - DPI scaling utilities for high-resolution displays
//! - Window detection and management APIs
//! - Pixel color reading for condition evaluation
//!
//! ## Module Structure
//!
//! Each functional area is organized as a submodule with platform-specific implementations:
//!
//! - `error` - Common error types
//! - `injector` - Input injection (shared implementation using enigo)
//! - `events` - Global event listening (macOS native implementation)
//! - `input_hook` - Input event hooking for recording
//! - `mouse_tracker` - Mouse position tracking
//! - `window` - Window detection and management
//! - `pixel` - Pixel color reading
//! - `dpi` - DPI scaling utilities

mod dpi;
mod error;
mod events;
mod injector;
mod input_hook;
mod mouse_tracker;
mod permissions;
mod pixel;
mod window;

// Re-export error types
pub use error::{PlatformError, PlatformResult};

// Re-export DPI utilities
pub use dpi::{browser_to_injection_scale, set_dpi_aware};

// Re-export input injection
pub use injector::{EnigoInjector, InputInjector};

// Re-export input hook
pub use input_hook::{
    start_input_hook, InputEventType, InputHookHandle, MouseButtonType, RawInputEvent,
};

// Re-export mouse tracker
pub use mouse_tracker::{
    start_mouse_tracker, MouseTrackerCommand, MouseTrackerConfig, MouseTrackerEvent,
    MouseTrackerHandle,
};

// Re-export permission probing
pub use permissions::{
    check_permissions, open_permission_settings, request_screen_recording, PermissionStatus,
};

// Re-export pixel detection
pub use pixel::{get_pixel_color, Color};

// Re-export window API
pub use window::{
    get_foreground_window, is_window_focused, list_windows, window_exists, window_origin,
    WindowInfo,
};
