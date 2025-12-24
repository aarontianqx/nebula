//! macOS DPI handling implementation.
//!
//! macOS handles DPI scaling automatically through Retina display support.
//! The system abstracts physical vs logical pixels, so we don't need
//! manual DPI awareness like on Windows.

/// Set DPI awareness (no-op on macOS).
///
/// macOS handles DPI scaling automatically through Retina display support.
pub fn set_dpi_aware() {
    // No-op: macOS handles this automatically
}

/// Get the current DPI scale factor.
///
/// On macOS, this returns 1.0 as the system handles scaling transparently.
/// Note: For accurate per-display scale factors, use NSScreen.backingScaleFactor.
pub fn get_primary_scale_factor() -> f64 {
    // TODO: Could use NSScreen.backingScaleFactor for accurate per-display info
    1.0
}

