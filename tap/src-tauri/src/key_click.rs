//! Key-to-Click tool mode.
//!
//! Behavior:
//! - On KeyDown (A-Z): Immediately click once
//! - If key is held longer than `hold_delay_ms`: Start repeating clicks at `interval_ms`
//! - On KeyUp: Stop repeating (return to armed state)
//! - On Space KeyDown: Stop the entire mode immediately

use crossbeam_channel::{bounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tap_core::{Action, MouseButton};
use tap_platform::{EnigoInjector, InputEventType, InputHookHandle, InputInjector};
use tracing::{debug, info, warn};

#[derive(Debug, Clone, serde::Serialize)]
pub enum KeyClickEvent {
    Started,
    Click { count: u64, x: i32, y: i32 },
    Stopped { total_clicks: u64 },
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyClickStatus {
    pub running: bool,
    pub click_count: u64,
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

#[derive(Debug, Clone)]
pub struct KeyClickConfig {
    pub interval_ms: u64,
    pub hold_delay_ms: u64,
}

impl Default for KeyClickConfig {
    fn default() -> Self {
        Self {
            interval_ms: 50,
            hold_delay_ms: 150,
        }
    }
}

fn is_az_key(key: &str) -> bool {
    key.len() == 1 && key.chars().next().map_or(false, |c| c.is_ascii_alphabetic())
}

pub fn start_key_click_runner(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32) + Send + 'static,
) -> KeyClickHandle {
    let (event_tx, event_rx) = bounded::<KeyClickEvent>(256);

    let stop_requested = Arc::new(AtomicBool::new(false));
    let running = Arc::new(AtomicBool::new(true));
    let click_count = Arc::new(AtomicU64::new(0));

    let stop_clone = stop_requested.clone();
    let running_clone = running.clone();
    let count_clone = click_count.clone();

    let thread = thread::spawn(move || {
        run_loop(config, input_hook, injector, get_mouse_position, stop_clone, event_tx, running_clone, count_clone);
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

fn run_loop(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32),
    stop_requested: Arc<AtomicBool>,
    event_tx: Sender<KeyClickEvent>,
    running: Arc<AtomicBool>,
    click_count: Arc<AtomicU64>,
) {
    info!("Key-click started (interval={}ms, hold_delay={}ms)", config.interval_ms, config.hold_delay_ms);
    let _ = event_tx.send(KeyClickEvent::Started);

    let hold_delay = Duration::from_millis(config.hold_delay_ms);
    let repeat_interval = Duration::from_millis(config.interval_ms);
    let mut active: Option<ActiveKey> = None;

    loop {
        // Check stop flag
        if stop_requested.load(Ordering::SeqCst) {
            info!("Key-click received stop signal");
            break;
        }

        // Process input events
        for raw_event in input_hook.drain() {
            match &raw_event.event {
                InputEventType::KeyDown { key } => {
                    debug!(key, "KeyDown received");
                    
                    // Space stops immediately
                    if key == "Space" {
                        info!("Key-click stopped by Space");
                        cleanup(&running, &click_count, &event_tx, &input_hook);
                        return;
                    }

                    // A-Z triggers click (only if no active key)
                    if is_az_key(key) && active.is_none() {
                        let (x, y) = get_mouse_position();
                        if do_click(&injector, x, y, &click_count, &event_tx) {
                            debug!(key, "Initial click");
                        }
                        active = Some(ActiveKey {
                            key: key.clone(),
                            repeating: false,
                            next_repeat_at: Instant::now() + hold_delay,
                        });
                    }
                }
                InputEventType::KeyUp { key } => {
                    debug!(key, "KeyUp received");
                    
                    // Clear active key if it matches
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

        // Handle repeat clicks
        if let Some(ref mut state) = active {
            let now = Instant::now();
            if now >= state.next_repeat_at {
                if !state.repeating {
                    state.repeating = true;
                    debug!("Entering repeat mode");
                }
                let (x, y) = get_mouse_position();
                do_click(&injector, x, y, &click_count, &event_tx);
                state.next_repeat_at = now + repeat_interval;
            }
        }

        thread::sleep(Duration::from_millis(5));
    }

    cleanup(&running, &click_count, &event_tx, &input_hook);
}

fn do_click(
    injector: &EnigoInjector,
    x: i32,
    y: i32,
    click_count: &AtomicU64,
    event_tx: &Sender<KeyClickEvent>,
) -> bool {
    let action = Action::Click { x, y, button: MouseButton::Left };
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
) {
    running.store(false, Ordering::SeqCst);
    let total = click_count.load(Ordering::SeqCst);
    let _ = event_tx.send(KeyClickEvent::Stopped { total_clicks: total });
    input_hook.stop();
    info!("Key-click exited, total clicks: {}", total);
}

