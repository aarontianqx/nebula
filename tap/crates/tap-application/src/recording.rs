//! A recording in progress: raw input events in, macro steps out.
//!
//! Wraps the [`Recorder`] with what the platform's events need before they are worth recording:
//! button events that omit their position get the last known one, and when the macro measures
//! positions from a target window they are made relative to that window.

use std::time::{Duration, Instant};

use serde::Serialize;
use tap_core::{DslTimedAction, MacroDocument};
use tracing::debug;

use crate::coords::{relativize_raw, window_binding};
use crate::recorder::{RawEventType, Recorder, RecorderEvent, RecorderState};

/// Finds where a window's top-left corner is on the screen, given its title and/or process.
pub type WindowLocator = Box<dyn Fn(Option<&str>, Option<&str>) -> Option<(i32, i32)> + Send>;

/// How long the window origin found for a mouse move is reused. Moves arrive in floods and the
/// window rarely moves between two of them; button events always look the origin up again.
const MOVE_ORIGIN_CACHE: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Serialize)]
pub struct RecordingStatus {
    pub state: RecorderState,
    pub event_count: usize,
    pub duration_ms: u64,
}

pub struct RecordingSession {
    recorder: Recorder,
    locate: WindowLocator,
    /// Title and process of the window positions are measured from, for the current recording.
    window: Option<(Option<String>, Option<String>)>,
    origins: OriginTracker,
    /// The pointer on the screen, whatever space the recording uses.
    cursor: (i32, i32),
}

impl RecordingSession {
    pub fn new(locate: WindowLocator) -> Self {
        Self {
            recorder: Recorder::with_defaults(),
            locate,
            window: None,
            origins: OriginTracker::default(),
            cursor: (0, 0),
        }
    }

    pub fn state(&self) -> RecorderState {
        self.recorder.state()
    }

    pub fn status(&self) -> RecordingStatus {
        RecordingStatus {
            state: self.recorder.state(),
            event_count: self.recorder.event_count(),
            duration_ms: self.recorder.duration_ms(),
        }
    }

    /// Begin recording for `document`. A window-relative document needs its window on screen, to
    /// measure positions from.
    pub fn start(&mut self, document: &MacroDocument) -> Result<(), String> {
        if self.state() != RecorderState::Idle {
            return Err("Recording already in progress".into());
        }
        let window = window_binding(document).map(|t| (t.title.clone(), t.process.clone()));
        if let Some((title, process)) = &window {
            if (self.locate)(title.as_deref(), process.as_deref()).is_none() {
                return Err(
                    "The target window was not found. Bring it on screen to record positions \
                     relative to it."
                        .into(),
                );
            }
        }
        self.window = window;
        self.origins = OriginTracker::default();
        self.cursor = (0, 0);
        self.recorder.start();
        Ok(())
    }

    pub fn pause(&mut self) -> Result<(), String> {
        if self.state() != RecorderState::Recording {
            return Err("Not recording".into());
        }
        self.recorder.pause();
        Ok(())
    }

    pub fn resume(&mut self) -> Result<(), String> {
        if self.state() != RecorderState::Paused {
            return Err("Not paused".into());
        }
        self.recorder.resume();
        Ok(())
    }

    /// End the recording and return its steps; `None` when nothing was being recorded.
    pub fn stop(&mut self) -> Option<Vec<DslTimedAction>> {
        match self.recorder.stop() {
            Some(RecorderEvent::RecordingCompleted { steps }) => Some(steps),
            _ => None,
        }
    }

    /// Feed one input event. Returns the new status when the event was recorded.
    pub fn push(&mut self, timestamp_ms: u64, event: RawEventType) -> Option<RecordingStatus> {
        let event = self.with_known_position(event);
        let event = self.relative_to_window(event)?;
        match self.recorder.push_event(timestamp_ms, event) {
            Some(RecorderEvent::EventCaptured { .. }) => Some(self.status()),
            _ => None,
        }
    }

    /// Some backends report button events at (0, 0); the pointer was last seen elsewhere.
    fn with_known_position(&mut self, event: RawEventType) -> RawEventType {
        let event = match event {
            RawEventType::MouseDown { x: 0, y: 0, button } => RawEventType::MouseDown {
                x: self.cursor.0,
                y: self.cursor.1,
                button,
            },
            RawEventType::MouseUp { x: 0, y: 0, button } => RawEventType::MouseUp {
                x: self.cursor.0,
                y: self.cursor.1,
                button,
            },
            other => other,
        };
        if let Some(position) = position_of(&event) {
            self.cursor = position;
        }
        event
    }

    /// `None` drops the event: it has a position but the window to measure it from is unknown.
    fn relative_to_window(&mut self, event: RawEventType) -> Option<RawEventType> {
        let Some((title, process)) = &self.window else {
            return Some(event);
        };
        // Button events are the ones that matter, so their origin is never served from the cache.
        let precise = !matches!(event, RawEventType::MouseMove { .. });
        let locate = &self.locate;
        let origin = self.origins.origin(Instant::now(), precise, || {
            locate(title.as_deref(), process.as_deref())
        });
        match origin {
            Some(origin) => Some(relativize_raw(event, origin)),
            None if position_of(&event).is_some() => {
                debug!("target window not found, dropping a positioned event");
                None
            }
            None => Some(event),
        }
    }
}

fn position_of(event: &RawEventType) -> Option<(i32, i32)> {
    match event {
        RawEventType::MouseMove { x, y }
        | RawEventType::MouseDown { x, y, .. }
        | RawEventType::MouseUp { x, y, .. } => Some((*x, *y)),
        _ => None,
    }
}

/// The recorded window's origin, as of the last successful lookup.
#[derive(Default)]
struct OriginTracker {
    last: Option<((i32, i32), Instant)>,
}

impl OriginTracker {
    /// The window origin at `now`. `precise` forces a fresh `lookup`; otherwise a result younger
    /// than `MOVE_ORIGIN_CACHE` is reused. When the window cannot be found the last known origin
    /// stands in, so a press and its release (or a drag that ends outside) stay consistent; with
    /// none known the answer is `None`.
    fn origin(
        &mut self,
        now: Instant,
        precise: bool,
        lookup: impl FnOnce() -> Option<(i32, i32)>,
    ) -> Option<(i32, i32)> {
        if !precise {
            if let Some((origin, at)) = self.last {
                if now.saturating_duration_since(at) < MOVE_ORIGIN_CACHE {
                    return Some(origin);
                }
            }
        }
        match lookup() {
            Some(origin) => {
                self.last = Some((origin, now));
                Some(origin)
            }
            None => self.last.map(|(origin, _)| origin),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tap_core::{CoordinateSpace, DslAction, DslTargetWindow};

    use super::*;
    use crate::recorder::MouseButtonRaw;

    #[test]
    fn button_events_always_look_the_origin_up_again() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        assert_eq!(tracker.origin(t0, true, || Some((10, 20))), Some((10, 20)));
        // A second precise lookup right away still asks the platform (the window may have moved).
        assert_eq!(tracker.origin(t0, true, || Some((30, 40))), Some((30, 40)));
    }

    #[test]
    fn move_events_reuse_a_fresh_origin_and_refresh_a_stale_one() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        tracker.origin(t0, true, || Some((10, 20)));

        let soon = t0 + Duration::from_millis(50);
        assert_eq!(
            tracker.origin(soon, false, || panic!("must be served from the cache")),
            Some((10, 20))
        );

        let later = t0 + Duration::from_millis(150);
        assert_eq!(
            tracker.origin(later, false, || Some((99, 99))),
            Some((99, 99))
        );
    }

    #[test]
    fn a_lost_window_falls_back_to_the_last_known_origin_and_then_to_none() {
        let mut tracker = OriginTracker::default();
        let t0 = Instant::now();
        assert_eq!(tracker.origin(t0, true, || None), None);

        tracker.origin(t0, true, || Some((10, 20)));
        assert_eq!(tracker.origin(t0, true, || None), Some((10, 20)));
    }

    type SharedOrigin = Arc<Mutex<Option<(i32, i32)>>>;

    fn session(origin: SharedOrigin) -> RecordingSession {
        RecordingSession::new(Box::new(move |_, _| *origin.lock().unwrap()))
    }

    fn screen_document() -> MacroDocument {
        crate::SessionStore::new().document().clone()
    }

    fn window_document() -> MacroDocument {
        MacroDocument {
            target_window: Some(DslTargetWindow {
                title: Some("Game".into()),
                process: None,
                pause_when_unfocused: true,
                coordinates: CoordinateSpace::Window,
            }),
            ..screen_document()
        }
    }

    fn press_and_release(session: &mut RecordingSession, x: i32, y: i32) {
        let button = MouseButtonRaw::Left;
        session.push(10, RawEventType::MouseDown { x, y, button });
        session.push(60, RawEventType::MouseUp { x, y, button });
    }

    fn click_position(steps: &[DslTimedAction]) -> (i64, i64) {
        steps
            .iter()
            .find_map(|step| match &step.action {
                DslAction::Click {
                    x: tap_core::DslValue::Int(x),
                    y: tap_core::DslValue::Int(y),
                    ..
                } => Some((*x, *y)),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no click among {steps:?}"))
    }

    #[test]
    fn a_screen_recording_keeps_screen_positions() {
        let mut session = session(Arc::new(Mutex::new(None)));
        session.start(&screen_document()).unwrap();

        press_and_release(&mut session, 300, 200);

        assert_eq!(click_position(&session.stop().unwrap()), (300, 200));
    }

    #[test]
    fn a_window_recording_measures_positions_from_the_window() {
        let mut session = session(Arc::new(Mutex::new(Some((100, 50)))));
        session.start(&window_document()).unwrap();

        press_and_release(&mut session, 300, 200);

        assert_eq!(click_position(&session.stop().unwrap()), (200, 150));
    }

    #[test]
    fn a_window_recording_cannot_start_without_the_window() {
        let mut session = session(Arc::new(Mutex::new(None)));

        let error = session.start(&window_document()).unwrap_err();

        assert!(error.contains("target window was not found"));
        assert_eq!(session.state(), RecorderState::Idle);
    }

    #[test]
    fn a_positioned_event_is_dropped_while_no_window_origin_is_known() {
        let origin: SharedOrigin = Arc::new(Mutex::new(Some((100, 50))));
        let mut session = session(origin.clone());
        session.start(&window_document()).unwrap();
        // The window goes away after the start-up check.
        *origin.lock().unwrap() = None;

        assert!(session
            .push(
                10,
                RawEventType::MouseDown {
                    x: 5,
                    y: 5,
                    button: MouseButtonRaw::Left
                }
            )
            .is_none());
        assert_eq!(session.status().event_count, 0);
    }

    #[test]
    fn a_button_event_without_a_position_takes_the_last_known_one() {
        let mut session = session(Arc::new(Mutex::new(None)));
        session.start(&screen_document()).unwrap();
        session.push(0, RawEventType::MouseMove { x: 300, y: 200 });

        press_and_release(&mut session, 0, 0);

        assert_eq!(click_position(&session.stop().unwrap()), (300, 200));
    }

    #[test]
    fn starting_twice_is_refused_and_pausing_needs_a_recording() {
        let mut session = session(Arc::new(Mutex::new(None)));
        assert_eq!(session.pause().unwrap_err(), "Not recording");
        assert_eq!(session.resume().unwrap_err(), "Not paused");

        session.start(&screen_document()).unwrap();
        assert!(session.start(&screen_document()).is_err());
        session.pause().unwrap();
        session.resume().unwrap();
        assert!(session.stop().is_some());
        assert!(session.stop().is_none());
    }
}
