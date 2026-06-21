//! macOS DPI / coordinate handling.
//!
//! macOS exposes a single *point*-based coordinate space across the APIs this
//! crate uses: `CGEvent` injection (`enigo`), the recording event tap
//! (`rdev`/`CGEventTap`), `CGWindowList` bounds and `CGWindowListCreateImage`
//! all speak points (logical, density-independent). High-resolution ("Retina")
//! displays are handled transparently by the window server.
//!
//! Therefore the canonical injection coordinate space on macOS is *points*, and
//! no scaling is needed to convert browser CSS pixels (also points) into
//! injection coordinates -- see [`browser_to_injection_scale`].
//!
//! [`get_primary_scale_factor`] still reports the true backing scale of the
//! main display (e.g. 2.0 on Retina) for diagnostics and density-aware UI.

use core_graphics::display::CGDisplay;

/// Set DPI awareness (no-op on macOS).
///
/// macOS handles display scaling automatically; there is no per-process DPI
/// awareness to opt into like on Windows.
pub fn set_dpi_aware() {
    // No-op: macOS uses a point-based coordinate system end-to-end.
}

/// Backing scale factor of the main display (2.0 on Retina, 1.0 otherwise).
///
/// Derived from the active display mode's pixel width versus its point width,
/// which avoids a dependency on AppKit/`NSScreen`.
pub fn get_primary_scale_factor() -> f64 {
    if let Some(mode) = CGDisplay::main().display_mode() {
        let points = mode.width();
        if points > 0 {
            return mode.pixel_width() as f64 / points as f64;
        }
    }
    1.0
}

/// Factor to convert browser CSS pixels (`window.screenX/Y`) into the OS
/// injection coordinate space.
///
/// On macOS both spaces are points, so the factor is always `1.0`.
pub fn browser_to_injection_scale() -> f64 {
    1.0
}
