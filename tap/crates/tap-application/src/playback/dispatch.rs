//! Performing one concrete action: the engine's own actions (counters, waits, `exit`) are handled
//! here, everything else goes to the executor.

use std::time::Instant;

use tap_core::{
    Action, Condition, ConditionColor, ConditionEvaluator, ConditionResult, DslTargetWindow,
    MacroDocument, MouseButton, Point, VariableStore,
};
use tracing::error;

use super::player::Player;
use super::ports::{ActionExecutor, PlatformConditionProvider};
use super::protocol::EngineEvent;
use crate::coords::{has_position, shift_action, window_binding};

/// Target interval between interpolated drag steps (~60 fps).
const DRAG_STEP_MS: u64 = 16;
/// Upper bound on interpolated drag steps, so very long drags stay cheap while remaining
/// interruptible within one step.
const MAX_DRAG_STEPS: u32 = 240;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ActionResult {
    Completed,
    Stopped,
    /// A `wait_until` ran out of time.
    Timeout,
    Exit,
    /// The executor reported a failure for this action.
    Failed,
}

impl<E: ActionExecutor, P: PlatformConditionProvider> Player<E, P> {
    /// `doc` is the document the action belongs to: when its coordinates are window-relative, a
    /// positioned action is moved to its screen position here, the one place every injected
    /// action passes through. `ActionStarting` reports the screen position, i.e. what is
    /// injected.
    pub(super) fn execute_action(
        &mut self,
        action: &Action,
        index: usize,
        doc: &MacroDocument,
    ) -> ActionResult {
        // Humanize first, in the document's own coordinates: the window shift then moves the
        // already varied position, so both settings combine whatever the order.
        let humanized;
        let action = if has_position(action) {
            humanized = self.humanizer.jitter_action(action);
            &humanized
        } else {
            action
        };

        let on_screen;
        let action = match window_binding(doc).filter(|_| has_position(action)) {
            Some(target) => {
                let wanted = |platform: &P| {
                    platform.window_origin(target.title.as_deref(), target.process.as_deref())
                };
                let Some(origin) = self.wait_for_window(target, wanted) else {
                    return ActionResult::Stopped;
                };
                on_screen = shift_action(action, origin);
                &on_screen
            }
            None => action,
        };

        match action {
            Action::SetCounter { key, value } => {
                self.run.variables.set_counter(key, *value);
                self.counter_changed(key, *value)
            }
            Action::IncrCounter { key } => {
                let value = self.run.variables.incr_counter(key);
                self.counter_changed(key, value)
            }
            Action::DecrCounter { key } => {
                let value = self.run.variables.decr_counter(key);
                self.counter_changed(key, value)
            }
            Action::ResetCounter { key } => {
                self.run.variables.reset_counter(key);
                self.counter_changed(key, 0)
            }
            Action::Exit => ActionResult::Exit,
            Action::WaitUntil {
                condition,
                timeout_ms,
                poll_interval_ms,
            } => self.wait_until(condition, *timeout_ms, *poll_interval_ms, doc),
            Action::Wait { ms } => {
                self.interruptible_sleep(*ms);
                ActionResult::Completed
            }
            // Interpolated by the engine so it stays interruptible and always releases the button.
            Action::Drag {
                from,
                to,
                duration_ms,
            } => {
                self.emit(EngineEvent::ActionStarting {
                    index,
                    action: action.clone(),
                });
                self.drag(*from, *to, *duration_ms)
            }
            _ => {
                self.emit(EngineEvent::ActionStarting {
                    index,
                    action: action.clone(),
                });
                match self.executor.execute(action) {
                    Ok(()) => ActionResult::Completed,
                    Err(e) => {
                        error!(index, error = %e, "action execution failed");
                        self.emit(EngineEvent::Error {
                            message: format!("Action {index} failed: {e}"),
                        });
                        ActionResult::Failed
                    }
                }
            }
        }
    }

    fn counter_changed(&self, key: &str, value: i32) -> ActionResult {
        self.emit(EngineEvent::CounterChanged {
            key: key.to_string(),
            value,
        });
        ActionResult::Completed
    }

    fn wait_until(
        &mut self,
        condition: &Condition,
        timeout_ms: Option<u64>,
        poll_interval_ms: u64,
        doc: &MacroDocument,
    ) -> ActionResult {
        let description = format!("{condition:?}");
        self.emit(EngineEvent::WaitingForCondition {
            condition: description.clone(),
        });

        let start = Instant::now();
        loop {
            if self.should_stop() {
                return ActionResult::Stopped;
            }
            if self.evaluate_condition(condition, doc).is_satisfied() {
                self.emit(EngineEvent::ConditionSatisfied {
                    condition: description,
                });
                return ActionResult::Completed;
            }
            if timeout_ms.is_some_and(|timeout| start.elapsed().as_millis() as u64 >= timeout) {
                self.emit(EngineEvent::ConditionTimeout {
                    condition: description,
                });
                return ActionResult::Timeout;
            }
            self.interruptible_sleep(poll_interval_ms);
        }
    }

    /// Press at `from`, step the cursor to `to` over `duration_ms`, release. On a stop or an
    /// injection failure the button is released first, so a half-finished drag can never leave
    /// the mouse pressed.
    fn drag(&mut self, from: Point, to: Point, duration_ms: u64) -> ActionResult {
        if let Err(e) = self.executor.execute(&Action::MouseDown {
            x: from.x,
            y: from.y,
            button: MouseButton::Left,
        }) {
            self.emit(EngineEvent::Error {
                message: format!("Drag press failed: {e}"),
            });
            return ActionResult::Failed;
        }

        let steps = drag_steps(duration_ms);
        let step_ms = duration_ms / u64::from(steps);

        for k in 1..=steps {
            if step_ms > 0 {
                self.interruptible_sleep(step_ms);
            }
            let (x, y) = lerp_point(from, to, k, steps);

            if self.should_stop() {
                self.release(x, y);
                return ActionResult::Stopped;
            }

            if let Err(e) = self.executor.execute(&Action::MouseMove { x, y }) {
                self.emit(EngineEvent::Error {
                    message: format!("Drag move failed: {e}"),
                });
                self.release(x, y);
                return ActionResult::Failed;
            }
        }

        let released = self.executor.execute(&Action::MouseUp {
            x: to.x,
            y: to.y,
            button: MouseButton::Left,
        });
        match released {
            Ok(()) => ActionResult::Completed,
            Err(e) => {
                self.emit(EngineEvent::Error {
                    message: format!("Drag release failed: {e}"),
                });
                ActionResult::Failed
            }
        }
    }

    /// Best-effort release when a drag is abandoned; there is nobody left to tell if it fails.
    fn release(&self, x: i32, y: i32) {
        let _ = self.executor.execute(&Action::MouseUp {
            x,
            y,
            button: MouseButton::Left,
        });
    }

    /// In a window-relative document, `pixel_color` positions are measured from the target window.
    pub(super) fn evaluate_condition(
        &self,
        condition: &Condition,
        doc: &MacroDocument,
    ) -> ConditionResult {
        RuntimeConditionEvaluator {
            platform: &self.platform,
            variables: &self.run.variables,
            window: window_binding(doc),
        }
        .evaluate(condition)
    }
}

/// Number of interpolation steps for a drag of the given duration.
fn drag_steps(duration_ms: u64) -> u32 {
    // Clamp before narrowing: a huge duration must not wrap around into a small step count.
    let by_time = (duration_ms / DRAG_STEP_MS).min(u64::from(MAX_DRAG_STEPS)) as u32;
    by_time.max(1)
}

/// The cursor position at step `k` of `steps` on the straight line from `from` to `to`.
fn lerp_point(from: Point, to: Point, k: u32, steps: u32) -> (i32, i32) {
    let t = k as f64 / steps as f64;
    let x = from.x as f64 + (to.x - from.x) as f64 * t;
    let y = from.y as f64 + (to.y - from.y) as f64 * t;
    (x.round() as i32, y.round() as i32)
}

struct RuntimeConditionEvaluator<'a, P> {
    platform: &'a P,
    variables: &'a VariableStore,
    window: Option<&'a DslTargetWindow>,
}

impl<P: PlatformConditionProvider> ConditionEvaluator for RuntimeConditionEvaluator<'_, P> {
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool {
        self.platform.is_window_focused(title, process)
    }

    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool {
        self.platform.window_exists(title, process)
    }

    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor> {
        let (dx, dy) = match self.window {
            // No window to measure from: the pixel cannot be read, so the condition errors out
            // (not satisfied) rather than reading some other place on the screen.
            Some(target) => self
                .platform
                .window_origin(target.title.as_deref(), target.process.as_deref())?,
            None => (0, 0),
        };
        self.platform
            .get_pixel_color(x.saturating_add(dx), y.saturating_add(dy))
    }

    fn get_counter(&self, key: &str) -> i32 {
        self.variables.get_counter(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_has_at_least_one_step_and_at_most_the_cap() {
        assert_eq!(drag_steps(0), 1);
        assert_eq!(drag_steps(DRAG_STEP_MS - 1), 1);
        assert_eq!(drag_steps(DRAG_STEP_MS * 10), 10);
        assert_eq!(drag_steps(u64::MAX), MAX_DRAG_STEPS);
    }

    #[test]
    fn a_huge_drag_duration_does_not_wrap_into_a_tiny_step_count() {
        // Just past what a u32 step count can hold: casting before clamping wraps to 0 -> 1.
        let duration_ms = (u64::from(u32::MAX) + 1) * DRAG_STEP_MS;
        assert_eq!(drag_steps(duration_ms), MAX_DRAG_STEPS);
    }
}
