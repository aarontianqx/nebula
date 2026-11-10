//! The event pump: a background thread that drains engine, recorder, mouse-tracker and key-click
//! events and forwards them to the frontend.

use std::sync::Mutex;

use tap_application::{EngineEvent, MouseButtonRaw, RawEventType};
use tap_platform::{InputEventType, MouseButtonType, MouseTrackerEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, warn};

use crate::key_click::KeyClickEvent;
use crate::state::{AppState, MousePositionUpdate, RecordingStatus};

pub fn poll_events(app: AppHandle) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(16)); // ~60fps for smooth recording

        let state: State<'_, Mutex<AppState>> = app.state();

        // Collect player events
        let player_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state.coordinator.drain_events()
        };

        // Process player events
        for event in player_events {
            debug!(?event, "received engine event");

            // Update state
            {
                let mut app_state = state.lock().unwrap();
                match &event {
                    EngineEvent::StateChanged { new, .. } => {
                        app_state.coordinator.set_engine_state(*new);
                    }
                    EngineEvent::IterationCompleted { iteration } => {
                        debug!(iteration, "iteration completed");
                    }
                    _ => {}
                }
            }

            // Emit to frontend
            if let Err(e) = app.emit("engine-event", &event) {
                warn!("Failed to emit event to frontend: {}", e);
            }
        }

        // Process recording input events
        let input_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .input_hook
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default()
        };

        if !input_events.is_empty() {
            let mut app_state = state.lock().unwrap();
            let last_pos = app_state.recorder.last_mouse_position();

            for raw_event in input_events {
                // Convert platform event to core event
                let core_event = convert_input_event(&raw_event.event, last_pos);

                // Push to recorder; emit recording status to the frontend.
                if let Some(tap_application::RecorderEvent::EventCaptured {
                    event_count,
                    duration_ms,
                }) = app_state
                    .recorder
                    .push_event(raw_event.timestamp_ms, core_event)
                {
                    let status = RecordingStatus {
                        state: app_state.recorder.state(),
                        event_count,
                        duration_ms,
                    };
                    drop(app_state);
                    if let Err(e) = app.emit("recording-status", &status) {
                        warn!("Failed to emit recording status: {}", e);
                    }
                    app_state = state.lock().unwrap();
                }
            }
        }

        // Process global mouse tracker events
        let mouse_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .mouse_tracker
                .as_ref()
                .map(|t| t.drain())
                .unwrap_or_default()
        };

        for mouse_event in mouse_events {
            let MouseTrackerEvent::PositionUpdate { x, y } = mouse_event;
            let _ = app.emit("mouse-position", MousePositionUpdate { x, y });
        }

        // Process key-click events
        let (key_click_events, should_cleanup) = {
            let app_state = state.lock().unwrap();
            let events = app_state
                .key_click_handle
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default();
            // Check if handle exists but is no longer running
            let cleanup = app_state
                .key_click_handle
                .as_ref()
                .map(|h| !h.is_running())
                .unwrap_or(false);
            (events, cleanup)
        };

        // Emit events to frontend
        for event in &key_click_events {
            match event {
                KeyClickEvent::Started => {
                    debug!("Key-click mode started event");
                }
                KeyClickEvent::Click { count, x, y } => {
                    debug!(count, x, y, "Key-click: click performed");
                }
                KeyClickEvent::Stopped {
                    total_clicks,
                    reason,
                } => {
                    debug!(total_clicks, reason, "Key-click mode stopped event");
                }
            }

            if let Err(e) = app.emit("key-click-event", event) {
                warn!("Failed to emit key-click event: {}", e);
            }
        }

        // Clean up handle if stopped (separate lock acquisition)
        if should_cleanup {
            let mut app_state = state.lock().unwrap();
            if app_state
                .key_click_handle
                .as_ref()
                .map(|h| !h.is_running())
                .unwrap_or(false)
            {
                app_state.key_click_handle = None;
                debug!("Key-click handle cleaned up");
            }
        }
    }
}

/// Convert platform input event to core raw event type.
fn convert_input_event(event: &InputEventType, last_pos: (i32, i32)) -> RawEventType {
    match event {
        InputEventType::MouseMove { x, y } => RawEventType::MouseMove { x: *x, y: *y },
        InputEventType::MouseDown { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 {
                last_pos
            } else {
                (*x, *y)
            };
            RawEventType::MouseDown {
                x: px,
                y: py,
                button: convert_button(*button),
            }
        }
        InputEventType::MouseUp { x, y, button } => {
            let (px, py) = if *x == 0 && *y == 0 {
                last_pos
            } else {
                (*x, *y)
            };
            RawEventType::MouseUp {
                x: px,
                y: py,
                button: convert_button(*button),
            }
        }
        InputEventType::Scroll { delta_x, delta_y } => RawEventType::Scroll {
            delta_x: *delta_x,
            delta_y: *delta_y,
        },
        InputEventType::KeyDown { key } => RawEventType::KeyDown { key: key.clone() },
        InputEventType::KeyUp { key } => RawEventType::KeyUp { key: key.clone() },
    }
}

fn convert_button(button: MouseButtonType) -> MouseButtonRaw {
    match button {
        MouseButtonType::Left => MouseButtonRaw::Left,
        MouseButtonType::Right => MouseButtonRaw::Right,
        MouseButtonType::Middle => MouseButtonRaw::Middle,
        MouseButtonType::Unknown => MouseButtonRaw::Unknown,
    }
}
