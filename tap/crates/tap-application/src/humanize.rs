//! Humanization: small random variation in where and when a macro acts.
//!
//! `run.humanize` (see [`Humanize`]) asks for positions to land a few pixels off and for step
//! start times to move by a few milliseconds, so repeated runs do not hit exactly the same pixel
//! at exactly the same instant. The variation is applied on the way to the injector; the document
//! and its timeline are never changed.
//!
//! The random numbers come from a tiny seedable generator rather than a dependency: the quality
//! needed here is "not obviously periodic", and a fixed seed makes runs reproducible in tests.

use std::time::{SystemTime, UNIX_EPOCH};

use tap_core::{
    Action, Humanize, MouseButton, Point, MAX_HUMANIZE_DELAY_MS, MAX_HUMANIZE_POSITION_PX,
};

/// SplitMix64: a 64-bit generator with a one-word state, good statistical quality for its size.
#[derive(Debug, Clone)]
pub(crate) struct JitterRng {
    state: u64,
}

impl JitterRng {
    pub(crate) fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    /// A seed that differs between runs of the program.
    pub(crate) fn entropy_seed() -> u64 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15);
        nanos ^ (u64::from(std::process::id()) << 32)
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniformly distributed integer in `-bound..=bound`. Consumes nothing for a zero bound.
    fn offset(&mut self, bound: u32) -> i32 {
        if bound == 0 {
            return 0;
        }
        let span = 2 * u64::from(bound) + 1;
        ((self.next_u64() % span) as i64 - i64::from(bound)) as i32
    }
}

/// Applies the run's `humanize` settings. One lives in the player for its whole life; every run
/// starts it afresh with [`Humanizer::begin_run`].
#[derive(Debug)]
pub(crate) struct Humanizer {
    config: Humanize,
    rng: JitterRng,
    /// The offset each currently pressed mouse button was pressed with: its release reuses it.
    pressed: Vec<(MouseButton, (i32, i32))>,
}

impl Humanizer {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            config: Humanize::default(),
            rng: JitterRng::new(seed),
            pressed: Vec::new(),
        }
    }

    /// Start a run with `config`. A validated document is within the limits already; clamping
    /// here keeps an unvalidated one from asking for absurd offsets.
    pub(crate) fn begin_run(&mut self, config: Humanize) {
        self.config = Humanize {
            position_px: config.position_px.min(MAX_HUMANIZE_POSITION_PX),
            delay_ms: config.delay_ms.min(MAX_HUMANIZE_DELAY_MS),
        };
        self.pressed.clear();
    }

    fn point_offset(&mut self) -> (i32, i32) {
        let px = self.config.position_px;
        (self.rng.offset(px), self.rng.offset(px))
    }

    /// `action` with its position moved by a random offset.
    ///
    /// - `Click`, `DoubleClick`, `MouseMove`: one offset.
    /// - `Drag`: each end point gets its own, since the engine presses and releases it itself.
    /// - `MouseDown` draws an offset and `MouseUp` of the same button reuses it, so a recorded
    ///   click stays a click and a recorded drag stays one long gesture. A release with no press
    ///   before it draws its own.
    ///
    /// Actions without a position are returned unchanged.
    pub(crate) fn jitter_action(&mut self, action: &Action) -> Action {
        if self.config.position_px == 0 {
            return action.clone();
        }
        match action {
            Action::Click { x, y, button } => {
                let (x, y) = self.moved(*x, *y);
                Action::Click {
                    x,
                    y,
                    button: *button,
                }
            }
            Action::DoubleClick { x, y, button } => {
                let (x, y) = self.moved(*x, *y);
                Action::DoubleClick {
                    x,
                    y,
                    button: *button,
                }
            }
            Action::MouseMove { x, y } => {
                let (x, y) = self.moved(*x, *y);
                Action::MouseMove { x, y }
            }
            Action::MouseDown { x, y, button } => {
                let offset = self.point_offset();
                self.pressed.retain(|(pressed, _)| pressed != button);
                self.pressed.push((*button, offset));
                Action::MouseDown {
                    x: x.saturating_add(offset.0),
                    y: y.saturating_add(offset.1),
                    button: *button,
                }
            }
            Action::MouseUp { x, y, button } => {
                let offset = match self
                    .pressed
                    .iter()
                    .position(|(pressed, _)| pressed == button)
                {
                    Some(at) => self.pressed.remove(at).1,
                    None => self.point_offset(),
                };
                Action::MouseUp {
                    x: x.saturating_add(offset.0),
                    y: y.saturating_add(offset.1),
                    button: *button,
                }
            }
            Action::Drag {
                from,
                to,
                duration_ms,
            } => {
                let from_offset = self.point_offset();
                let to_offset = self.point_offset();
                Action::Drag {
                    from: Point {
                        x: from.x.saturating_add(from_offset.0),
                        y: from.y.saturating_add(from_offset.1),
                    },
                    to: Point {
                        x: to.x.saturating_add(to_offset.0),
                        y: to.y.saturating_add(to_offset.1),
                    },
                    duration_ms: *duration_ms,
                }
            }
            other => other.clone(),
        }
    }

    fn moved(&mut self, x: i32, y: i32) -> (i32, i32) {
        let (dx, dy) = self.point_offset();
        (x.saturating_add(dx), y.saturating_add(dy))
    }

    /// When a step scheduled for `scheduled_ms` (on the run's active clock) should start: moved
    /// earlier or later by up to `delay_ms`, never before the run began.
    ///
    /// The variation is applied to each step's own schedule rather than added up from step to
    /// step, so a run does not drift. Steps still execute in order: a step whose moved start has
    /// already passed simply starts at once.
    pub(crate) fn jitter_start(&mut self, scheduled_ms: u64) -> u64 {
        let offset = self.rng.offset(self.config.delay_ms);
        scheduled_ms.saturating_add_signed(i64::from(offset))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn humanizer(position_px: u32, delay_ms: u32) -> Humanizer {
        let mut humanizer = Humanizer::new(42);
        humanizer.begin_run(Humanize {
            position_px,
            delay_ms,
        });
        humanizer
    }

    fn click(x: i32, y: i32) -> Action {
        Action::Click {
            x,
            y,
            button: MouseButton::Left,
        }
    }

    fn position_of(action: &Action) -> (i32, i32) {
        match action {
            Action::Click { x, y, .. }
            | Action::MouseDown { x, y, .. }
            | Action::MouseUp { x, y, .. }
            | Action::MouseMove { x, y } => (*x, *y),
            other => panic!("no single position: {other:?}"),
        }
    }

    #[test]
    fn the_same_seed_gives_the_same_variation_and_another_seed_a_different_one() {
        let run = |seed: u64| {
            let mut h = Humanizer::new(seed);
            h.begin_run(Humanize {
                position_px: 10,
                delay_ms: 0,
            });
            (0..20)
                .map(|_| position_of(&h.jitter_action(&click(100, 100))))
                .collect::<Vec<_>>()
        };
        assert_eq!(run(7), run(7));
        assert_ne!(run(7), run(8));
    }

    #[test]
    fn offsets_stay_within_the_bound_and_use_all_of_it() {
        let mut h = humanizer(3, 0);
        let mut seen = std::collections::HashSet::new();
        for _ in 0..2000 {
            let (x, y) = position_of(&h.jitter_action(&click(100, 100)));
            assert!((97..=103).contains(&x) && (97..=103).contains(&y));
            seen.insert((x - 100, y - 100));
        }
        // 7 x 7 possible offsets; the bound itself is reachable on both sides.
        assert_eq!(seen.len(), 49);
    }

    #[test]
    fn nothing_changes_when_humanize_is_off() {
        let mut h = humanizer(0, 0);
        assert_eq!(h.jitter_action(&click(5, 6)), click(5, 6));
        assert_eq!(h.jitter_start(1234), 1234);
    }

    #[test]
    fn only_actions_with_a_position_are_moved() {
        let mut h = humanizer(50, 0);
        for action in [
            Action::Scroll {
                delta_x: 1,
                delta_y: 2,
            },
            Action::KeyTap { key: "A".into() },
            Action::TextInput { text: "x".into() },
            Action::Wait { ms: 10 },
            Action::Exit,
        ] {
            assert_eq!(h.jitter_action(&action), action);
        }
    }

    #[test]
    fn a_release_lands_where_its_press_did() {
        let mut h = humanizer(20, 0);
        for _ in 0..200 {
            let down = h.jitter_action(&Action::MouseDown {
                x: 300,
                y: 400,
                button: MouseButton::Left,
            });
            let up = h.jitter_action(&Action::MouseUp {
                x: 300,
                y: 400,
                button: MouseButton::Left,
            });
            assert_eq!(
                position_of(&down),
                position_of(&up),
                "a click must not become a drag"
            );
        }
    }

    #[test]
    fn a_drag_gesture_keeps_one_offset_between_press_and_release() {
        let mut h = humanizer(20, 0);
        let down = h.jitter_action(&Action::MouseDown {
            x: 100,
            y: 100,
            button: MouseButton::Left,
        });
        // Moves in between are jittered on their own.
        h.jitter_action(&Action::MouseMove { x: 150, y: 150 });
        let up = h.jitter_action(&Action::MouseUp {
            x: 200,
            y: 200,
            button: MouseButton::Left,
        });
        let (dx, dy) = (position_of(&down).0 - 100, position_of(&down).1 - 100);
        assert_eq!(position_of(&up), (200 + dx, 200 + dy));
    }

    #[test]
    fn each_button_remembers_its_own_press() {
        let mut h = humanizer(40, 0);
        let left = h.jitter_action(&Action::MouseDown {
            x: 0,
            y: 0,
            button: MouseButton::Left,
        });
        let right = h.jitter_action(&Action::MouseDown {
            x: 0,
            y: 0,
            button: MouseButton::Right,
        });
        let right_up = h.jitter_action(&Action::MouseUp {
            x: 0,
            y: 0,
            button: MouseButton::Right,
        });
        let left_up = h.jitter_action(&Action::MouseUp {
            x: 0,
            y: 0,
            button: MouseButton::Left,
        });
        assert_eq!(position_of(&left), position_of(&left_up));
        assert_eq!(position_of(&right), position_of(&right_up));
    }

    #[test]
    fn a_new_run_forgets_a_press_that_was_never_released() {
        let mut h = humanizer(40, 0);
        h.jitter_action(&Action::MouseDown {
            x: 0,
            y: 0,
            button: MouseButton::Left,
        });
        assert_eq!(h.pressed.len(), 1);
        h.begin_run(Humanize {
            position_px: 40,
            delay_ms: 0,
        });
        assert!(h.pressed.is_empty());
    }

    #[test]
    fn the_ends_of_a_drag_vary_independently() {
        let mut h = humanizer(30, 0);
        let mut differing = 0;
        for _ in 0..50 {
            let Action::Drag { from, to, .. } = h.jitter_action(&Action::Drag {
                from: Point { x: 0, y: 0 },
                to: Point { x: 0, y: 0 },
                duration_ms: 100,
            }) else {
                panic!("a drag stays a drag");
            };
            assert!([from.x, from.y, to.x, to.y].iter().all(|v| v.abs() <= 30));
            if (from.x, from.y) != (to.x, to.y) {
                differing += 1;
            }
        }
        assert!(differing > 40, "the two ends should rarely share an offset");
    }

    #[test]
    fn positions_at_the_edge_of_the_range_do_not_wrap() {
        let mut h = humanizer(50, 0);
        for _ in 0..100 {
            let (x, y) = position_of(&h.jitter_action(&click(i32::MAX, i32::MIN)));
            assert!(x >= i32::MAX - 50);
            assert!(y <= i32::MIN + 50);
        }
    }

    #[test]
    fn a_step_start_moves_both_ways_but_never_before_the_run_began() {
        let mut h = humanizer(0, 40);
        let moved: Vec<u64> = (0..2000).map(|_| h.jitter_start(100)).collect();
        assert!(moved.iter().all(|t| (60..=140).contains(t)));
        assert!(moved.iter().any(|t| *t < 100) && moved.iter().any(|t| *t > 100));

        // A step at the very start can only be delayed, not started before time zero.
        let at_zero: Vec<u64> = (0..2000).map(|_| h.jitter_start(0)).collect();
        assert!(at_zero.iter().all(|t| *t <= 40));
        assert!(at_zero.iter().any(|t| *t > 0));
    }

    #[test]
    fn a_document_asking_for_too_much_is_clamped() {
        let mut h = Humanizer::new(1);
        h.begin_run(Humanize {
            position_px: u32::MAX,
            delay_ms: u32::MAX,
        });
        for _ in 0..200 {
            let (x, _) = position_of(&h.jitter_action(&click(0, 0)));
            assert!(x.unsigned_abs() <= MAX_HUMANIZE_POSITION_PX);
            assert!(h.jitter_start(1_000_000) <= 1_000_000 + u64::from(MAX_HUMANIZE_DELAY_MS));
        }
    }
}
