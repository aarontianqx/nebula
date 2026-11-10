//! Key->Click tool mode commands, plus the one-shot key capture helper.

use std::sync::{Arc, Mutex};

use tap_application::{EngineState, RecorderState};
use tap_core::normalize_key;
use tap_platform::{is_window_focused, start_input_hook, EnigoInjector, InputEventType};
use tauri::State;
use tracing::{info, warn};

use crate::key_click::{start_key_click_runner, FocusGate, KeyClickRequest, KeyClickStatus};
use crate::state::AppState;

/// Capture the next physical key press and return its normalized name.
///
/// Blocks (up to a timeout) on a temporary input hook, so it requires the app
/// to be idle (no run / recording / Key->Click) to avoid competing hooks.
#[tauri::command]
pub fn cmd_capture_key(state: State<'_, Mutex<AppState>>) -> Result<String, String> {
    {
        let app_state = state.lock().unwrap();
        if app_state.coordinator.engine_state() != EngineState::Idle {
            return Err("Cannot capture: engine is not idle".into());
        }
        if app_state.recorder.state() != RecorderState::Idle {
            return Err("Cannot capture: recording in progress".into());
        }
        if app_state.key_click_handle.is_some() {
            return Err("Cannot capture: Key->Click is running".into());
        }
    } // release the lock before the blocking wait

    let hook = start_input_hook();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        for ev in hook.drain() {
            if let InputEventType::KeyDown { key } = ev.event {
                let normalized = normalize_key(&key);
                hook.stop();
                info!(key = %normalized, "Captured key");
                return Ok(normalized);
            }
        }
        if std::time::Instant::now() >= deadline {
            hook.stop();
            return Err("Key capture timed out (no key pressed)".into());
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[tauri::command]
pub fn start_key_click(
    state: State<'_, Mutex<AppState>>,
    request: KeyClickRequest,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    // Check mutual exclusion: must be idle
    if app_state.coordinator.engine_state() != EngineState::Idle {
        return Err("Cannot start key-click: engine is not idle".into());
    }

    if app_state.recorder.state() != RecorderState::Idle {
        return Err("Cannot start key-click: recording in progress".into());
    }

    if app_state.key_click_handle.is_some() {
        return Err("Key-click mode is already running".into());
    }

    // Optionally lock clicks to the window focused right now, so alt-tabbing
    // away never leaks clicks into another app. Match on process name (stable
    // across title changes), falling back to title when the process is unknown.
    let focus_gate: Option<FocusGate> = if request.only_target_focused {
        match tap_platform::get_foreground_window() {
            Some(w) => {
                let process = w.process_name.clone();
                let title = w.title.clone();
                info!(target_process = %process, target_title = %title, "Key-click gated to active window");
                Some(Box::new(move || {
                    if !process.is_empty() {
                        is_window_focused(None, Some(&process))
                    } else {
                        is_window_focused(Some(&title), None)
                    }
                }) as FocusGate)
            }
            None => {
                warn!("only_target_focused set but no foreground window; gate disabled");
                None
            }
        }
    } else {
        None
    };

    // Reuse the shared injector, creating it on first use. Fail the command (rather than
    // panic while holding the state lock) if the platform refuses to give us one.
    let injector = match &app_state.key_click_injector {
        Some(injector) => injector.clone(),
        None => {
            let injector = Arc::new(
                EnigoInjector::new().map_err(|e| format!("Failed to create injector: {e}"))?,
            );
            app_state.key_click_injector = Some(injector.clone());
            injector
        }
    };
    let dry_run = app_state.dry_run.clone();

    // Start the input hook for capturing key events
    let input_hook = start_input_hook();

    // Resolve the live cursor position for cursor-mode clicks via the platform
    // API directly (the runner thread can't share the mouse tracker).
    let get_position = move || {
        // Use platform-specific mouse position query
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
            let mut point = POINT::default();
            unsafe {
                let _ = GetCursorPos(&mut point);
            }
            (point.x, point.y)
        }
        #[cfg(target_os = "macos")]
        {
            // On macOS, use Core Graphics
            use core_graphics::event::CGEvent;
            use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
            if let Ok(source) = CGEventSource::new(CGEventSourceStateID::CombinedSessionState) {
                if let Ok(event) = CGEvent::new(source) {
                    let loc = event.location();
                    return (loc.x as i32, loc.y as i32);
                }
            }
            (0, 0)
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            (0, 0)
        }
    };

    let config = request.to_config();
    let handle = start_key_click_runner(
        config,
        input_hook,
        injector,
        get_position,
        focus_gate,
        dry_run,
    );
    app_state.key_click_handle = Some(handle);

    info!(
        min_interval_ms = config.min_interval_ms,
        hold_delay_ms = config.hold_delay_ms,
        only_target_focused = request.only_target_focused,
        "Key-click mode started"
    );

    Ok(())
}

#[tauri::command]
pub fn stop_key_click(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    // Just signal stop, don't take the handle.
    // The poll_events loop will clean up when it sees !is_running().
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested");
    }

    Ok(())
}

#[tauri::command]
pub fn get_key_click_status(state: State<'_, Mutex<AppState>>) -> KeyClickStatus {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.key_click_handle {
        handle.status()
    } else {
        KeyClickStatus {
            running: false,
            click_count: 0,
        }
    }
}
