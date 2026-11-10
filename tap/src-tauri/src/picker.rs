//! Position picker: a fullscreen transparent overlay window that reports the clicked screen position.

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder};
use tracing::info;

use crate::state::PositionPickedEvent;

#[tauri::command]
pub async fn open_picker_window(app: AppHandle) -> Result<(), String> {
    // Check if picker window already exists
    if app.get_webview_window("picker").is_some() {
        info!("Picker window already open");
        return Ok(());
    }

    // Create a new fullscreen transparent overlay window
    let picker_window =
        WebviewWindowBuilder::new(&app, "picker", WebviewUrl::App("picker.html".into()))
            .title("Pick Position")
            .fullscreen(true)
            .transparent(true)
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .focused(true)
            .build()
            .map_err(|e| format!("Failed to create picker window: {}", e))?;

    info!("Picker window opened");

    // The picker window will handle its own close when position is selected
    let _ = picker_window;

    Ok(())
}

#[tauri::command]
pub async fn close_picker_window(app: AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("picker") {
        window
            .close()
            .map_err(|e| format!("Failed to close picker window: {}", e))?;
        info!("Picker window closed");
    }
    Ok(())
}

#[tauri::command]
pub async fn picker_position_selected(app: AppHandle, x: i32, y: i32) -> Result<(), String> {
    // Close the picker window
    if let Some(window) = app.get_webview_window("picker") {
        let _ = window.close();
    }

    // Convert the browser's CSS pixels (window.screenX/Y) into the canonical
    // injection coordinate space. On Windows that means scaling logical pixels
    // up to physical pixels; on macOS both spaces are points, so the factor is
    // 1.0. Recording, injection and picking therefore share one coordinate
    // system.
    let scale = tap_platform::browser_to_injection_scale();
    let inject_x = (x as f64 * scale).round() as i32;
    let inject_y = (y as f64 * scale).round() as i32;

    info!(
        "Position picked: css ({}, {}), injection ({}, {}), scale {}",
        x, y, inject_x, inject_y, scale
    );

    // Emit the injection-space coordinates to the main window
    app.emit(
        "position-picked",
        PositionPickedEvent {
            x: inject_x,
            y: inject_y,
        },
    )
    .map_err(|e| format!("Failed to emit position-picked: {}", e))?;

    Ok(())
}
