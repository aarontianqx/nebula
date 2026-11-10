//! Turns the stream of raw input events into the steps of a macro: press/release pairs fold into
//! clicks, double clicks, drags and key taps, and straight cursor paths collapse to their endpoints.

use serde::{Deserialize, Serialize};
use std::time::Instant;
use tap_core::{normalize_key, DslAction, DslTimedAction, DslValue, MouseButton};
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
    /// `Drag` and `KeyTap` actions when the recording stops.
    #[serde(default = "default_true")]
    pub synthesize_actions: bool,
    /// Drop collinear intermediate mouse-move samples when the recording stops, so straight
    /// cursor paths collapse to their endpoints.
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
    /// Recording completed.
    RecordingCompleted { steps: Vec<DslTimedAction> },
}

/// A raw event waiting for the recording to end.
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

/// Buffers raw input events while recording and turns them into steps when it stops.
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

    /// Stop recording and generate the steps.
    pub fn stop(&mut self) -> Option<RecorderEvent> {
        if self.state == RecorderState::Idle {
            return None;
        }

        let _old = self.state;
        self.state = RecorderState::Idle;

        let steps = self.generate_steps();
        info!("Recording stopped, generated {} steps", steps.len());

        Some(RecorderEvent::RecordingCompleted { steps })
    }

    /// Push a raw event into the buffer.
    pub fn push_event(&mut self, timestamp_ms: u64, event: RawEventType) -> Option<RecorderEvent> {
        if self.state != RecorderState::Recording {
            return None;
        }

        // Canonicalize key names at capture time so the saved macro uses a
        // single vocabulary regardless of which platform hook produced it.
        let event = match event {
            RawEventType::KeyDown { key } => RawEventType::KeyDown {
                key: normalize_key(&key),
            },
            RawEventType::KeyUp { key } => RawEventType::KeyUp {
                key: normalize_key(&key),
            },
            other => other,
        };

        // Adjust timestamp for paused time
        let adjusted_ts = timestamp_ms.saturating_sub(self.total_paused_ms);

        // Apply noise reduction for mouse move
        if matches!(event, RawEventType::MouseMove { .. }) {
            if !self.config.record_mouse_move {
                return None;
            }
            // Sample-rate limiting: always record the first move so the initial
            // cursor position is captured, then throttle subsequent samples.
            if let Some(last) = self.last_move_time_ms {
                if adjusted_ts < last + self.config.move_sample_interval_ms {
                    return None;
                }
            }
            self.last_move_time_ms = Some(adjusted_ts);
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

    /// The macro steps for the buffered events: synthesis folds raw transitions into gestures, then
    /// the move merge drops intermediate samples of straight paths. Both are optional.
    fn generate_steps(&self) -> Vec<DslTimedAction> {
        let mut actions = if self.config.synthesize_actions {
            self.synthesize_actions()
        } else {
            self.events
                .iter()
                .map(|b| (b.timestamp_ms, raw_action(&b.event)))
                .collect()
        };

        if self.config.merge_moves {
            actions = merge_collinear_moves(actions);
        }

        actions
            .into_iter()
            .map(|(at_ms, action)| DslTimedAction::new(at_ms, action))
            .collect()
    }

    /// Fold raw button/key transitions into high-level actions.
    fn synthesize_actions(&self) -> Vec<(u64, DslAction)> {
        let events = &self.events;
        let mut out: Vec<(u64, DslAction)> = Vec::new();
        let mut i = 0;

        while i < events.len() {
            let buffered = &events[i];
            match &buffered.event {
                RawEventType::MouseDown { x, y, button } => {
                    let down_ts = buffered.timestamp_ms;
                    let down_pos = (*x, *y);

                    if let Some((up_idx, only_moves)) = find_matching_up(events, i, *button) {
                        if only_moves {
                            let up = &events[up_idx];
                            let up_pos = match &up.event {
                                RawEventType::MouseUp { x, y, .. } => (*x, *y),
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
                                out.push((down_ts, DslAction::drag(down_pos, up_pos, dt)));
                                i = up_idx + 1;
                                continue;
                            }
                            // Slow in-place hold: keep raw down/up, let the
                            // loop emit them (and any moves) individually.
                        }
                    }

                    out.push((down_ts, raw_action(&buffered.event)));
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
                                    DslAction::KeyTap { key: key.clone() },
                                ));
                                i += 2;
                                continue;
                            }
                        }
                    }
                    out.push((buffered.timestamp_ms, raw_action(&buffered.event)));
                    i += 1;
                }

                _ => {
                    out.push((buffered.timestamp_ms, raw_action(&buffered.event)));
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

/// One raw event as the action that replays it as is.
fn raw_action(event: &RawEventType) -> DslAction {
    match event {
        RawEventType::MouseMove { x, y } => DslAction::mouse_move(*x, *y),
        RawEventType::MouseDown { x, y, button } => DslAction::mouse_down(*x, *y, (*button).into()),
        RawEventType::MouseUp { x, y, button } => DslAction::mouse_up(*x, *y, (*button).into()),
        RawEventType::Scroll { delta_x, delta_y } => DslAction::scroll(*delta_x, *delta_y),
        RawEventType::KeyDown { key } => DslAction::KeyDown { key: key.clone() },
        RawEventType::KeyUp { key } => DslAction::KeyUp { key: key.clone() },
    }
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
fn push_click(out: &mut Vec<(u64, DslAction)>, ts: u64, pos: (i32, i32), button: MouseButton) {
    let doubled = match out.last() {
        Some((prev_ts, DslAction::Click { x, y, button: prev })) => {
            literal_point(x, y).filter(|prev_pos| {
                MouseButton::from(*prev) == button
                    && ts.saturating_sub(*prev_ts) <= DOUBLE_CLICK_MAX_MS
                    && distance(*prev_pos, pos) <= DOUBLE_CLICK_MAX_DIST
            })
        }
        _ => None,
    };

    match doubled {
        Some((x, y)) => out.last_mut().unwrap().1 = DslAction::double_click(x, y, button),
        None => out.push((ts, DslAction::click(pos.0, pos.1, button))),
    }
}

/// The recorder only writes integer literals; anything else is not a recorded position.
fn literal_point(x: &DslValue, y: &DslValue) -> Option<(i32, i32)> {
    Some((x.as_i32()?, y.as_i32()?))
}

fn move_point(action: &DslAction) -> Option<(i32, i32)> {
    match action {
        DslAction::MouseMove { x, y } => literal_point(x, y),
        _ => None,
    }
}

type MoveStep = ((u64, DslAction), (i32, i32));

/// Collapse runs of consecutive `MouseMove` actions, dropping intermediate
/// samples whose perpendicular distance from the straight line between their
/// neighbours is within [`MOVE_SIMPLIFY_TOL`].
fn merge_collinear_moves(actions: Vec<(u64, DslAction)>) -> Vec<(u64, DslAction)> {
    let mut out = Vec::with_capacity(actions.len());
    let mut run: Vec<MoveStep> = Vec::new();

    for step in actions {
        match move_point(&step.1) {
            Some(point) => run.push((step, point)),
            None => {
                simplify_move_run(std::mem::take(&mut run), &mut out);
                out.push(step);
            }
        }
    }
    simplify_move_run(run, &mut out);
    out
}

/// Append a run of moves, keeping its endpoints and the samples where the path turns.
fn simplify_move_run(run: Vec<MoveStep>, out: &mut Vec<(u64, DslAction)>) {
    let points: Vec<(i32, i32)> = run.iter().map(|(_, point)| *point).collect();
    let last = points.len().saturating_sub(1);
    let mut last_kept = None;

    for (i, (step, point)) in run.into_iter().enumerate() {
        let turns = |from: (i32, i32)| {
            perpendicular_distance(point, from, points[i + 1]) > MOVE_SIMPLIFY_TOL
        };
        let keep = i == 0 || i == last || last_kept.is_some_and(turns);
        if keep {
            last_kept = Some(point);
            out.push(step);
        }
    }
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
    fn recorded(config: RecorderConfig, events: &[(u64, RawEventType)]) -> Vec<DslAction> {
        let mut recorder = Recorder::new(config);
        recorder.start();
        for (ts, ev) in events {
            recorder.push_event(*ts, ev.clone());
        }
        match recorder.stop() {
            Some(RecorderEvent::RecordingCompleted { steps }) => {
                steps.into_iter().map(|step| step.action).collect()
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
        assert_eq!(actions, vec![DslAction::click(100, 200, MouseButton::Left)]);
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
            vec![DslAction::double_click(50, 50, MouseButton::Left)]
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
        assert_eq!(actions, vec![DslAction::drag((10, 10), (100, 100), 30)]);
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
        assert_eq!(actions, vec![DslAction::KeyTap { key: "a".into() }]);
    }

    #[test]
    fn modifier_combo_normalizes_modifier_and_taps_inner_key() {
        // Raw hook spellings are canonicalized at capture time, while the
        // standalone modifier press/release stays a raw KeyDown/KeyUp pair.
        let actions = recorded(
            synth_config(),
            &[
                (
                    0,
                    RawEventType::KeyDown {
                        key: "ShiftLeft".into(),
                    },
                ),
                (10, RawEventType::KeyDown { key: "a".into() }),
                (30, RawEventType::KeyUp { key: "a".into() }),
                (
                    40,
                    RawEventType::KeyUp {
                        key: "ShiftLeft".into(),
                    },
                ),
            ],
        );
        assert_eq!(
            actions,
            vec![
                DslAction::KeyDown {
                    key: "Shift".into()
                },
                DslAction::KeyTap { key: "a".into() },
                DslAction::KeyUp {
                    key: "Shift".into()
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
            vec![DslAction::mouse_move(0, 0), DslAction::mouse_move(40, 40),]
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
                DslAction::mouse_move(0, 0),
                DslAction::mouse_move(10, 0),
                DslAction::mouse_move(10, 10),
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
                DslAction::mouse_down(5, 5, MouseButton::Left),
                DslAction::mouse_up(5, 5, MouseButton::Left),
            ]
        );
    }
}
