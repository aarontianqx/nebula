//! System queries: OS permissions, global mouse tracking, windows and pixels.

use std::sync::Mutex;

use tap_platform::{
    check_permissions, get_pixel_color, is_window_focused, list_windows, open_permission_settings,
    request_screen_recording, start_mouse_tracker, window_exists, Color, MouseTrackerConfig,
    PermissionStatus, WindowInfo,
};
use tauri::State;
use tracing::info;

use crate::state::AppState;

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
pub fn start_mouse_tracking(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if app_state.mouse_tracker.is_some() {
        return Ok(()); // Already tracking
    }

    let config = MouseTrackerConfig::default();
    let tracker = start_mouse_tracker(config);
    app_state.mouse_tracker = Some(tracker);

    info!("Global mouse tracking started");
    Ok(())
}

#[tauri::command]
pub fn stop_mouse_tracking(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    if let Some(tracker) = app_state.mouse_tracker.take() {
        tracker.stop();
        info!("Global mouse tracking stopped");
    }

    Ok(())
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

#[tauri::command]
pub fn cmd_check_window_focused(title: Option<String>, process: Option<String>) -> bool {
    is_window_focused(title.as_deref(), process.as_deref())
}

#[tauri::command]
pub fn cmd_check_window_exists(title: Option<String>, process: Option<String>) -> bool {
    window_exists(title.as_deref(), process.as_deref())
}
