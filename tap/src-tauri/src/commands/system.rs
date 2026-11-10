//! System queries: OS permissions, windows and pixels.

use tap_platform::{
    check_permissions, get_pixel_color, list_windows, open_permission_settings,
    request_screen_recording, PermissionStatus,
};

use crate::dto::{ColorResponse, WindowInfoResponse};

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
