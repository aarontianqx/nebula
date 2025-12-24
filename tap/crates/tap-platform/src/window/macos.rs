//! macOS implementation of window API.
//!
//! TODO: Implement using Accessibility API or CGWindowListCopyWindowInfo.

use super::{WindowInfo, WindowRect};

/// Get the currently focused foreground window.
pub fn get_foreground_window() -> Option<WindowInfo> {
    // TODO: Implement using Accessibility API
    None
}

/// List all visible windows.
pub fn list_windows() -> Vec<WindowInfo> {
    // TODO: Implement using CGWindowListCopyWindowInfo
    Vec::new()
}

/// Get the rectangle of a window by handle.
pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
    let _ = handle;
    // TODO: Implement
    None
}

