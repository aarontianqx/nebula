use std::sync::Arc;

use tokio::sync::{mpsc, Mutex, RwLock};

use crate::application::input::gesture::{Gesture, GestureRecognizer};
use crate::infrastructure::input::{create_keyboard_listener, KeyboardListener, RawKeyEvent};

/// Click event to be sent to coordinator
#[derive(Debug, Clone)]
pub struct ClickEvent {
    pub session_id: String,
    pub x: f64,
    pub y: f64,
}

/// Shared cursor state accessible from multiple contexts
#[derive(Debug, Default)]
pub struct CursorState {
    pub position: Option<(i32, i32)>,
    pub in_canvas: bool,
    pub active_session: Option<String>,
}

/// Input event processor that manages keyboard listening and gesture recognition
pub struct InputEventProcessor {
    keyboard: Arc<Mutex<Box<dyn KeyboardListener>>>,
    cursor_state: Arc<RwLock<CursorState>>,
    enabled: Arc<RwLock<bool>>,
    click_tx: mpsc::UnboundedSender<ClickEvent>,
    processing_started: Arc<RwLock<bool>>,
}

impl InputEventProcessor {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ClickEvent>) {
        let (click_tx, click_rx) = mpsc::unbounded_channel();
        let keyboard = create_keyboard_listener();

        (
            Self {
                keyboard: Arc::new(Mutex::new(keyboard)),
                cursor_state: Arc::new(RwLock::new(CursorState::default())),
                enabled: Arc::new(RwLock::new(false)),
                click_tx,
                processing_started: Arc::new(RwLock::new(false)),
            },
            click_rx,
        )
    }

    /// Start the input processing loop
    /// This should be called once during app initialization
    pub async fn start_processing(&self) {
        // Check if already started
        {
            let mut started = self.processing_started.write().await;
            if *started {
                tracing::warn!("Input processing already started");
                return;
            }
            *started = true;
        }

        let keyboard = self.keyboard.clone();
        let cursor_state = self.cursor_state.clone();
        let enabled = self.enabled.clone();
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

        // Create gesture recognizer channel
        let (gesture_tx, mut gesture_rx) = mpsc::unbounded_channel();
        let mut recognizer = GestureRecognizer::new(gesture_tx);

        // Spawn keyboard event processing task
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(raw_event) = keyboard_rx.recv() => {
                        recognizer.process(raw_event);
                    }
                    Some(gesture) = gesture_rx.recv() => {
                        // Check if enabled
                        if !*enabled.read().await {
                            continue;
                        }

                        // Handle gesture - convert to click
                        match gesture {
                            Gesture::Tap { key } | 
                            Gesture::LongPressStart { key } | 
                            Gesture::LongPressRepeat { key } => {
                                let state = cursor_state.read().await;
                                
                                // Only click if cursor is in canvas and we have an active session
                                if state.in_canvas {
                                    if let (Some((x, y)), Some(session_id)) = 
                                        (state.position, state.active_session.clone()) 
                                    {
                                        tracing::debug!(
                                            "Keyboard passthrough: {:?} -> click at ({}, {}) for session {}",
                                            key, x, y, session_id
                                        );
                                        
                                        let _ = click_tx.send(ClickEvent {
                                            session_id,
                                            x: x as f64,
                                            y: y as f64,
                                        });
                                    }
                                }
                            }
                            Gesture::LongPressEnd { .. } => {
                                // No action needed on release
                            }
                        }
                    }
                }
            }
        });

        tracing::info!("Input processing started");
    }

    /// Update cursor position
    pub async fn update_cursor(&self, x: i32, y: i32, in_canvas: bool) {
        let mut state = self.cursor_state.write().await;
        state.position = Some((x, y));
        state.in_canvas = in_canvas;
    }

    /// Set active session for click events
    pub async fn set_active_session(&self, session_id: Option<String>) {
        let mut state = self.cursor_state.write().await;
        state.active_session = session_id;
    }

    /// Enable or disable keyboard passthrough
    pub async fn set_enabled(&self, enabled: bool) -> anyhow::Result<()> {
        let current = *self.enabled.read().await;
        
        if enabled && !current {
            let mut kb = self.keyboard.lock().await;
            kb.start()?;
            *self.enabled.write().await = true;
            tracing::info!("Keyboard passthrough enabled");
        } else if !enabled && current {
            let mut kb = self.keyboard.lock().await;
            kb.stop();
            *self.enabled.write().await = false;
            tracing::info!("Keyboard passthrough disabled");
        }
        Ok(())
    }

    /// Check if keyboard passthrough is enabled
    pub async fn is_enabled(&self) -> bool {
        *self.enabled.read().await
    }

    /// Get click sender for external use
    pub fn click_sender(&self) -> mpsc::UnboundedSender<ClickEvent> {
        self.click_tx.clone()
    }
}

impl Default for InputEventProcessor {
    fn default() -> Self {
        Self::new().0
    }
}
