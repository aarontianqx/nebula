//! DPI scaling utilities for high-resolution display support.
//!
//! This module provides functions to handle DPI scaling on Windows,
//! ensuring that coordinates from rdev (which may be physical pixels)
//! are compatible with enigo (which may use logical pixels).
//!
//! On Windows, the behavior depends on the process's DPI awareness mode.
//! We set Per-Monitor V2 DPI awareness at process startup to ensure
//! we always work with physical (unscaled) coordinates.

#[cfg(target_os = "windows")]
mod windows_dpi {
    use std::sync::Once;
    use tracing::{info, warn};

    static INIT: Once = Once::new();

    /// Set the process DPI awareness to Per-Monitor V2.
    /// This ensures we receive physical (unscaled) coordinates from the system.
    ///
    /// Must be called early in the application lifecycle, before any window is created.
    pub fn set_dpi_aware() {
        INIT.call_once(|| {
            // Try to set Per-Monitor V2 DPI awareness (Windows 10 1703+)
            unsafe {
                // Define constants inline to avoid windows-sys dependency just for this
                const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;
                
                #[link(name = "user32")]
                extern "system" {
                    fn SetProcessDpiAwarenessContext(value: isize) -> i32;
                }

                let result = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
                if result != 0 {
                    info!("Set Per-Monitor V2 DPI awareness");
                } else {
                    warn!("Failed to set Per-Monitor V2 DPI awareness, coordinates may be scaled");
                }
            }
        });
    }

    /// Get the current DPI scale factor for the primary monitor.
    /// Returns 1.0 if DPI awareness is properly set, or the scale factor if not.
    pub fn get_primary_scale_factor() -> f64 {
        unsafe {
            #[link(name = "user32")]
            extern "system" {
                fn GetDpiForSystem() -> u32;
            }

            let dpi = GetDpiForSystem();
            dpi as f64 / 96.0
        }
    }
}

#[cfg(target_os = "windows")]
pub use windows_dpi::*;

#[cfg(not(target_os = "windows"))]
mod other_dpi {
    /// Set DPI awareness (no-op on non-Windows platforms).
    pub fn set_dpi_aware() {
        // macOS handles DPI scaling automatically through Retina display support
    }

    /// Get the current DPI scale factor.
    /// On macOS, this would be handled by the system, so we return 1.0.
    pub fn get_primary_scale_factor() -> f64 {
        1.0
    }
}

#[cfg(not(target_os = "windows"))]
pub use other_dpi::*;

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

