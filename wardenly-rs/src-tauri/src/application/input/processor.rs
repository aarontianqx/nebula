use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::application::input::gesture::{Gesture, GestureRecognizer};
use crate::infrastructure::input::{create_keyboard_listener, KeyboardListener, RawKeyEvent};

/// Input event processor that manages keyboard listening and gesture recognition
pub struct InputEventProcessor {
    keyboard: Arc<Mutex<Box<dyn KeyboardListener>>>,
    active_session: Option<String>,
    cursor_position: Option<(i32, i32)>,
    cursor_in_bounds: bool,
    enabled: bool,
    click_tx: mpsc::UnboundedSender<(String, f64, f64)>,
}

impl InputEventProcessor {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<(String, f64, f64)>) {
        let (click_tx, click_rx) = mpsc::unbounded_channel();
        let keyboard = create_keyboard_listener();

        (
            Self {
                keyboard: Arc::new(Mutex::new(keyboard)),
                active_session: None,
                cursor_position: None,
                cursor_in_bounds: false,
                enabled: false,
                click_tx,
            },
            click_rx,
        )
    }

    /// Start the input processing loop
    pub async fn start_processing(&self) {
        let keyboard = self.keyboard.clone();
        let click_tx = self.click_tx.clone();

        // Get the receiver from keyboard
        let keyboard_rx = {
            let mut kb = keyboard.lock().await;
            kb.take_receiver()
        };

        let Some(mut keyboard_rx) = keyboard_rx else {
            tracing::warn!("Keyboard receiver already taken");
            return;
        };

        // Create gesture recognizer
        let (gesture_tx, mut gesture_rx) = mpsc::unbounded_channel();
        let mut recognizer = GestureRecognizer::new(gesture_tx);

        // Spawn processing task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(raw_event) = keyboard_rx.recv() => {
                        recognizer.process(raw_event);
                    }
                    Some(gesture) = gesture_rx.recv() => {
                        // Gestures will be processed by the main handler
                        tracing::trace!("Gesture: {:?}", gesture);
                    }
                }
            }
        });
    }

    /// Update cursor position
    pub fn update_cursor(&mut self, x: i32, y: i32, in_bounds: bool) {
        self.cursor_position = Some((x, y));
        self.cursor_in_bounds = in_bounds;
    }

    /// Set active session
    pub fn set_active_session(&mut self, session_id: Option<String>) {
        self.active_session = session_id;
    }

    /// Enable or disable keyboard passthrough
    pub async fn set_enabled(&mut self, enabled: bool) -> anyhow::Result<()> {
        if enabled && !self.enabled {
            let mut kb = self.keyboard.lock().await;
            kb.start()?;
            self.enabled = true;
            tracing::info!("Keyboard passthrough enabled");
        } else if !enabled && self.enabled {
            let mut kb = self.keyboard.lock().await;
            kb.stop();
            self.enabled = false;
            tracing::info!("Keyboard passthrough disabled");
        }
        Ok(())
    }

    /// Check if keyboard passthrough is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get click sender for external use
    pub fn click_sender(&self) -> mpsc::UnboundedSender<(String, f64, f64)> {
        self.click_tx.clone()
    }
}

impl Default for InputEventProcessor {
    fn default() -> Self {
        Self::new().0
    }
}

