//! macOS speaks points end to end (`CGEvent` injection, the event tap, `CGWindowList` bounds and
//! the pixel capture), so there is nothing to opt into and nothing to scale.

pub fn set_dpi_aware() {}

/// Browser CSS pixels and injection coordinates are both points.
pub fn browser_to_injection_scale() -> f64 {
    1.0
}
