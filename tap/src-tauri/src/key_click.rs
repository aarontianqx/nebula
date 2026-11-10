//! Key->Click runner: turns the input hook's key events into clicks. When a click is due is
//! decided by `tap_application::KeyClickMachine`; this is the I/O around it.
//!
//! Clicks can be gated to one window, so alt-tabbing away never leaks clicks into another app.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::RecvTimeoutError;
use tap_application::{ClickLocation, KeyClickConfig, KeyClickMachine, KeyDownOutcome};
use tap_core::Action;
use tap_platform::{
    cursor_position, get_foreground_window, is_window_focused, EnigoInjector, InputEventType,
    InputHookHandle, InputInjector,
};
use tauri::AppHandle;
use tracing::{debug, info, warn};

use crate::forward::emit_to_frontend;
use crate::state::DryRun;

/// Longest the runner waits on the hook before checking whether it was asked to stop.
const STOP_CHECK_INTERVAL: Duration = Duration::from_millis(20);

/// Whether clicks are currently allowed (the target window is focused).
pub type FocusGate = Box<dyn Fn() -> bool + Send>;

#[derive(Debug, Clone, serde::Serialize)]
pub enum KeyClickEvent {
    Started,
    Click { count: u64, x: i32, y: i32 },
    Stopped { total_clicks: u64, reason: String },
}

/// Gate clicks to the window that is in the foreground right now, matched by process name (stable
/// across title changes) or, when the process is unknown, by title.
pub fn gate_to_foreground_window() -> Option<FocusGate> {
    let Some(window) = get_foreground_window() else {
        warn!("no foreground window to gate Key->Click to; clicking is not gated");
        return None;
    };
    info!(process = %window.process_name, title = %window.title, "Key->Click gated to the active window");
    Some(Box::new(move || {
        if window.process_name.is_empty() {
            is_window_focused(Some(&window.title), None)
        } else {
            is_window_focused(None, Some(&window.process_name))
        }
    }))
}

struct Running {
    stop_requested: Arc<AtomicBool>,
    running: Arc<AtomicBool>,
}

impl Running {
    fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    fn stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }
}

#[derive(Default)]
pub struct KeyClickState(Mutex<Option<Running>>);

impl KeyClickState {
    pub fn is_running(&self) -> bool {
        self.0
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(Running::is_running)
    }

    /// Ask the runner to stop; it ends on its own a moment later.
    pub fn stop(&self) {
        if let Some(runner) = self.0.lock().unwrap().as_ref() {
            runner.stop();
        }
    }

    pub fn start(&self, run: KeyClickRun) -> Result<(), String> {
        let mut slot = self.0.lock().unwrap();
        if slot.as_ref().is_some_and(Running::is_running) {
            return Err("Key-click mode is already running".into());
        }
        let runner = Running {
            stop_requested: Arc::new(AtomicBool::new(false)),
            running: Arc::new(AtomicBool::new(true)),
        };
        let (stop, running) = (runner.stop_requested.clone(), runner.running.clone());
        thread::spawn(move || run.run(&stop, &running));
        *slot = Some(runner);
        Ok(())
    }
}

pub struct KeyClickRun {
    pub app: AppHandle,
    pub config: KeyClickConfig,
    pub hook: InputHookHandle,
    pub injector: Arc<EnigoInjector>,
    pub dry_run: DryRun,
    pub focus_gate: Option<FocusGate>,
}

impl KeyClickRun {
    fn run(self, stop_requested: &AtomicBool, running: &AtomicBool) {
        let Self {
            app,
            config,
            hook,
            injector,
            dry_run,
            focus_gate,
        } = self;
        let emit = |event: KeyClickEvent| emit_to_frontend(&app, "key-click-event", event);
        info!(
            min_interval_ms = config.min_interval_ms(),
            hold_delay_ms = config.hold_delay_ms(),
            button = ?config.button,
            "Key-click started"
        );
        emit(KeyClickEvent::Started);

        let mut machine = KeyClickMachine::new(&config);
        let mut total_clicks = 0u64;
        let mut click = || {
            if focus_gate.as_ref().is_some_and(|gate| !gate()) {
                return;
            }
            let point = match config.location {
                ClickLocation::Fixed { x, y } => Some((x, y)),
                ClickLocation::Cursor => cursor_position(),
            };
            let Some((x, y)) = point else {
                warn!("cannot read the cursor position; click skipped");
                return;
            };
            // In dry-run the click is only counted and reported, so the live counter shows what
            // would have happened.
            if !dry_run.get() {
                let action = Action::Click {
                    x,
                    y,
                    button: config.button,
                };
                if let Err(e) = injector.inject(&action) {
                    warn!(error = %e, "click failed");
                    return;
                }
            }
            total_clicks += 1;
            emit(KeyClickEvent::Click {
                count: total_clicks,
                x,
                y,
            });
        };

        let reason = loop {
            if stop_requested.load(Ordering::SeqCst) {
                break "external";
            }
            let wait = machine.next_repeat_at().map_or(STOP_CHECK_INTERVAL, |due| {
                due.saturating_duration_since(Instant::now())
                    .min(STOP_CHECK_INTERVAL)
            });
            match hook.recv_timeout(wait) {
                Ok(raw) => match raw.event {
                    InputEventType::KeyDown { key } => {
                        debug!(key, "key down");
                        match machine.key_down(&key, Instant::now()) {
                            KeyDownOutcome::Click => click(),
                            KeyDownOutcome::Stop => break "space",
                            KeyDownOutcome::Ignored => {}
                        }
                    }
                    InputEventType::KeyUp { key } => machine.key_up(&key),
                    _ => {}
                },
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break "input hook ended",
            }
            if machine.repeat_due(Instant::now()) {
                click();
            }
        };

        hook.stop();
        // Before the event: a frontend that starts again on `Stopped` must find the runner free.
        running.store(false, Ordering::SeqCst);
        info!(reason, total_clicks, "Key-click exited");
        emit(KeyClickEvent::Stopped {
            total_clicks,
            reason: reason.to_string(),
        });
    }
}
