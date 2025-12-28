use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::mpsc;

use crate::infrastructure::config;
use crate::infrastructure::input::{KeyCode, KeyEventType, RawKeyEvent};

/// Recognized gestures from raw keyboard events
#[derive(Debug, Clone)]
pub enum Gesture {
    /// Single tap (key pressed and released quickly)
    Tap { key: KeyCode },
    /// Long press started
    LongPressStart { key: KeyCode },
    /// Long press repeat (fired periodically while held)
    LongPressRepeat { key: KeyCode },
    /// Long press ended
    LongPressEnd { key: KeyCode },
}

/// State of a single key
#[derive(Debug)]
enum KeyState {
    /// Key is pressed, waiting to determine if it's a tap or long press
    Pressed {
        since: Instant,
        /// Set to true when long press is triggered OR when key is released early (cancels timer)
        long_press_triggered: Arc<AtomicBool>,
        /// Set to true when key is released to stop the repeat loop
        cancelled: Arc<AtomicBool>,
    },
}

/// Gesture recognizer that converts raw key events into gestures
pub struct GestureRecognizer {
    key_states: HashMap<KeyCode, KeyState>,
    gesture_tx: mpsc::UnboundedSender<Gesture>,
}

impl GestureRecognizer {
    pub fn new(gesture_tx: mpsc::UnboundedSender<Gesture>) -> Self {
        Self {
            key_states: HashMap::new(),
            gesture_tx,
        }
    }

    /// Process a raw keyboard event
    /// Only A-Z letter keys are processed for passthrough
    pub fn process(&mut self, event: RawKeyEvent) {
        // Ignore non-letter keys
        if !event.key.is_passthrough_enabled() {
            return;
        }

        match event.event_type {
            KeyEventType::Press => self.on_press(event.key, event.timestamp),
            KeyEventType::Release => self.on_release(event.key, event.timestamp),
        }
    }

    fn on_press(&mut self, key: KeyCode, now: Instant) {
        let cfg = config::gesture();

        // Debounce: ignore if already pressed recently
        if let Some(KeyState::Pressed { since, .. }) = self.key_states.get(&key) {
            if now.duration_since(*since) < cfg.keyboard_passthrough.debounce_window() {
                return;
            }
        }

        let long_press_triggered = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::new(AtomicBool::new(false));

        // Update state
        self.key_states.insert(
            key,
            KeyState::Pressed {
                since: now,
                long_press_triggered: long_press_triggered.clone(),
                cancelled: cancelled.clone(),
            },
        );

        // Start long press detection timer
        let threshold = cfg.keyboard_passthrough.long_press_threshold();
        let repeat_interval = cfg.keyboard_passthrough.repeat_interval();
        let tx = self.gesture_tx.clone();
        let triggered = long_press_triggered;

        tokio::spawn(async move {
            tokio::time::sleep(threshold).await;

            // Check if still pressed (not cancelled by early release)
            if !triggered.load(Ordering::SeqCst) {
                triggered.store(true, Ordering::SeqCst);

                // Send long press start
                if tx.send(Gesture::LongPressStart { key }).is_err() {
                    return;
                }

                // Periodically send repeat events until cancelled
                while !cancelled.load(Ordering::SeqCst) {
                    tokio::time::sleep(repeat_interval).await;
                    // Check again after sleep to avoid sending extra event
                    if cancelled.load(Ordering::SeqCst) {
                        break;
                    }
                    if tx.send(Gesture::LongPressRepeat { key }).is_err() {
                        break;
                    }
                }
            }
        });
    }

    fn on_release(&mut self, key: KeyCode, now: Instant) {
        let Some(state) = self.key_states.remove(&key) else {
            return;
        };

        let cfg = config::gesture();

        let KeyState::Pressed {
            since,
            long_press_triggered,
            cancelled,
        } = state;

        // Signal the repeat loop to stop
        cancelled.store(true, Ordering::SeqCst);

        let was_long_press = long_press_triggered.load(Ordering::SeqCst);

        if was_long_press {
            // Long press ended
            let _ = self.gesture_tx.send(Gesture::LongPressEnd { key });
        } else {
            // Check if it was a tap (released before threshold)
            let duration = now.duration_since(since);
            if duration < cfg.keyboard_passthrough.long_press_threshold() {
                let _ = self.gesture_tx.send(Gesture::Tap { key });
            }
            // Mark as triggered to cancel the timer task before it starts long press
            long_press_triggered.store(true, Ordering::SeqCst);
        }
    }
}

