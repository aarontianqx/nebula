//! The coordinate space injection, recording, window bounds and pixel capture all share.
//!
//! - **Windows**: physical pixels (the process opts into Per-Monitor V2 DPI awareness so the
//!   system stops scaling coordinates).
//! - **macOS**: points (the window server abstracts physical pixels away).
//!
//! Browser overlays (the picker) report CSS pixels from `window.screenX/Y`;
//! [`browser_to_injection_scale`] converts those into that space.

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{browser_to_injection_scale, set_dpi_aware};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::{browser_to_injection_scale, set_dpi_aware};

#[cfg(not(any(windows, target_os = "macos")))]
pub fn set_dpi_aware() {}

#[cfg(not(any(windows, target_os = "macos")))]
pub fn browser_to_injection_scale() -> f64 {
    1.0
}
