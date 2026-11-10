//! Recording: the platform's input hook feeding a [`RecordingSession`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crossbeam_channel::RecvTimeoutError;
use tap_application::{
    MouseButtonRaw, RawEventType, RecorderState, RecordingSession, RecordingStatus,
};
use tap_core::{DslTimedAction, MacroDocument};
use tap_platform::{start_input_hook, InputEventType, InputHookHandle, MouseButtonType};
use tauri::AppHandle;

use crate::forward::emit_to_frontend;

/// How often the forwarding thread looks up from the hook to see whether recording has ended.
const STOP_CHECK_INTERVAL: Duration = Duration::from_millis(100);

pub struct RecordingState {
    session: Arc<Mutex<RecordingSession>>,
    /// Ends the thread forwarding the current recording's events.
    forwarder_stop: Mutex<Option<Arc<AtomicBool>>>,
}

impl Default for RecordingState {
    fn default() -> Self {
        let locate = Box::new(|title: Option<&str>, process: Option<&str>| {
            tap_platform::window_origin(title, process)
        });
        Self {
            session: Arc::new(Mutex::new(RecordingSession::new(locate))),
            forwarder_stop: Mutex::new(None),
        }
    }
}

impl RecordingState {
    pub fn state(&self) -> RecorderState {
        self.session.lock().unwrap().state()
    }

    pub fn status(&self) -> RecordingStatus {
        self.session.lock().unwrap().status()
    }

    pub fn start(&self, app: &AppHandle, document: &MacroDocument) -> Result<(), String> {
        let mut session = self.session.lock().unwrap();
        session.start(document)?;

        let stop = Arc::new(AtomicBool::new(false));
        *self.forwarder_stop.lock().unwrap() = Some(stop.clone());
        let forwarder = InputForwarder {
            app: app.clone(),
            session: self.session.clone(),
            hook: start_input_hook(),
            stop,
        };
        thread::spawn(move || forwarder.run());
        Ok(())
    }

    pub fn pause(&self) -> Result<(), String> {
        self.session.lock().unwrap().pause()
    }

    pub fn resume(&self) -> Result<(), String> {
        self.session.lock().unwrap().resume()
    }

    pub fn stop(&self) -> Option<Vec<DslTimedAction>> {
        if let Some(stop) = self.forwarder_stop.lock().unwrap().take() {
            stop.store(true, Ordering::SeqCst);
        }
        self.session.lock().unwrap().stop()
    }
}

/// Relays the hook's events into the session until recording stops.
struct InputForwarder {
    app: AppHandle,
    session: Arc<Mutex<RecordingSession>>,
    hook: InputHookHandle,
    stop: Arc<AtomicBool>,
}

impl InputForwarder {
    /// Dropping the hook on the way out ends it.
    fn run(self) {
        while !self.stop.load(Ordering::SeqCst) {
            match self.hook.recv_timeout(STOP_CHECK_INTERVAL) {
                Ok(raw) => {
                    let status = self
                        .session
                        .lock()
                        .unwrap()
                        .push(raw.timestamp_ms, convert(raw.event));
                    if let Some(status) = status {
                        emit_to_frontend(&self.app, "recording-status", status);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }
}

fn convert(event: InputEventType) -> RawEventType {
    match event {
        InputEventType::MouseMove { x, y } => RawEventType::MouseMove { x, y },
        InputEventType::MouseDown { x, y, button } => RawEventType::MouseDown {
            x,
            y,
            button: convert_button(button),
        },
        InputEventType::MouseUp { x, y, button } => RawEventType::MouseUp {
            x,
            y,
            button: convert_button(button),
        },
        InputEventType::Scroll { delta_x, delta_y } => RawEventType::Scroll { delta_x, delta_y },
        InputEventType::KeyDown { key } => RawEventType::KeyDown { key },
        InputEventType::KeyUp { key } => RawEventType::KeyUp { key },
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
