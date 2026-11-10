//! System queries: OS permissions, windows and pixels.

use tap_platform::{
    check_permissions, get_pixel_color, list_windows, open_permission_settings,
    request_screen_recording, Color, PermissionStatus, WindowInfo,
};

#[tauri::command]
pub fn cmd_check_permissions() -> PermissionStatus {
    check_permissions()
}

#[tauri::command]
pub fn cmd_request_screen_recording() -> PermissionStatus {
    request_screen_recording()
}

#[tauri::command]
pub fn cmd_open_permission_settings(which: String) -> Result<(), String> {
    open_permission_settings(&which)
}

/// Window info for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct WindowInfoResponse {
    pub handle: usize,
    pub title: String,
    pub process_name: String,
    pub pid: u32,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl From<WindowInfo> for WindowInfoResponse {
    fn from(w: WindowInfo) -> Self {
        Self {
            handle: w.handle,
            title: w.title,
            process_name: w.process_name,
            pid: w.pid,
            x: w.rect.x,
            y: w.rect.y,
            width: w.rect.width,
            height: w.rect.height,
        }
    }
}

/// Color info for frontend.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ColorResponse {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub hex: String,
}

impl From<Color> for ColorResponse {
    fn from(c: Color) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
            hex: c.to_hex(),
        }
    }
}

#[tauri::command]
pub fn cmd_list_windows() -> Vec<WindowInfoResponse> {
    list_windows().into_iter().map(|w| w.into()).collect()
}

#[tauri::command]
pub fn cmd_get_foreground_window() -> Option<WindowInfoResponse> {
    tap_platform::get_foreground_window().map(|w| w.into())
}

#[tauri::command]
pub fn cmd_get_pixel_color(x: i32, y: i32) -> Option<ColorResponse> {
    get_pixel_color(x, y).map(|c| c.into())
}
