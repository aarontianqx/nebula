//! Key-to-Click tool mode.
//!
//! An event-driven "hold a key to keep clicking" tool, separate from the
//! timeline engine. Behavior:
//! - On KeyDown (A-Z): click once immediately, then arm a repeat timer.
//! - If the key is held past `hold_delay_ms`: repeat clicks every `min_interval_ms`.
//! - On KeyUp: stop repeating (return to the armed state, ready for the next key).
//! - On Space KeyDown: stop the whole mode immediately.
//!
//! Clicks use the configured mouse button at either the live cursor position or
//! a fixed point, and can be gated to a single window so alt-tabbing away never
//! leaks clicks into the wrong app.

use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tap_core::{Action, MouseButton};
use tap_platform::{EnigoInjector, InputEventType, InputHookHandle, InputInjector};
use tracing::{debug, info, warn};

/// Reports whether clicks are currently allowed (e.g. the target window is focused).
pub type FocusGate = Box<dyn Fn() -> bool + Send>;

#[derive(Debug, Clone, serde::Serialize)]
pub enum KeyClickEvent {
    Started,
    Click { count: u64, x: i32, y: i32 },
    Stopped { total_clicks: u64, reason: String },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyClickStatus {
    pub running: bool,
    pub click_count: u64,
}

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

/// Where each click lands.
#[derive(Debug, Clone, Copy)]
pub enum ClickLocation {
    /// The live cursor position at click time.
    Cursor,
    /// A fixed point in injection-space coordinates.
    Fixed { x: i32, y: i32 },
}

/// Raw Key->Click request from the frontend; [`to_config`](Self::to_config) clamps and resolves it.
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
    pub fn to_config(&self) -> KeyClickConfig {
        KeyClickConfig {
            min_interval_ms: self.min_interval_ms.clamp(10, 1000),
            hold_delay_ms: self.hold_delay_ms.min(5000),
            button: self.button.into(),
            location: if self.location_mode == "fixed" {
                ClickLocation::Fixed {
                    x: self.fixed_x,
                    y: self.fixed_y,
                }
            } else {
                ClickLocation::Cursor
            },
        }
    }
}

pub struct KeyClickHandle {
    stop_requested: Arc<AtomicBool>,
    event_rx: Receiver<KeyClickEvent>,
    running: Arc<AtomicBool>,
    click_count: Arc<AtomicU64>,
    thread: Option<JoinHandle<()>>,
}

impl KeyClickHandle {
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn click_count(&self) -> u64 {
        self.click_count.load(Ordering::SeqCst)
    }

    pub fn drain(&self) -> Vec<KeyClickEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::SeqCst);
    }

    pub fn status(&self) -> KeyClickStatus {
        KeyClickStatus {
            running: self.is_running(),
            click_count: self.click_count(),
        }
    }
}

impl Drop for KeyClickHandle {
    fn drop(&mut self) {
        self.stop();
        // Don't join the thread - let it exit on its own.
        // Joining here can cause issues if the thread is still processing.
        let _ = self.thread.take();
    }
}

#[derive(Debug, Clone, Copy)]
pub struct KeyClickConfig {
    pub min_interval_ms: u64,
    pub hold_delay_ms: u64,
    pub button: MouseButton,
    pub location: ClickLocation,
}

impl Default for KeyClickConfig {
    fn default() -> Self {
        Self {
            min_interval_ms: 40,
            hold_delay_ms: 150,
            button: MouseButton::Left,
            location: ClickLocation::Cursor,
        }
    }
}

fn is_az_key(key: &str) -> bool {
    key.len() == 1 && key.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
}

pub fn start_key_click_runner(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32) + Send + 'static,
    focus_gate: Option<FocusGate>,
) -> KeyClickHandle {
    let (event_tx, event_rx) = bounded::<KeyClickEvent>(256);

    let stop_requested = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let click_count = Arc::new(AtomicU64::new(0));

    let stop_clone = stop_requested.clone();
    let running_clone = running.clone();
    let count_clone = click_count.clone();

    let thread = thread::spawn(move || {
        run_loop(
            config,
            input_hook,
            injector,
            get_mouse_position,
            focus_gate,
            stop_clone,
            event_tx,
            running_clone,
            count_clone,
        );
    });

    KeyClickHandle {
        stop_requested,
        event_rx,
        running,
        click_count,
        thread: Some(thread),
    }
}

struct ActiveKey {
    key: String,
    repeating: bool,
    next_repeat_at: Instant,
}

#[allow(clippy::too_many_arguments)] // internal worker: shares the runner's handles directly.
fn run_loop(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32),
    focus_gate: Option<FocusGate>,
    stop_requested: Arc<AtomicBool>,
    event_tx: Sender<KeyClickEvent>,
    running: Arc<AtomicBool>,
    click_count: Arc<AtomicU64>,
) {
    info!(
        min_interval_ms = config.min_interval_ms,
        hold_delay_ms = config.hold_delay_ms,
        button = ?config.button,
        "Key-click started"
    );
    let _ = event_tx.send(KeyClickEvent::Started);

    let hold_delay = Duration::from_millis(config.hold_delay_ms);
    let repeat_interval = Duration::from_millis(config.min_interval_ms);
    let mut active: Option<ActiveKey> = None;

    // Fire one click, respecting the focus gate and resolving the target point.
    let click_once = |click_count: &AtomicU64, event_tx: &Sender<KeyClickEvent>| {
        if let Some(ref gate) = focus_gate {
            if !gate() {
                return; // target window not focused; skip silently
            }
        }
        let (x, y) = match config.location {
            ClickLocation::Cursor => get_mouse_position(),
            ClickLocation::Fixed { x, y } => (x, y),
        };
        do_click(&injector, config.button, x, y, click_count, event_tx);
    };

    loop {
        if stop_requested.load(Ordering::SeqCst) {
            info!("Key-click received stop signal");
            break;
        }

        for raw_event in input_hook.drain() {
            match &raw_event.event {
                InputEventType::KeyDown { key } => {
                    debug!(key, "KeyDown received");

                    // Space stops immediately.
                    if key == "Space" {
                        info!("Key-click stopped by Space");
                        cleanup(&running, &click_count, &event_tx, &input_hook, "space");
                        return;
                    }

                    // A-Z triggers a click (only if no key is already active).
                    if is_az_key(key) && active.is_none() {
                        click_once(&click_count, &event_tx);
                        active = Some(ActiveKey {
                            key: key.clone(),
                            repeating: false,
                            next_repeat_at: Instant::now() + hold_delay,
                        });
                    }
                }
                InputEventType::KeyUp { key } => {
                    debug!(key, "KeyUp received");
                    if let Some(ref state) = active {
                        if state.key.eq_ignore_ascii_case(key) {
                            debug!(key, "Key released");
                            active = None;
                        }
                    }
                }
                _ => {}
            }
        }

        if let Some(ref mut state) = active {
            let now = Instant::now();
            if now >= state.next_repeat_at {
                if !state.repeating {
                    state.repeating = true;
                    debug!("Entering repeat mode");
                }
                click_once(&click_count, &event_tx);
                state.next_repeat_at = now + repeat_interval;
            }
        }

        thread::sleep(Duration::from_millis(5));
    }

    cleanup(&running, &click_count, &event_tx, &input_hook, "external");
}

fn do_click(
    injector: &EnigoInjector,
    button: MouseButton,
    x: i32,
    y: i32,
    click_count: &AtomicU64,
    event_tx: &Sender<KeyClickEvent>,
) -> bool {
    let action = Action::Click { x, y, button };
    match injector.inject(&action) {
        Ok(()) => {
            let count = click_count.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = event_tx.send(KeyClickEvent::Click { count, x, y });
            true
        }
        Err(e) => {
            warn!(?e, "Click failed");
            false
        }
    }
}

fn cleanup(
    running: &AtomicBool,
    click_count: &AtomicU64,
    event_tx: &Sender<KeyClickEvent>,
    input_hook: &InputHookHandle,
    reason: &str,
) {
    running.store(false, Ordering::SeqCst);
    let total = click_count.load(Ordering::SeqCst);
    let _ = event_tx.send(KeyClickEvent::Stopped {
        total_clicks: total,
        reason: reason.to_string(),
    });
    input_hook.stop();
    info!(reason, total_clicks = total, "Key-click exited");
}
