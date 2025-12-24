//! DPI scaling utilities for high-resolution display support.
//!
//! This module provides functions to handle DPI scaling,
//! ensuring that coordinates from rdev (which may be physical pixels)
//! are compatible with enigo (which may use logical pixels).
//!
//! Platform implementations:
//! - Windows: Uses Per-Monitor V2 DPI awareness (`windows.rs`)
//! - macOS: Relies on system-handled Retina display support (`macos.rs`)

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

// Re-export platform-specific functions
#[cfg(windows)]
pub use windows::{get_primary_scale_factor, set_dpi_aware};

#[cfg(target_os = "macos")]
pub use macos::{get_primary_scale_factor, set_dpi_aware};

#[cfg(not(any(windows, target_os = "macos")))]
mod fallback {
    /// Set DPI awareness (no-op on unsupported platforms).
    pub fn set_dpi_aware() {}

    /// Get the current DPI scale factor.
    pub fn get_primary_scale_factor() -> f64 {
        1.0
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub use fallback::{get_primary_scale_factor, set_dpi_aware};

/// Coordinates that can be converted between physical and logical pixels.
#[derive(Debug, Clone, Copy)]
pub struct ScaledCoords {
    /// X coordinate in physical pixels.
    pub physical_x: i32,
    /// Y coordinate in physical pixels.
    pub physical_y: i32,
    /// The scale factor used for conversion.
    pub scale_factor: f64,
}

impl ScaledCoords {
    /// Create scaled coordinates from physical pixels.
    pub fn from_physical(x: i32, y: i32) -> Self {
        Self {
            physical_x: x,
            physical_y: y,
            scale_factor: get_primary_scale_factor(),
        }
    }

    /// Convert to logical pixels (for use with scaled systems).
    pub fn to_logical(&self) -> (i32, i32) {
        (
            (self.physical_x as f64 / self.scale_factor).round() as i32,
            (self.physical_y as f64 / self.scale_factor).round() as i32,
        )
    }

    /// Get the physical coordinates.
    pub fn to_physical(&self) -> (i32, i32) {
        (self.physical_x, self.physical_y)
    }
}

