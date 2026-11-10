use std::sync::Once;
use tracing::{info, warn};

static INIT: Once = Once::new();

/// Opts into Per-Monitor V2 so the system hands out physical (unscaled) coordinates. Must run
/// before any window is created.
pub fn set_dpi_aware() {
    INIT.call_once(|| {
        // Per-Monitor V2 needs Windows 10 1703+.
        unsafe {
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

/// Injection is in physical pixels while the browser reports CSS pixels, so the factor is the
/// system DPI scale.
pub fn browser_to_injection_scale() -> f64 {
    unsafe {
        #[link(name = "user32")]
        extern "system" {
            fn GetDpiForSystem() -> u32;
        }

        GetDpiForSystem() as f64 / 96.0
    }
}
