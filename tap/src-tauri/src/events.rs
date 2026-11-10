//! The event pump: a background thread that drains engine, recorder, mouse-tracker and key-click
//! events and forwards them to the frontend.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use tap_application::{relativize_raw, window_binding, EngineEvent, MouseButtonRaw, RawEventType};
use tap_platform::{InputEventType, MouseButtonType, MouseTrackerEvent};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{debug, warn};

use crate::key_click::KeyClickEvent;
use crate::state::{AppState, MousePositionUpdate, RecordingStatus};

pub fn poll_events(app: AppHandle) {
    let mut origin_tracker = OriginTracker::default();
    // The cursor on the screen, whatever space the recording uses (see `convert_input_event`).
    let mut last_screen_pos = (0, 0);
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
        let (input_events, recording_window) = {
            let app_state = state.lock().unwrap();
            let events = app_state
                .input_hook
                .as_ref()
                .map(|h| h.drain())
                .unwrap_or_default();
            // The window recorded positions are measured from, if the document uses one.
            let window = window_binding(app_state.coordinator.document())
                .map(|t| (t.title.clone(), t.process.clone()));
            if app_state.input_hook.is_none() {
                origin_tracker.reset();
            }
            (events, window)
        };

        if !input_events.is_empty() {
            let mut app_state = state.lock().unwrap();

            for raw_event in input_events {
                // Convert platform event to core event
                let mut core_event = convert_input_event(&raw_event.event, last_screen_pos);
                match &core_event {
                    RawEventType::MouseMove { x, y }
                    | RawEventType::MouseDown { x, y, .. }
                    | RawEventType::MouseUp { x, y, .. } => last_screen_pos = (*x, *y),
                    _ => {}
                }

                // Window-relative document: measure positions from the window. The origin is
                // looked up fresh for button events (the ones that matter) and cached briefly for
                // the flood of move events. Without any known origin the event cannot be placed.
                if let Some((title, process)) = &recording_window {
                    let precise = !matches!(core_event, RawEventType::MouseMove { .. });
                    let origin = origin_tracker.origin(Instant::now(), precise, || {
                        tap_platform::window_origin(title.as_deref(), process.as_deref())
                    });
                    match origin {
                        Some(origin) => core_event = relativize_raw(core_event, origin),
                        None if has_position(&core_event) => {
                            debug!("target window not found, dropping a positioned event");
                            continue;
                        }
                        None => {}
                    }
                }

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

/// How long the window origin found for a mouse move is reused. Moves arrive in floods and the
/// window rarely moves between two of them; button events always look the origin up again.
const MOVE_ORIGIN_CACHE: Duration = Duration::from_millis(100);

/// The recorded window's origin, as of the last successful lookup.
#[derive(Default)]
struct OriginTracker {
    last: Option<((i32, i32), Instant)>,
}

impl OriginTracker {
    /// The window origin at `now`. `precise` forces a fresh `lookup`; otherwise a result younger
    /// than `MOVE_ORIGIN_CACHE` is reused. When the window cannot be found the last known origin
    /// stands in, so a press and its release (or a drag that ends outside) stay consistent; with
    /// none known the answer is `None`.
    fn origin(
        &mut self,
        now: Instant,
        precise: bool,
        lookup: impl FnOnce() -> Option<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        if !precise {
            if let Some((origin, at)) = self.last {
                if now.saturating_duration_since(at) < MOVE_ORIGIN_CACHE {
                    return Some(origin);
                }
            }
        }
        match lookup() {
            Some(origin) => {
                self.last = Some((origin, now));
                Some(origin)
            }
            None => self.last.map(|(origin, _)| origin),
        }
    }

    /// Forget the cached origin (a new recording starts from scratch).
    fn reset(&mut self) {
        self.last = None;
    }
}

fn has_position(event: &RawEventType) -> bool {
    matches!(
        event,
        RawEventType::MouseMove { .. }
            | RawEventType::MouseDown { .. }
            | RawEventType::MouseUp { .. }
    )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_events_always_look_the_origin_up_again() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        assert_eq!(tracker.origin(t0, true, || Some((10, 20))), Some((10, 20)));
        // A second precise lookup right away still asks the platform (the window may have moved).
        assert_eq!(tracker.origin(t0, true, || Some((30, 40))), Some((30, 40)));
    }

    #[test]
    fn move_events_reuse_a_fresh_origin_and_refresh_a_stale_one() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        tracker.origin(t0, true, || Some((10, 20)));

        let soon = t0 + Duration::from_millis(50);
        assert_eq!(
            tracker.origin(soon, false, || panic!("must be served from the cache")),
            Some((10, 20))
        );

        let later = t0 + Duration::from_millis(150);
        assert_eq!(
            tracker.origin(later, false, || Some((99, 99))),
            Some((99, 99))
        );
    }

    #[test]
    fn a_lost_window_falls_back_to_the_last_known_origin_and_then_to_none() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        assert_eq!(tracker.origin(t0, true, || None), None);

        tracker.origin(t0, true, || Some((10, 20)));
        assert_eq!(tracker.origin(t0, true, || None), Some((10, 20)));

        tracker.reset();
        assert_eq!(tracker.origin(t0, true, || None), None);
    }
}
