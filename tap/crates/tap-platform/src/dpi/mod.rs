//! DPI / coordinate-system utilities for high-resolution display support.
//!
//! ## Canonical injection coordinate space
//!
//! Each platform has a single coordinate space shared by injection (`enigo`),
//! recording (`rdev`/native event tap), window bounds and pixel capture:
//!
//! - **Windows**: physical pixels (the process opts into Per-Monitor V2 DPI
//!   awareness so the system stops scaling coordinates).
//! - **macOS**: points (the window server abstracts physical pixels away).
//!
//! Browser overlays (the picker) report positions in CSS pixels via
//! `window.screenX/Y`. [`browser_to_injection_scale`] converts those into the
//! canonical injection space: the DPI scale on Windows, `1.0` on macOS.
//!
//! [`get_primary_scale_factor`] reports the display's backing scale factor and
//! is informational only -- do not use it to convert picker coordinates.

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

// Re-export platform-specific functions
#[cfg(windows)]
pub use windows::{browser_to_injection_scale, get_primary_scale_factor, set_dpi_aware};

#[cfg(target_os = "macos")]
pub use macos::{browser_to_injection_scale, get_primary_scale_factor, set_dpi_aware};

#[cfg(not(any(windows, target_os = "macos")))]
mod fallback {
    /// Set DPI awareness (no-op on unsupported platforms).
    pub fn set_dpi_aware() {}

    /// Get the current DPI scale factor.
    pub fn get_primary_scale_factor() -> f64 {
        1.0
    }

    /// Factor to convert browser CSS pixels into injection coordinates.
    pub fn browser_to_injection_scale() -> f64 {
        1.0
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
pub use fallback::{browser_to_injection_scale, get_primary_scale_factor, set_dpi_aware};

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
