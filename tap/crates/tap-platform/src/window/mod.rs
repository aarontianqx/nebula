//! Window API for detecting and interacting with OS windows.
//!
//! Provides functionality for:
//! - Getting foreground window information
//! - Finding windows by title or process name
//! - Getting window rectangles for relative coordinate support
//!
//! Platform implementations:
//! - Windows: Uses Win32 API (`windows.rs`)
//! - macOS: Stub implementation (`macos.rs`, TODO)

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// Information about a window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window handle (platform-specific identifier).
    pub handle: usize,
    /// Window title.
    pub title: String,
    /// Process name (e.g., "notepad.exe").
    pub process_name: String,
    /// Process ID.
    pub pid: u32,
    /// Window rectangle.
    pub rect: WindowRect,
    /// Whether the window is visible.
    pub visible: bool,
}

/// Window rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    /// Convert relative coordinates to absolute screen coordinates.
    pub fn to_absolute(&self, rel_x: i32, rel_y: i32) -> (i32, i32) {
        (self.x + rel_x, self.y + rel_y)
    }
}

/// Get the currently focused foreground window.
pub fn get_foreground_window() -> Option<WindowInfo> {
    #[cfg(windows)]
    {
        windows::get_foreground_window()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_window()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Find a window by its title (partial match, case-insensitive).
pub fn find_window_by_title(title: &str) -> Option<WindowInfo> {
    let title_lower = title.to_lowercase();
    list_windows()
        .into_iter()
        .find(|w| w.title.to_lowercase().contains(&title_lower))
}

/// Find a window by process name (partial match, case-insensitive).
pub fn find_window_by_process(process: &str) -> Option<WindowInfo> {
    let process_lower = process.to_lowercase();
    list_windows()
        .into_iter()
        .find(|w| w.process_name.to_lowercase().contains(&process_lower))
}

/// List all visible windows.
pub fn list_windows() -> Vec<WindowInfo> {
    #[cfg(windows)]
    {
        windows::list_windows()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_windows()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Get the rectangle of a window by handle.
pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
    #[cfg(windows)]
    {
        windows::get_window_rect(handle)
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_window_rect(handle)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = handle;
        None
    }
}

/// Check if a window with the given title is currently focused.
pub fn is_window_focused(title: Option<&str>, process: Option<&str>) -> bool {
    let Some(fg) = get_foreground_window() else {
        return false;
    };

    let title_match = title
        .map(|t| fg.title.to_lowercase().contains(&t.to_lowercase()))
        .unwrap_or(true);

    let process_match = process
        .map(|p| fg.process_name.to_lowercase().contains(&p.to_lowercase()))
        .unwrap_or(true);

    title_match && process_match
}

/// Check if a window with the given title/process exists.
pub fn window_exists(title: Option<&str>, process: Option<&str>) -> bool {
    let windows = list_windows();

    windows.iter().any(|w| {
        let title_match = title
            .map(|t| w.title.to_lowercase().contains(&t.to_lowercase()))
            .unwrap_or(true);

        let process_match = process
            .map(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()))
            .unwrap_or(true);

        title_match && process_match
    })
}

