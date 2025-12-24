//! Windows DPI awareness implementation.

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

