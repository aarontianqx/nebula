//! Position picker: a fullscreen transparent overlay window that reports the clicked screen position,
//! and optionally the color under it.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tap_application::window_binding;
use tap_core::Point;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder};
use tracing::{info, warn};

use crate::dto::{ColorResponse, PositionPickedEvent};
use crate::state::CoordinatorState;

/// What a pick should report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PickMode {
    /// Just the position.
    #[default]
    Position,
    /// The position and the color under it.
    PositionColor,
}

/// The mode of the picker that is (or is about to be) open.
///
/// Every `open_picker_window` call sets it, also when the overlay already exists, so a click is
/// always answered for the most recent request.
#[derive(Default)]
pub struct PickerState {
    mode: Mutex<PickMode>,
}

/// How long to wait for the overlay to disappear from the screen before sampling it anyway.
const OVERLAY_CLOSE_TIMEOUT: Duration = Duration::from_millis(500);
/// After the overlay is gone the compositor still needs a few frames to repaint what was under
/// it (the overlay is a translucent shade, so sampling earlier reads a tinted color).
const REPAINT_DELAY: Duration = Duration::from_millis(80);

#[tauri::command]
pub async fn open_picker_window(
    app: AppHandle,
    state: State<'_, PickerState>,
    mode: Option<PickMode>,
) -> Result<(), String> {
    *state.mode.lock().map_err(|e| e.to_string())? = mode.unwrap_or_default();

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

/// Read the color at an injection-space position once the overlay is off the screen.
///
/// The overlay is a translucent shade, so the color must not be read while it is visible: wait
/// for the window to be gone, then one repaint, then sample. Blocking, so run it off the async
/// runtime's threads.
fn sample_color_without_overlay(app: &AppHandle, x: i32, y: i32) -> Option<ColorResponse> {
    let deadline = Instant::now() + OVERLAY_CLOSE_TIMEOUT;
    while app.get_webview_window("picker").is_some() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(10));
    }
    std::thread::sleep(REPAINT_DELAY);
    tap_platform::get_pixel_color(x, y).map(ColorResponse::from)
}

#[tauri::command]
pub async fn picker_position_selected(
    app: AppHandle,
    state: State<'_, PickerState>,
    x: i32,
    y: i32,
) -> Result<(), String> {
    let mode = *state.mode.lock().map_err(|e| e.to_string())?;

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

    let color = match mode {
        PickMode::Position => None,
        PickMode::PositionColor => {
            let sampler_app = app.clone();
            let sampled = tauri::async_runtime::spawn_blocking(move || {
                sample_color_without_overlay(&sampler_app, inject_x, inject_y)
            })
            .await
            .map_err(|e| format!("Color sampling failed: {}", e))?;
            if sampled.is_none() {
                warn!("Could not read the color at ({}, {})", inject_x, inject_y);
            }
            sampled
        }
    };

    // A document measured from its target window needs that window's origin to turn the picked
    // screen position into a window-relative one. The editor does the subtraction (it also knows
    // which field asked); Simple mode keeps the screen position.
    let binding = {
        let coordinator = app.state::<CoordinatorState>();
        let coordinator = coordinator.lock().map_err(|e| e.to_string())?;
        window_binding(coordinator.document()).map(|t| (t.title.clone(), t.process.clone()))
    };
    let window_origin = binding.as_ref().and_then(|(title, process)| {
        tap_platform::window_origin(title.as_deref(), process.as_deref())
            .map(|(x, y)| Point { x, y })
    });
    let window_missing = binding.is_some() && window_origin.is_none();
    if window_missing {
        warn!("Target window not found while picking a window-relative position");
    }

    // Emit the injection-space coordinates (and color, if asked for) to the main window
    app.emit(
        "position-picked",
        PositionPickedEvent {
            x: inject_x,
            y: inject_y,
            color,
            window_origin,
            window_missing,
        },
    )
    .map_err(|e| format!("Failed to emit position-picked: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_mode_names_are_the_ones_the_frontend_sends() {
        assert_eq!(
            serde_json::from_str::<PickMode>("\"position\"").unwrap(),
            PickMode::Position
        );
        assert_eq!(
            serde_json::from_str::<PickMode>("\"position_color\"").unwrap(),
            PickMode::PositionColor
        );
        assert_eq!(PickMode::default(), PickMode::Position);
    }

    #[test]
    fn the_color_is_left_out_of_the_event_unless_it_was_asked_for() {
        let plain = serde_json::to_value(PositionPickedEvent {
            x: 1,
            y: 2,
            color: None,
            window_origin: None,
            window_missing: false,
        })
        .unwrap();
        assert_eq!(plain, serde_json::json!({ "x": 1, "y": 2 }));

        let with_color = serde_json::to_value(PositionPickedEvent {
            x: 1,
            y: 2,
            color: Some(ColorResponse {
                r: 255,
                g: 136,
                b: 0,
                hex: "#FF8800".into(),
            }),
            window_origin: None,
            window_missing: false,
        })
        .unwrap();
        assert_eq!(with_color["color"]["hex"], "#FF8800");
    }

    #[test]
    fn the_window_origin_is_reported_only_for_window_relative_documents() {
        let relative = serde_json::to_value(PositionPickedEvent {
            x: 110,
            y: 70,
            color: None,
            window_origin: Some(Point { x: 100, y: 50 }),
            window_missing: false,
        })
        .unwrap();
        assert_eq!(
            relative,
            serde_json::json!({ "x": 110, "y": 70, "window_origin": { "x": 100, "y": 50 } })
        );

        let missing = serde_json::to_value(PositionPickedEvent {
            x: 110,
            y: 70,
            color: None,
            window_origin: None,
            window_missing: true,
        })
        .unwrap();
        assert_eq!(
            missing,
            serde_json::json!({ "x": 110, "y": 70, "window_missing": true })
        );
    }
}
