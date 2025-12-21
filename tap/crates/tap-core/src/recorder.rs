//! Recording engine: captures input events and converts to Timeline.

use crate::{Action, MouseButton, TimedAction, Timeline};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, info};

/// Configuration for the recorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecorderConfig {
    /// Minimum interval between mouse move events (ms).
    /// Events within this window are dropped to reduce noise.
    pub move_sample_interval_ms: u64,
    /// Whether to record mouse move events.
    pub record_mouse_move: bool,
    /// Whether to record scroll events.
    pub record_scroll: bool,
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            move_sample_interval_ms: 50,
            record_mouse_move: true,
            record_scroll: true,
        }
    }
}

/// State of the recorder.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecorderState {
    /// Not recording.
    Idle,
    /// Recording in progress.
    Recording,
    /// Recording paused.
    Paused,
}

impl Default for RecorderState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Events emitted by the recorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderEvent {
    /// State changed.
    StateChanged { old: RecorderState, new: RecorderState },
    /// Event captured (for UI feedback).
    EventCaptured { event_count: usize, duration_ms: u64 },
    /// Recording completed, timeline generated.
    RecordingCompleted { timeline: Timeline },
}

/// A buffered raw event before conversion to Action.
#[derive(Debug, Clone)]
pub struct BufferedEvent {
    pub timestamp_ms: u64,
    pub event: RawEventType,
}

/// Raw event types from input hook.
#[derive(Debug, Clone)]
pub enum RawEventType {
    MouseMove { x: i32, y: i32 },
    MouseDown { x: i32, y: i32, button: MouseButtonRaw },
    MouseUp { x: i32, y: i32, button: MouseButtonRaw },
    Scroll { delta_x: i32, delta_y: i32 },
    KeyDown { key: String },
    KeyUp { key: String },
}

/// Raw mouse button (from platform layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButtonRaw {
    Left,
    Right,
    Middle,
    Unknown,
}

impl From<MouseButtonRaw> for MouseButton {
    fn from(raw: MouseButtonRaw) -> Self {
        match raw {
            MouseButtonRaw::Left => MouseButton::Left,
            MouseButtonRaw::Right => MouseButton::Right,
            MouseButtonRaw::Middle => MouseButton::Middle,
            MouseButtonRaw::Unknown => MouseButton::Left, // fallback
        }
    }
}

/// The recorder collects raw input events and converts them to a Timeline.
pub struct Recorder {
    config: RecorderConfig,
    state: RecorderState,
    events: Vec<BufferedEvent>,
    start_time: Option<Instant>,
    pause_time: Option<Instant>,
    total_paused_ms: u64,
    last_move_time_ms: u64,
    last_mouse_pos: (i32, i32),
}

impl Recorder {
    /// Create a new recorder with the given configuration.
    pub fn new(config: RecorderConfig) -> Self {
        Self {
            config,
            state: RecorderState::Idle,
            events: Vec::new(),
            start_time: None,
            pause_time: None,
            total_paused_ms: 0,
            last_move_time_ms: 0,
            last_mouse_pos: (0, 0),
        }
    }

    /// Create a recorder with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(RecorderConfig::default())
    }

    /// Get current state.
    pub fn state(&self) -> RecorderState {
        self.state
    }

    /// Get the number of captured events.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    /// Get recording duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        match self.start_time {
            Some(start) => {
                let elapsed = start.elapsed().as_millis() as u64;
                elapsed.saturating_sub(self.total_paused_ms)
            }
            None => 0,
        }
    }

    /// Start recording.
    pub fn start(&mut self) -> Option<RecorderEvent> {
        if self.state != RecorderState::Idle {
            return None;
        }

        let old = self.state;
        self.state = RecorderState::Recording;
        self.events.clear();
        self.start_time = Some(Instant::now());
        self.pause_time = None;
        self.total_paused_ms = 0;
        self.last_move_time_ms = 0;
        self.last_mouse_pos = (0, 0);

        info!("Recording started");
        Some(RecorderEvent::StateChanged {
            old,
            new: self.state,
        })
    }

    /// Pause recording.
    pub fn pause(&mut self) -> Option<RecorderEvent> {
        if self.state != RecorderState::Recording {
            return None;
        }

        let old = self.state;
        self.state = RecorderState::Paused;
        self.pause_time = Some(Instant::now());

        info!("Recording paused");
        Some(RecorderEvent::StateChanged {
            old,
            new: self.state,
        })
    }

    /// Resume recording.
    pub fn resume(&mut self) -> Option<RecorderEvent> {
        if self.state != RecorderState::Paused {
            return None;
        }

        // Add paused duration to total
        if let Some(pause_start) = self.pause_time.take() {
            self.total_paused_ms += pause_start.elapsed().as_millis() as u64;
        }

        let old = self.state;
        self.state = RecorderState::Recording;

        info!("Recording resumed");
        Some(RecorderEvent::StateChanged {
            old,
            new: self.state,
        })
    }

    /// Stop recording and generate timeline.
    pub fn stop(&mut self) -> Option<RecorderEvent> {
        if self.state == RecorderState::Idle {
            return None;
        }

        let _old = self.state;
        self.state = RecorderState::Idle;

        let timeline = self.generate_timeline();
        info!(
            "Recording stopped, generated {} actions",
            timeline.actions.len()
        );

        Some(RecorderEvent::RecordingCompleted { timeline })
    }

    /// Push a raw event into the buffer.
    pub fn push_event(&mut self, timestamp_ms: u64, event: RawEventType) -> Option<RecorderEvent> {
        if self.state != RecorderState::Recording {
            return None;
        }

        // Adjust timestamp for paused time
        let adjusted_ts = timestamp_ms.saturating_sub(self.total_paused_ms);

        // Apply noise reduction for mouse move
        if let RawEventType::MouseMove { x, y } = &event {
            if !self.config.record_mouse_move {
                return None;
            }
            // Sample rate limiting
            if adjusted_ts < self.last_move_time_ms + self.config.move_sample_interval_ms {
                // Still update last known position for button events
                self.last_mouse_pos = (*x, *y);
                return None;
            }
            self.last_move_time_ms = adjusted_ts;
            self.last_mouse_pos = (*x, *y);
        }

        // Update mouse position from move events
        if let RawEventType::MouseMove { x, y } = &event {
            self.last_mouse_pos = (*x, *y);
        }

        // Skip scroll if disabled
        if matches!(event, RawEventType::Scroll { .. }) && !self.config.record_scroll {
            return None;
        }

        debug!(?adjusted_ts, ?event, "Recording event");

        self.events.push(BufferedEvent {
            timestamp_ms: adjusted_ts,
            event,
        });

        Some(RecorderEvent::EventCaptured {
            event_count: self.events.len(),
            duration_ms: self.duration_ms(),
        })
    }

    /// Get the last known mouse position.
    pub fn last_mouse_position(&self) -> (i32, i32) {
        self.last_mouse_pos
    }

    /// Generate a Timeline from the buffered events.
    fn generate_timeline(&self) -> Timeline {
        let mut actions = Vec::new();

        for buffered in &self.events {
            let action = match &buffered.event {
                RawEventType::MouseMove { x, y } => Action::MouseMove { x: *x, y: *y },
                RawEventType::MouseDown { x, y, button } => {
                    // Use last known position if x/y are 0
                    let (px, py) = if *x == 0 && *y == 0 {
                        self.last_mouse_pos
                    } else {
                        (*x, *y)
                    };
                    Action::MouseDown {
                        x: px,
                        y: py,
                        button: (*button).into(),
                    }
                }
                RawEventType::MouseUp { x, y, button } => {
                    let (px, py) = if *x == 0 && *y == 0 {
                        self.last_mouse_pos
                    } else {
                        (*x, *y)
                    };
                    Action::MouseUp {
                        x: px,
                        y: py,
                        button: (*button).into(),
                    }
                }
                RawEventType::Scroll { delta_x, delta_y } => Action::Scroll {
                    delta_x: *delta_x,
                    delta_y: *delta_y,
                },
                RawEventType::KeyDown { key } => Action::KeyDown { key: key.clone() },
                RawEventType::KeyUp { key } => Action::KeyUp { key: key.clone() },
            };

            actions.push(TimedAction {
                at_ms: buffered.timestamp_ms,
                action,
                enabled: true,
                note: None,
            });
        }

        Timeline { actions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recorder_basic() {
        let mut recorder = Recorder::with_defaults();
        assert_eq!(recorder.state(), RecorderState::Idle);

        recorder.start();
        assert_eq!(recorder.state(), RecorderState::Recording);

        recorder.push_event(0, RawEventType::MouseMove { x: 100, y: 200 });
        recorder.push_event(
            100,
            RawEventType::MouseDown {
                x: 100,
                y: 200,
                button: MouseButtonRaw::Left,
            },
        );
        recorder.push_event(
            150,
            RawEventType::MouseUp {
                x: 100,
                y: 200,
                button: MouseButtonRaw::Left,
            },
        );

        assert_eq!(recorder.event_count(), 3);

        let event = recorder.stop();
        assert!(matches!(
            event,
            Some(RecorderEvent::RecordingCompleted { .. })
        ));
        assert_eq!(recorder.state(), RecorderState::Idle);
    }

    #[test]
    fn test_move_noise_reduction() {
        let config = RecorderConfig {
            move_sample_interval_ms: 50,
            record_mouse_move: true,
            record_scroll: true,
        };
        let mut recorder = Recorder::new(config);
        recorder.start();

        // First move should be recorded
        recorder.push_event(0, RawEventType::MouseMove { x: 100, y: 100 });
        assert_eq!(recorder.event_count(), 1);

        // Move within 50ms should be dropped
        recorder.push_event(30, RawEventType::MouseMove { x: 110, y: 110 });
        assert_eq!(recorder.event_count(), 1);

        // Move after 50ms should be recorded
        recorder.push_event(60, RawEventType::MouseMove { x: 120, y: 120 });
        assert_eq!(recorder.event_count(), 2);
    }
}

