//! Key-to-Click tool mode.
//!
//! This module implements an event-driven tool mode where pressing A-Z keys
//! triggers mouse clicks at the current cursor position. Holding a key repeats
//! clicks at a configurable interval. Pressing Space stops the mode.

use crossbeam_channel::{bounded, Receiver, Sender};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tap_core::{Action, MouseButton};
use tap_platform::{EnigoInjector, InputEventType, InputHookHandle, InputInjector};
use tracing::{debug, info, warn};

/// Event emitted by the KeyClickRunner.
#[derive(Debug, Clone, serde::Serialize)]
pub enum KeyClickEvent {
    /// Mode started.
    Started,
    /// A click was performed.
    Click { count: u64, x: i32, y: i32 },
    /// Mode stopped (by Space or external stop).
    Stopped { total_clicks: u64 },
}

/// Status of the key-click mode.
#[derive(Debug, Clone, serde::Serialize)]
pub struct KeyClickStatus {
    pub running: bool,
    pub click_count: u64,
}

/// Handle to control and observe the KeyClickRunner.
pub struct KeyClickHandle {
    /// Signal to stop the runner.
    stop_tx: Sender<()>,
    /// Receive events from the runner.
    event_rx: Receiver<KeyClickEvent>,
    /// Shared running state.
    running: Arc<AtomicBool>,
    /// Shared click count.
    click_count: Arc<AtomicU64>,
    /// Thread handle.
    thread: Option<JoinHandle<()>>,
}

impl KeyClickHandle {
    /// Check if the runner is still active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Get the current click count.
    pub fn click_count(&self) -> u64 {
        self.click_count.load(Ordering::SeqCst)
    }

    /// Drain all pending events.
    pub fn drain(&self) -> Vec<KeyClickEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Signal the runner to stop.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }

    /// Get current status.
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
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Configuration for the KeyClickRunner.
#[derive(Debug, Clone)]
pub struct KeyClickConfig {
    /// Interval between repeated clicks when holding a key (ms).
    pub interval_ms: u64,
}

impl Default for KeyClickConfig {
    fn default() -> Self {
        Self { interval_ms: 50 }
    }
}

/// Check if a key string represents an A-Z key.
fn is_az_key(key: &str) -> bool {
    if key.len() == 1 {
        let c = key.chars().next().unwrap();
        c.is_ascii_alphabetic()
    } else {
        false
    }
}

/// Start the key-click runner.
///
/// Returns a handle to control and observe the runner.
pub fn start_key_click_runner(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32) + Send + 'static,
) -> KeyClickHandle {
    let (stop_tx, stop_rx) = bounded::<()>(1);
    let (event_tx, event_rx) = bounded::<KeyClickEvent>(256);

    let running = Arc::new(AtomicBool::new(true));
    let click_count = Arc::new(AtomicU64::new(0));

    let running_clone = running.clone();
    let click_count_clone = click_count.clone();

    let thread = thread::spawn(move || {
        run_key_click_loop(
            config,
            input_hook,
            injector,
            get_mouse_position,
            stop_rx,
            event_tx,
            running_clone,
            click_count_clone,
        );
    });

    // Send started event
    // Note: The actual Started event is sent from within the loop

    KeyClickHandle {
        stop_tx,
        event_rx,
        running,
        click_count,
        thread: Some(thread),
    }
}

/// Main loop for key-click mode.
fn run_key_click_loop(
    config: KeyClickConfig,
    input_hook: InputHookHandle,
    injector: Arc<EnigoInjector>,
    get_mouse_position: impl Fn() -> (i32, i32),
    stop_rx: Receiver<()>,
    event_tx: Sender<KeyClickEvent>,
    running: Arc<AtomicBool>,
    click_count: Arc<AtomicU64>,
) {
    info!("Key-click runner started");

    // Send started event
    let _ = event_tx.send(KeyClickEvent::Started);

    // Track which keys are currently held
    let mut keys_held: HashSet<String> = HashSet::new();

    // Track last click time for rate limiting
    let mut last_click_time = std::time::Instant::now();
    let click_interval = Duration::from_millis(config.interval_ms);

    loop {
        // Check for stop signal
        if stop_rx.try_recv().is_ok() {
            info!("Key-click runner received stop signal");
            break;
        }

        // Drain input events
        let events = input_hook.drain();

        for raw_event in events {
            match &raw_event.event {
                InputEventType::KeyDown { key } => {
                    // Check for Space to stop
                    if key == "Space" {
                        info!("Key-click runner stopped by Space key");
                        running.store(false, Ordering::SeqCst);
                        let total = click_count.load(Ordering::SeqCst);
                        let _ = event_tx.send(KeyClickEvent::Stopped { total_clicks: total });
                        return;
                    }

                    // Check if it's an A-Z key
                    if is_az_key(key) && !keys_held.contains(key) {
                        debug!(key, "Key pressed, adding to held set");
                        keys_held.insert(key.clone());
                    }
                }
                InputEventType::KeyUp { key } => {
                    if is_az_key(key) {
                        debug!(key, "Key released, removing from held set");
                        keys_held.remove(key);
                    }
                }
                _ => {
                    // Ignore mouse events
                }
            }
        }

        // If any A-Z keys are held, perform clicks at interval
        if !keys_held.is_empty() {
            let now = std::time::Instant::now();
            if now.duration_since(last_click_time) >= click_interval {
                // Get current mouse position
                let (x, y) = get_mouse_position();

                // Inject click
                let action = Action::Click {
                    x,
                    y,
                    button: MouseButton::Left,
                };

                match injector.inject(&action) {
                    Ok(()) => {
                        let count = click_count.fetch_add(1, Ordering::SeqCst) + 1;
                        debug!(x, y, count, "Click injected");
                        let _ = event_tx.send(KeyClickEvent::Click { count, x, y });
                    }
                    Err(e) => {
                        warn!(?e, "Failed to inject click");
                    }
                }

                last_click_time = now;
            }
        }

        // Small sleep to avoid busy loop
        thread::sleep(Duration::from_millis(5));
    }

    // Cleanup
    running.store(false, Ordering::SeqCst);
    let total = click_count.load(Ordering::SeqCst);
    let _ = event_tx.send(KeyClickEvent::Stopped { total_clicks: total });

    // Stop the input hook
    input_hook.stop();

    info!("Key-click runner exited, total clicks: {}", total);
}

