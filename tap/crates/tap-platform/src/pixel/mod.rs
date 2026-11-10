//! Pixel color detection for condition evaluation.
//!
//! Provides functionality for:
//! - Reading pixel color at screen coordinates
//!
//! Platform implementations:
//! - Windows: Uses GDI API (`windows.rs`)
//! - macOS: Uses `CGWindowListCreateImage` (`macos.rs`)

use tap_core::Color;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// Get the color of a pixel at the given screen coordinates.
pub fn get_pixel_color(x: i32, y: i32) -> Option<Color> {
    #[cfg(windows)]
    {
        windows::get_pixel_color(x, y)
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_pixel_color(x, y)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = (x, y);
        None
    }
}
