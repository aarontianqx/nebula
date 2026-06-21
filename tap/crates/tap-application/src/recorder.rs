//! Recording engine: captures input events and converts to Timeline.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use tap_core::{Action, MouseButton, Point, TimedAction, Timeline};
use tracing::{debug, info};

/// A press/release closer than this in time *and* space becomes a `Click`.
const CLICK_MAX_MS: u64 = 400;
const CLICK_MAX_DIST: f64 = 5.0;
/// Two clicks closer than this become a `DoubleClick`.
const DOUBLE_CLICK_MAX_MS: u64 = 350;
const DOUBLE_CLICK_MAX_DIST: f64 = 6.0;
/// A press/release that travels at least this far becomes a `Drag`.
const DRAG_MIN_DIST: f64 = 5.0;
/// A key press/release closer than this in time becomes a `KeyTap`.
const KEYTAP_MAX_MS: u64 = 600;
/// Perpendicular tolerance (px) for dropping collinear intermediate moves.
const MOVE_SIMPLIFY_TOL: f64 = 2.0;

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
    /// Fold raw button/key transitions into high-level `Click`, `DoubleClick`,
    /// `Drag` and `KeyTap` actions when generating the timeline.
    #[serde(default = "default_true")]
    pub synthesize_actions: bool,
    /// Drop collinear intermediate mouse-move samples when generating the
    /// timeline so straight cursor paths collapse to their endpoints.
    #[serde(default = "default_true")]
    pub merge_moves: bool,
}

fn default_true() -> bool {
    true
}

impl Default for RecorderConfig {
    fn default() -> Self {
        Self {
            move_sample_interval_ms: 50,
            record_mouse_move: true,
            record_scroll: true,
            synthesize_actions: true,
            merge_moves: true,
        }
    }
}

/// State of the recorder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecorderState {
    /// Not recording.
    #[default]
    Idle,
    /// Recording in progress.
    Recording,
    /// Recording paused.
    Paused,
}

/// Events emitted by the recorder.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecorderEvent {
    /// State changed.
    StateChanged {
        old: RecorderState,
        new: RecorderState,
    },
    /// Event captured (for UI feedback).
    EventCaptured {
        event_count: usize,
        duration_ms: u64,
    },
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
    MouseMove {
        x: i32,
        y: i32,
    },
    MouseDown {
        x: i32,
        y: i32,
        button: MouseButtonRaw,
    },
    MouseUp {
        x: i32,
        y: i32,
        button: MouseButtonRaw,
    },
    Scroll {
        delta_x: i32,
        delta_y: i32,
    },
    KeyDown {
        key: String,
    },
    KeyUp {
        key: String,
    },
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
    /// Timestamp of the last recorded mouse-move sample. `None` until the first
    /// move is captured so that the initial position is never throttled away.
    last_move_time_ms: Option<u64>,
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
            last_move_time_ms: None,
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
        self.last_move_time_ms = None;
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
            // Sample-rate limiting: always record the first move so the initial
            // cursor position is captured, then throttle subsequent samples.
            if let Some(last) = self.last_move_time_ms {
                if adjusted_ts < last + self.config.move_sample_interval_ms {
                    // Still update last known position for button events.
                    self.last_mouse_pos = (*x, *y);
                    return None;
                }
            }
            self.last_move_time_ms = Some(adjusted_ts);
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

    /// Resolve an event position, falling back to the last known cursor
    /// position when the platform reported `(0, 0)` (some backends omit the
    /// coordinate on button events).
    fn resolve_pos(&self, x: i32, y: i32) -> (i32, i32) {
        if x == 0 && y == 0 {
            self.last_mouse_pos
        } else {
            (x, y)
        }
    }

    /// Generate a Timeline from the buffered events.
    ///
    /// Runs two optional denoising passes:
    /// 1. **Synthesis** -- fold raw press/release transitions into high-level
    ///    `Click` / `DoubleClick` / `Drag` / `KeyTap` actions.
    /// 2. **Move merge** -- drop collinear intermediate mouse-move samples.
    fn generate_timeline(&self) -> Timeline {
        let mut actions = if self.config.synthesize_actions {
            self.synthesize_actions()
        } else {
            self.events
                .iter()
                .map(|b| (b.timestamp_ms, self.raw_action(&b.event)))
                .collect()
        };

        if self.config.merge_moves {
            actions = merge_collinear_moves(actions);
        }

        Timeline {
            actions: actions
                .into_iter()
                .map(|(at_ms, action)| TimedAction {
                    at_ms,
                    action,
                    enabled: true,
                    note: None,
                })
                .collect(),
        }
    }

    /// Map a single raw event to its 1:1 `Action` (no synthesis).
    fn raw_action(&self, event: &RawEventType) -> Action {
        match event {
            RawEventType::MouseMove { x, y } => Action::MouseMove { x: *x, y: *y },
            RawEventType::MouseDown { x, y, button } => {
                let (px, py) = self.resolve_pos(*x, *y);
                Action::MouseDown {
                    x: px,
                    y: py,
                    button: (*button).into(),
                }
            }
            RawEventType::MouseUp { x, y, button } => {
                let (px, py) = self.resolve_pos(*x, *y);
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
        }
    }

    /// Fold raw button/key transitions into high-level actions.
    fn synthesize_actions(&self) -> Vec<(u64, Action)> {
        let events = &self.events;
        let mut out: Vec<(u64, Action)> = Vec::new();
        let mut i = 0;

        while i < events.len() {
            let buffered = &events[i];
            match &buffered.event {
                RawEventType::MouseDown { x, y, button } => {
                    let down_ts = buffered.timestamp_ms;
                    let down_pos = self.resolve_pos(*x, *y);

                    if let Some((up_idx, only_moves)) = find_matching_up(events, i, *button) {
                        if only_moves {
                            let up = &events[up_idx];
                            let up_pos = match &up.event {
                                RawEventType::MouseUp { x, y, .. } => self.resolve_pos(*x, *y),
                                _ => down_pos,
                            };
                            let dt = up.timestamp_ms.saturating_sub(down_ts);
                            let travel = distance(down_pos, up_pos);
                            let mb: MouseButton = (*button).into();

                            if dt <= CLICK_MAX_MS && travel <= CLICK_MAX_DIST {
                                push_click(&mut out, down_ts, down_pos, mb);
                                i = up_idx + 1;
                                continue;
                            } else if travel >= DRAG_MIN_DIST {
                                out.push((
                                    down_ts,
                                    Action::Drag {
                                        from: Point {
                                            x: down_pos.0,
                                            y: down_pos.1,
                                        },
                                        to: Point {
                                            x: up_pos.0,
                                            y: up_pos.1,
                                        },
                                        duration_ms: dt,
                                    },
                                ));
                                i = up_idx + 1;
                                continue;
                            }
                            // Slow in-place hold: keep raw down/up, let the
                            // loop emit them (and any moves) individually.
                        }
                    }

                    out.push((down_ts, self.raw_action(&buffered.event)));
                    i += 1;
                }

                RawEventType::KeyDown { key } => {
                    if let Some(next) = events.get(i + 1) {
                        if let RawEventType::KeyUp { key: up_key } = &next.event {
                            if up_key == key
                                && next.timestamp_ms.saturating_sub(buffered.timestamp_ms)
                                    <= KEYTAP_MAX_MS
                            {
                                out.push((
                                    buffered.timestamp_ms,
                                    Action::KeyTap { key: key.clone() },
                                ));
                                i += 2;
                                continue;
                            }
                        }
                    }
                    out.push((buffered.timestamp_ms, self.raw_action(&buffered.event)));
                    i += 1;
                }

                _ => {
                    out.push((buffered.timestamp_ms, self.raw_action(&buffered.event)));
                    i += 1;
                }
            }
        }

        out
    }
}

/// Euclidean distance between two integer points.
fn distance(a: (i32, i32), b: (i32, i32)) -> f64 {
    let dx = (a.0 - b.0) as f64;
    let dy = (a.1 - b.1) as f64;
    (dx * dx + dy * dy).sqrt()
}

/// Find the next `MouseUp` for `button` after `down_idx`.
///
/// Returns its index and whether every event in between is a `MouseMove`
/// (i.e. the press/release pair can be folded into a single gesture).
fn find_matching_up(
    events: &[BufferedEvent],
    down_idx: usize,
    button: MouseButtonRaw,
) -> Option<(usize, bool)> {
    let mut only_moves = true;
    for (offset, ev) in events.iter().enumerate().skip(down_idx + 1) {
        match &ev.event {
            RawEventType::MouseUp { button: b, .. } if *b == button => {
                return Some((offset, only_moves));
            }
            RawEventType::MouseMove { .. } => {}
            _ => only_moves = false,
        }
    }
    None
}

/// Push a `Click`, upgrading the previous action to `DoubleClick` when the two
/// clicks are close enough in time and space.
fn push_click(out: &mut Vec<(u64, Action)>, ts: u64, pos: (i32, i32), button: MouseButton) {
    let upgrade = match out.last() {
        Some((prev_ts, Action::Click { x, y, button: pb }))
            if *pb == button
                && ts.saturating_sub(*prev_ts) <= DOUBLE_CLICK_MAX_MS
                && distance((*x, *y), pos) <= DOUBLE_CLICK_MAX_DIST =>
        {
            Some((*x, *y, *pb))
        }
        _ => None,
    };

    match upgrade {
        Some((x, y, button)) => {
            out.last_mut().unwrap().1 = Action::DoubleClick { x, y, button };
        }
        None => out.push((
            ts,
            Action::Click {
                x: pos.0,
                y: pos.1,
                button,
            },
        )),
    }
}

/// Collapse runs of consecutive `MouseMove` actions, dropping intermediate
/// samples whose perpendicular distance from the straight line between their
/// neighbours is within [`MOVE_SIMPLIFY_TOL`].
fn merge_collinear_moves(actions: Vec<(u64, Action)>) -> Vec<(u64, Action)> {
    let mut out: Vec<(u64, Action)> = Vec::with_capacity(actions.len());
    let mut i = 0;

    while i < actions.len() {
        if matches!(actions[i].1, Action::MouseMove { .. }) {
            let start = i;
            while i < actions.len() && matches!(actions[i].1, Action::MouseMove { .. }) {
                i += 1;
            }
            simplify_move_run(&actions[start..i], &mut out);
        } else {
            out.push(actions[i].clone());
            i += 1;
        }
    }

    out
}

/// Append a simplified version of a single run of `MouseMove` actions.
fn simplify_move_run(run: &[(u64, Action)], out: &mut Vec<(u64, Action)>) {
    let pts: Vec<(i32, i32)> = run
        .iter()
        .map(|(_, a)| match a {
            Action::MouseMove { x, y } => (*x, *y),
            _ => unreachable!("run only contains MouseMove"),
        })
        .collect();

    if run.len() <= 2 {
        out.extend_from_slice(run);
        return;
    }

    out.push(run[0].clone());
    let mut last_kept = pts[0];
    for w in 1..run.len() - 1 {
        if perpendicular_distance(pts[w], last_kept, pts[w + 1]) > MOVE_SIMPLIFY_TOL {
            out.push(run[w].clone());
            last_kept = pts[w];
        }
    }
    out.push(run[run.len() - 1].clone());
}

/// Perpendicular distance from point `p` to the line through `a` and `b`.
fn perpendicular_distance(p: (i32, i32), a: (i32, i32), b: (i32, i32)) -> f64 {
    let (px, py) = (p.0 as f64, p.1 as f64);
    let (ax, ay) = (a.0 as f64, a.1 as f64);
    let (bx, by) = (b.0 as f64, b.1 as f64);
    let dx = bx - ax;
    let dy = by - ay;
    let len = (dx * dx + dy * dy).sqrt();
    if len < f64::EPSILON {
        // Degenerate segment: fall back to distance from the shared endpoint.
        return ((px - ax).powi(2) + (py - ay).powi(2)).sqrt();
    }
    ((dx * (ay - py) - (ax - px) * dy).abs()) / len
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
            ..Default::default()
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

    /// Record a sequence of raw events and return the synthesized actions.
    fn recorded(config: RecorderConfig, events: &[(u64, RawEventType)]) -> Vec<Action> {
        let mut recorder = Recorder::new(config);
        recorder.start();
        for (ts, ev) in events {
            recorder.push_event(*ts, ev.clone());
        }
        match recorder.stop() {
            Some(RecorderEvent::RecordingCompleted { timeline }) => {
                timeline.actions.into_iter().map(|a| a.action).collect()
            }
            other => panic!("expected RecordingCompleted, got {:?}", other),
        }
    }

    /// All moves are kept; no synthesis or merge is applied.
    fn raw_config() -> RecorderConfig {
        RecorderConfig {
            move_sample_interval_ms: 0,
            record_mouse_move: true,
            record_scroll: true,
            synthesize_actions: false,
            merge_moves: false,
        }
    }

    fn synth_config() -> RecorderConfig {
        RecorderConfig {
            move_sample_interval_ms: 0,
            ..Default::default()
        }
    }

    #[test]
    fn press_and_release_in_place_becomes_a_click() {
        let actions = recorded(
            synth_config(),
            &[
                (
                    0,
                    RawEventType::MouseDown {
                        x: 100,
                        y: 200,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (
                    60,
                    RawEventType::MouseUp {
                        x: 101,
                        y: 200,
                        button: MouseButtonRaw::Left,
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![Action::Click {
                x: 100,
                y: 200,
                button: MouseButton::Left
            }]
        );
    }

    #[test]
    fn two_quick_clicks_become_a_double_click() {
        let actions = recorded(
            synth_config(),
            &[
                (
                    0,
                    RawEventType::MouseDown {
                        x: 50,
                        y: 50,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (
                    40,
                    RawEventType::MouseUp {
                        x: 50,
                        y: 50,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (
                    120,
                    RawEventType::MouseDown {
                        x: 51,
                        y: 50,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (
                    150,
                    RawEventType::MouseUp {
                        x: 51,
                        y: 50,
                        button: MouseButtonRaw::Left,
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![Action::DoubleClick {
                x: 50,
                y: 50,
                button: MouseButton::Left
            }]
        );
    }

    #[test]
    fn press_move_release_becomes_a_drag_absorbing_moves() {
        let actions = recorded(
            synth_config(),
            &[
                (
                    0,
                    RawEventType::MouseDown {
                        x: 10,
                        y: 10,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (10, RawEventType::MouseMove { x: 40, y: 40 }),
                (20, RawEventType::MouseMove { x: 80, y: 80 }),
                (
                    30,
                    RawEventType::MouseUp {
                        x: 100,
                        y: 100,
                        button: MouseButtonRaw::Left,
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![Action::Drag {
                from: Point { x: 10, y: 10 },
                to: Point { x: 100, y: 100 },
                duration_ms: 30,
            }]
        );
    }

    #[test]
    fn key_press_release_becomes_a_key_tap() {
        let actions = recorded(
            synth_config(),
            &[
                (0, RawEventType::KeyDown { key: "a".into() }),
                (40, RawEventType::KeyUp { key: "a".into() }),
            ],
        );
        assert_eq!(actions, vec![Action::KeyTap { key: "a".into() }]);
    }

    #[test]
    fn modifier_combo_keeps_modifier_raw_and_taps_inner_key() {
        let actions = recorded(
            synth_config(),
            &[
                (
                    0,
                    RawEventType::KeyDown {
                        key: "shift".into(),
                    },
                ),
                (10, RawEventType::KeyDown { key: "a".into() }),
                (30, RawEventType::KeyUp { key: "a".into() }),
                (
                    40,
                    RawEventType::KeyUp {
                        key: "shift".into(),
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![
                Action::KeyDown {
                    key: "shift".into()
                },
                Action::KeyTap { key: "a".into() },
                Action::KeyUp {
                    key: "shift".into()
                },
            ]
        );
    }

    #[test]
    fn collinear_moves_collapse_to_endpoints() {
        let actions = recorded(
            synth_config(),
            &[
                (0, RawEventType::MouseMove { x: 0, y: 0 }),
                (10, RawEventType::MouseMove { x: 10, y: 10 }),
                (20, RawEventType::MouseMove { x: 20, y: 20 }),
                (30, RawEventType::MouseMove { x: 30, y: 30 }),
                (40, RawEventType::MouseMove { x: 40, y: 40 }),
            ],
        );
        assert_eq!(
            actions,
            vec![
                Action::MouseMove { x: 0, y: 0 },
                Action::MouseMove { x: 40, y: 40 },
            ]
        );
    }

    #[test]
    fn corner_in_move_path_is_preserved() {
        let actions = recorded(
            synth_config(),
            &[
                (0, RawEventType::MouseMove { x: 0, y: 0 }),
                (10, RawEventType::MouseMove { x: 5, y: 0 }),
                (20, RawEventType::MouseMove { x: 10, y: 0 }),
                (30, RawEventType::MouseMove { x: 10, y: 5 }),
                (40, RawEventType::MouseMove { x: 10, y: 10 }),
            ],
        );
        assert_eq!(
            actions,
            vec![
                Action::MouseMove { x: 0, y: 0 },
                Action::MouseMove { x: 10, y: 0 },
                Action::MouseMove { x: 10, y: 10 },
            ]
        );
    }

    #[test]
    fn raw_config_disables_synthesis_and_merge() {
        let actions = recorded(
            raw_config(),
            &[
                (
                    0,
                    RawEventType::MouseDown {
                        x: 5,
                        y: 5,
                        button: MouseButtonRaw::Left,
                    },
                ),
                (
                    20,
                    RawEventType::MouseUp {
                        x: 5,
                        y: 5,
                        button: MouseButtonRaw::Left,
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![
                Action::MouseDown {
                    x: 5,
                    y: 5,
                    button: MouseButton::Left
                },
                Action::MouseUp {
                    x: 5,
                    y: 5,
                    button: MouseButton::Left
                },
            ]
        );
    }
}
