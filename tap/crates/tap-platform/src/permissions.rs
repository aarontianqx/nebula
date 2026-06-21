//! OS permission probing and guidance.
//!
//! macOS gates global input (hook + injection) behind the **Accessibility**
//! permission and screen capture (pixel reads, window titles) behind **Screen
//! Recording**. Without them, recording/replay/pixel features fail silently, so
//! the UI needs to detect the grant status and guide the user to System
//! Settings. On other platforms these capabilities are not permission-gated, so
//! the probes report `true`.

use serde::Serialize;

/// Snapshot of the OS permissions tap depends on.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct PermissionStatus {
    /// Global input hook + injection is allowed (macOS Accessibility; `true` where not applicable).
    pub accessibility: bool,
    /// Screen capture for pixel/window-title reads is allowed (macOS Screen Recording; `true` where not applicable).
    pub screen_recording: bool,
    /// Target OS, so the UI can show platform-specific guidance (e.g. the Windows admin hint).
    pub os: &'static str,
}

/// Probe the current permission grant status.
pub fn check_permissions() -> PermissionStatus {
    imp::check()
}

/// Prompt for the macOS Screen Recording permission (no-op elsewhere); returns the refreshed status.
pub fn request_screen_recording() -> PermissionStatus {
    imp::request_screen_recording()
}

/// Open the relevant OS settings pane for a permission (`"accessibility"` / `"screen_recording"`).
pub fn open_permission_settings(which: &str) -> Result<(), String> {
    imp::open_settings(which)
}

#[cfg(target_os = "macos")]
mod imp {
    use super::PermissionStatus;

    // `AXIsProcessTrusted` lives in ApplicationServices and returns a Carbon
    // `Boolean` (an `unsigned char`), so we bind it as `u8` and test `!= 0`.
    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> u8;
    }

    // Screen-capture preflight/request (macOS 10.15+), declared `bool` by Apple.
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }

    pub fn check() -> PermissionStatus {
        let accessibility = unsafe { AXIsProcessTrusted() != 0 };
        let screen_recording = unsafe { CGPreflightScreenCaptureAccess() };
        PermissionStatus {
            accessibility,
            screen_recording,
            os: "macos",
        }
    }

    pub fn request_screen_recording() -> PermissionStatus {
        // Triggers the system prompt the first time; subsequent calls are no-ops.
        unsafe {
            let _ = CGRequestScreenCaptureAccess();
        }
        check()
    }

    pub fn open_settings(which: &str) -> Result<(), String> {
        let url = match which {
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            "screen_recording" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture"
            }
            other => return Err(format!("unknown settings pane: {other}")),
        };
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::PermissionStatus;

    pub fn check() -> PermissionStatus {
        PermissionStatus {
            accessibility: true,
            screen_recording: true,
            os: if cfg!(target_os = "windows") {
                "windows"
            } else {
                "other"
            },
        }
    }

    pub fn request_screen_recording() -> PermissionStatus {
        check()
    }

    pub fn open_settings(_which: &str) -> Result<(), String> {
        Ok(())
    }
}
