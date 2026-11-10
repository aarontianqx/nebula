//! Background threads that relay events from the lower layers to the frontend as they arrive.

use std::thread;

use crossbeam_channel::Receiver;
use tap_application::EngineEvent;
use tap_platform::{MouseTrackerEvent, MouseTrackerHandle};
use tauri::{AppHandle, Emitter};
use tracing::{debug, warn};

use crate::dto::MousePositionUpdate;

pub fn emit_to_frontend<S: serde::Serialize + Clone>(app: &AppHandle, event: &str, payload: S) {
    if let Err(e) = app.emit(event, payload) {
        warn!(event, error = %e, "failed to emit event to the frontend");
    }
}

pub fn forward_engine_events(app: AppHandle, events: Receiver<EngineEvent>) {
    thread::spawn(move || {
        for event in events {
            debug!(?event, "engine event");
            emit_to_frontend(&app, "engine-event", &event);
        }
    });
}

/// Takes ownership of the tracker, which lives as long as the app does.
pub fn forward_mouse_position(app: AppHandle, tracker: MouseTrackerHandle) {
    thread::spawn(move || {
        for MouseTrackerEvent::PositionUpdate { x, y } in tracker.events() {
            emit_to_frontend(&app, "mouse-position", MousePositionUpdate { x, y });
        }
    });
}
