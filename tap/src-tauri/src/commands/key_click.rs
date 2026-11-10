//! Key->Click tool mode commands, plus the one-shot key capture helper.

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use tap_application::{ClickLocation, KeyClickConfig};
use tap_core::{normalize_key, MouseButton};
use tap_platform::{start_input_hook, EnigoInjector, InputEventType};
use tauri::{AppHandle, State};
use tracing::info;

use crate::key_click::{gate_to_foreground_window, KeyClickRun, KeyClickState};
use crate::recording::RecordingState;
use crate::state::{ensure_idle, CoordinatorState, DryRun};

const KEY_CAPTURE_TIMEOUT: Duration = Duration::from_secs(10);

/// Mouse button choice, deserialized from the frontend (`"left"`/`"right"`/`"middle"`).
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KeyClickButton {
    Left,
    Right,
    Middle,
}

impl From<KeyClickButton> for MouseButton {
    fn from(b: KeyClickButton) -> Self {
        match b {
            KeyClickButton::Left => MouseButton::Left,
            KeyClickButton::Right => MouseButton::Right,
            KeyClickButton::Middle => MouseButton::Middle,
        }
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyClickRequest {
    pub min_interval_ms: u64,
    pub hold_delay_ms: u64,
    pub button: KeyClickButton,
    /// `"cursor"` (default) or `"fixed"`.
    pub location_mode: String,
    pub fixed_x: i32,
    pub fixed_y: i32,
    /// When true, lock clicks to the window focused at start time.
    pub only_target_focused: bool,
}

impl KeyClickRequest {
    fn config(&self) -> KeyClickConfig {
        let location = if self.location_mode == "fixed" {
            ClickLocation::Fixed {
                x: self.fixed_x,
                y: self.fixed_y,
            }
        } else {
            ClickLocation::Cursor
        };
        KeyClickConfig::new(
            self.min_interval_ms,
            self.hold_delay_ms,
            self.button.into(),
            location,
        )
    }
}

/// Capture the next physical key press and return its normalized name.
///
/// Blocks (up to a timeout) on a temporary input hook, so it needs everything else idle to avoid
/// competing hooks.
///
/// `async` runs this plain function on Tauri's thread pool: a plain command runs on the main
/// thread, which a wait of up to 10 s would freeze (window events, the UI) for its duration.
#[tauri::command(async)]
pub fn cmd_capture_key(
    coordinator: State<'_, CoordinatorState>,
    recording: State<'_, RecordingState>,
    key_click: State<'_, KeyClickState>,
) -> Result<String, String> {
    ensure_idle("capture a key", &coordinator, &recording, &key_click)?;

    let hook = start_input_hook();
    let deadline = Instant::now() + KEY_CAPTURE_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match hook.recv_timeout(remaining) {
            Ok(event) => {
                if let InputEventType::KeyDown { key } = event.event {
                    let normalized = normalize_key(&key);
                    info!(key = %normalized, "Captured key");
                    return Ok(normalized);
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                return Err("Key capture timed out (no key pressed)".into())
            }
            Err(RecvTimeoutError::Disconnected) => return Err("The input hook ended".into()),
        }
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)] // one managed state per concern is what Tauri injects.
pub fn start_key_click(
    app: AppHandle,
    coordinator: State<'_, CoordinatorState>,
    recording: State<'_, RecordingState>,
    key_click: State<'_, KeyClickState>,
    injector: State<'_, Arc<EnigoInjector>>,
    dry_run: State<'_, DryRun>,
    request: KeyClickRequest,
) -> Result<(), String> {
    ensure_idle("start Key->Click", &coordinator, &recording, &key_click)?;

    let config = request.config();
    let focus_gate = if request.only_target_focused {
        gate_to_foreground_window()
    } else {
        None
    };
    key_click.start(KeyClickRun {
        app,
        config,
        hook: start_input_hook(),
        injector: injector.inner().clone(),
        dry_run: dry_run.inner().clone(),
        focus_gate,
    })?;

    info!(
        min_interval_ms = config.min_interval_ms(),
        hold_delay_ms = config.hold_delay_ms(),
        only_target_focused = request.only_target_focused,
        "Key-click mode started"
    );
    Ok(())
}

#[tauri::command]
pub fn stop_key_click(key_click: State<'_, KeyClickState>) {
    key_click.stop();
    info!("Key-click mode stop requested");
}
