//! Window-relative coordinates: turning a position in the target window into a screen position.
//!
//! A document whose `target_window.coordinates` is `window` measures every position from the
//! top-left corner of its target window (see `CoordinateSpace`). The engine adds the window's
//! current origin at the moment an action is dispatched, so a window that was moved since the last
//! step, or since the macro was recorded, is followed.

use tap_core::{Action, CoordinateSpace, DslTargetWindow, MacroDocument, Point};

use crate::recorder::RawEventType;

/// The target window `doc`'s coordinates are relative to; `None` for a screen-space document.
pub fn window_binding(doc: &MacroDocument) -> Option<&DslTargetWindow> {
    doc.target_window
        .as_ref()
        .filter(|target| target.coordinates == CoordinateSpace::Window)
}

/// Whether `action` carries a screen position of its own.
///
/// Every variant is listed so that a new action with a position cannot be forgotten here.
pub(crate) fn has_position(action: &Action) -> bool {
    match action {
        Action::Click { .. }
        | Action::DoubleClick { .. }
        | Action::MouseDown { .. }
        | Action::MouseUp { .. }
        | Action::MouseMove { .. }
        | Action::Drag { .. } => true,
        Action::Scroll { .. }
        | Action::KeyTap { .. }
        | Action::KeyDown { .. }
        | Action::KeyUp { .. }
        | Action::TextInput { .. }
        | Action::Wait { .. }
        | Action::WaitUntil { .. }
        | Action::SetCounter { .. }
        | Action::IncrCounter { .. }
        | Action::DecrCounter { .. }
        | Action::ResetCounter { .. }
        | Action::Exit => false,
    }
}

fn shift(point: Point, (dx, dy): (i32, i32)) -> Point {
    Point {
        x: point.x.saturating_add(dx),
        y: point.y.saturating_add(dy),
    }
}

/// `action` with its position moved by `origin` (the window's top-left corner on the screen).
/// Actions without a position are returned unchanged.
pub(crate) fn shift_action(action: &Action, origin: (i32, i32)) -> Action {
    let moved = |x: i32, y: i32| {
        let p = shift(Point { x, y }, origin);
        (p.x, p.y)
    };
    match action {
        Action::Click { x, y, button } => {
            let (x, y) = moved(*x, *y);
            Action::Click {
                x,
                y,
                button: *button,
            }
        }
        Action::DoubleClick { x, y, button } => {
            let (x, y) = moved(*x, *y);
            Action::DoubleClick {
                x,
                y,
                button: *button,
            }
        }
        Action::MouseDown { x, y, button } => {
            let (x, y) = moved(*x, *y);
            Action::MouseDown {
                x,
                y,
                button: *button,
            }
        }
        Action::MouseUp { x, y, button } => {
            let (x, y) = moved(*x, *y);
            Action::MouseUp {
                x,
                y,
                button: *button,
            }
        }
        Action::MouseMove { x, y } => {
            let (x, y) = moved(*x, *y);
            Action::MouseMove { x, y }
        }
        Action::Drag {
            from,
            to,
            duration_ms,
        } => Action::Drag {
            from: shift(*from, origin),
            to: shift(*to, origin),
            duration_ms: *duration_ms,
        },
        other => other.clone(),
    }
}

/// A captured input event with its position measured from `origin` (the window's top-left corner
/// on the screen) instead of from the screen: the inverse of `shift_action`, applied while
/// recording. Events without a position are returned unchanged.
pub fn relativize_raw(event: RawEventType, (ox, oy): (i32, i32)) -> RawEventType {
    let rel = |x: i32, y: i32| (x.saturating_sub(ox), y.saturating_sub(oy));
    match event {
        RawEventType::MouseMove { x, y } => {
            let (x, y) = rel(x, y);
            RawEventType::MouseMove { x, y }
        }
        RawEventType::MouseDown { x, y, button } => {
            let (x, y) = rel(x, y);
            RawEventType::MouseDown { x, y, button }
        }
        RawEventType::MouseUp { x, y, button } => {
            let (x, y) = rel(x, y);
            RawEventType::MouseUp { x, y, button }
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recorder::MouseButtonRaw;
    use tap_core::{DslRunConfig, MouseButton};

    const ORIGIN: (i32, i32) = (100, -40);

    #[test]
    fn every_position_of_a_click_like_action_moves_by_the_origin() {
        let b = MouseButton::Left;
        assert_eq!(
            shift_action(
                &Action::Click {
                    x: 5,
                    y: 7,
                    button: b
                },
                ORIGIN
            ),
            Action::Click {
                x: 105,
                y: -33,
                button: b
            }
        );
        assert_eq!(
            shift_action(
                &Action::DoubleClick {
                    x: 5,
                    y: 7,
                    button: b
                },
                ORIGIN
            ),
            Action::DoubleClick {
                x: 105,
                y: -33,
                button: b
            }
        );
        assert_eq!(
            shift_action(
                &Action::MouseDown {
                    x: 5,
                    y: 7,
                    button: b
                },
                ORIGIN
            ),
            Action::MouseDown {
                x: 105,
                y: -33,
                button: b
            }
        );
        assert_eq!(
            shift_action(
                &Action::MouseUp {
                    x: 5,
                    y: 7,
                    button: b
                },
                ORIGIN
            ),
            Action::MouseUp {
                x: 105,
                y: -33,
                button: b
            }
        );
        assert_eq!(
            shift_action(&Action::MouseMove { x: 5, y: 7 }, ORIGIN),
            Action::MouseMove { x: 105, y: -33 }
        );
    }

    #[test]
    fn both_ends_of_a_drag_move_and_its_duration_does_not() {
        let drag = Action::Drag {
            from: Point { x: 1, y: 2 },
            to: Point { x: 30, y: 40 },
            duration_ms: 250,
        };
        assert_eq!(
            shift_action(&drag, ORIGIN),
            Action::Drag {
                from: Point { x: 101, y: -38 },
                to: Point { x: 130, y: 0 },
                duration_ms: 250,
            }
        );
    }

    #[test]
    fn actions_without_a_position_are_left_alone() {
        for action in [
            Action::Scroll {
                delta_x: 3,
                delta_y: -3,
            },
            Action::KeyTap { key: "A".into() },
            Action::TextInput { text: "hi".into() },
            Action::Wait { ms: 10 },
            Action::IncrCounter { key: "n".into() },
            Action::Exit,
        ] {
            assert!(!has_position(&action));
            assert_eq!(shift_action(&action, ORIGIN), action);
        }
    }

    #[test]
    fn recorded_positions_are_measured_from_the_window_and_round_trip() {
        let origin = ORIGIN;
        let down = RawEventType::MouseDown {
            x: 105,
            y: 7,
            button: MouseButtonRaw::Left,
        };
        match relativize_raw(down, origin) {
            RawEventType::MouseDown { x, y, .. } => assert_eq!((x, y), (5, 47)),
            other => panic!("unexpected {other:?}"),
        }
        match relativize_raw(RawEventType::MouseMove { x: 100, y: -40 }, origin) {
            RawEventType::MouseMove { x, y } => assert_eq!((x, y), (0, 0)),
            other => panic!("unexpected {other:?}"),
        }
        // Playback adds the origin back: recording then replaying lands on the same pixel.
        let click = Action::Click {
            x: 105,
            y: 7,
            button: MouseButton::Left,
        };
        let recorded = Action::Click {
            x: 5,
            y: 47,
            button: MouseButton::Left,
        };
        assert_eq!(shift_action(&recorded, origin), click);
    }

    #[test]
    fn events_without_a_position_are_not_touched_when_recording() {
        match relativize_raw(RawEventType::KeyDown { key: "A".into() }, ORIGIN) {
            RawEventType::KeyDown { key } => assert_eq!(key, "A"),
            other => panic!("unexpected {other:?}"),
        }
        match relativize_raw(
            RawEventType::Scroll {
                delta_x: 1,
                delta_y: -2,
            },
            ORIGIN,
        ) {
            RawEventType::Scroll { delta_x, delta_y } => assert_eq!((delta_x, delta_y), (1, -2)),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn a_position_that_would_overflow_saturates_instead_of_wrapping() {
        let shifted = shift_action(
            &Action::MouseMove {
                x: i32::MAX,
                y: i32::MIN,
            },
            (10, -10),
        );
        assert_eq!(
            shifted,
            Action::MouseMove {
                x: i32::MAX,
                y: i32::MIN
            }
        );
    }

    #[test]
    fn a_document_is_window_relative_only_with_a_window_and_the_flag() {
        let mut doc = MacroDocument {
            name: "m".into(),
            description: None,
            version: "1.0".into(),
            author: None,
            tags: Vec::new(),
            variables: Default::default(),
            target_window: None,
            timeline: Vec::new(),
            run: DslRunConfig::default(),
        };
        assert!(window_binding(&doc).is_none());

        let mut target = DslTargetWindow {
            title: Some("Game".into()),
            process: None,
            pause_when_unfocused: true,
            coordinates: CoordinateSpace::Screen,
        };
        doc.target_window = Some(target.clone());
        assert!(
            window_binding(&doc).is_none(),
            "a target alone does not change the space"
        );

        target.coordinates = CoordinateSpace::Window;
        doc.target_window = Some(target);
        assert_eq!(
            window_binding(&doc).and_then(|t| t.title.as_deref()),
            Some("Game")
        );
    }
}
