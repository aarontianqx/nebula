//! tap-platform: platform-specific I/O boundary for tap.
//!
//! This crate provides:
//! - Input injection (mouse/keyboard simulation) via `enigo`
//! - Global input hook for recording via `rdev`
//! - Global mouse position tracking via `rdev`
//! - DPI scaling utilities for high-resolution displays
//! - Window detection and management APIs
//! - Pixel color reading for condition evaluation

mod dpi;
mod injector;
mod input_hook;
mod mouse_tracker;
mod pixel;
mod window;

pub use dpi::{get_primary_scale_factor, set_dpi_aware, ScaledCoords};
pub use injector::{EnigoInjector, InputInjector, NoopInjector};
pub use input_hook::{
    start_input_hook, InputEventType, InputHookHandle, MouseButtonType, RawInputEvent,
};
pub use mouse_tracker::{
    start_mouse_tracker, MousePosition, MouseTrackerCommand, MouseTrackerConfig,
    MouseTrackerEvent, MouseTrackerHandle,
};
pub use pixel::{get_pixel_color, pixel_matches, Color};
pub use window::{
    find_window_by_process, find_window_by_title, get_foreground_window, get_window_rect,
    is_window_focused, list_windows, window_exists, WindowInfo, WindowRect,
};

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
