//! One run of a macro: the passes over its timeline, the schedule within a pass, and how a single
//! step is decided (`call_macro`, `conditional`, or a plain action to resolve and perform).

use std::collections::HashMap;
use tap_core::{DslAction, DslTimedAction, DslValue, MacroDocument, VariableStore, VariableValue};

use super::call_macro::{child_scope, prepare_args, CallStack};
use super::dispatch::ActionResult;
use super::player::Player;
use super::ports::{ActionExecutor, PlatformConditionProvider};
use super::protocol::{EngineEvent, EngineState};
use crate::resolve::{resolve_action, resolve_condition};

/// Stop the whole run after this many consecutive injection failures, so a broken injector can
/// never spin a forever-loop indefinitely.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// What belongs to the run in progress and starts over with the next one.
#[derive(Default)]
pub(super) struct RunState {
    /// The scope `{{ }}` templates and conditions read, and counter actions write.
    pub(super) variables: VariableStore,
    call_stack: CallStack,
    consecutive_failures: u32,
}

impl<E: ActionExecutor, P: PlatformConditionProvider> Player<E, P> {
    /// Count down, then play `document` pass after pass until it is done or stopped.
    pub(super) fn play(
        &mut self,
        document: MacroDocument,
        runtime_vars: HashMap<String, VariableValue>,
    ) {
        self.transition_state(EngineState::Arming);
        if !self.arming_countdown(document.run.start_delay_ms) {
            self.transition_state(EngineState::Idle);
            return;
        }

        let mut variables = VariableStore::new();
        variables.init_from_definitions(&document.variables);
        variables.set_variables(runtime_vars);
        self.run = RunState {
            variables,
            ..RunState::default()
        };
        // Sub-macros follow the main macro's variation settings.
        self.humanizer.begin_run(document.run.humanize);

        self.transition_state(EngineState::Running);

        let speed = document.run.speed;
        let repeat = document.run.repeat; // 0 == forever
        let mut iteration = 0u32;
        loop {
            iteration += 1;
            if !self.execute_timeline(&document.timeline, speed, &document) {
                break;
            }
            self.emit(EngineEvent::IterationCompleted { iteration });

            if repeat != 0 && iteration >= repeat {
                self.emit(EngineEvent::Completed);
                break;
            }
        }

        self.transition_state(EngineState::Stopped);
        self.transition_state(EngineState::Idle);
    }

    /// One pass over `timeline`. Returns false when the run should stop (a stop request, or the
    /// failure limit); `exit` only ends this macro.
    fn execute_timeline(
        &mut self,
        timeline: &[DslTimedAction],
        speed: f32,
        doc: &MacroDocument,
    ) -> bool {
        let origin = self.clock.origin();

        for (index, step) in timeline.iter().enumerate() {
            if !self.wait_while_paused() {
                return false;
            }

            if !step.enabled {
                continue;
            }

            if let Some(target) = doc
                .target_window
                .as_ref()
                .filter(|target| target.pause_when_unfocused)
            {
                let focused = |platform: &P| {
                    platform
                        .is_window_focused(target.title.as_deref(), target.process.as_deref())
                        .then_some(())
                };
                if self.wait_for_window(target, focused).is_none() {
                    return false;
                }
            }

            let due_ms = (step.at_ms as f32 / speed) as u64;
            let due_ms = self.humanizer.jitter_start(due_ms);
            let elapsed = self.clock.active_elapsed_ms(origin);
            if due_ms > elapsed {
                self.interruptible_sleep(due_ms - elapsed);
                if self.should_stop() {
                    return false;
                }
            }

            if !self.run_step(&step.action, index, speed, doc) {
                return false;
            }
        }

        true
    }

    /// Run one step. Returns false when this macro should end: a stop request, `exit`, or a
    /// failure that makes going on pointless.
    ///
    /// A step is decided here, on the DSL action, because only here is everything a branch may
    /// need: `call_macro` is expanded by the engine and `conditional` picks a branch, so a
    /// `call_macro` in a branch runs the same way as one on the timeline. Only the branch that is
    /// taken is resolved, so an expression in the other one is never evaluated.
    fn run_step(
        &mut self,
        step: &DslAction,
        index: usize,
        speed: f32,
        doc: &MacroDocument,
    ) -> bool {
        match step {
            DslAction::CallMacro { name, args } => {
                if !self.call_macro(name, args, speed) {
                    return false;
                }
                self.emit(EngineEvent::ActionCompleted { index });
                true
            }
            DslAction::Conditional {
                condition,
                then_action,
                else_action,
            } => {
                let condition = match resolve_condition(condition) {
                    Ok(condition) => condition,
                    Err(e) => return self.resolve_failed(index, e),
                };
                let branch = if self.evaluate_condition(&condition, doc).is_satisfied() {
                    Some(then_action.as_ref())
                } else {
                    else_action.as_deref()
                };
                match branch {
                    Some(branch) => self.run_step(branch, index, speed, doc),
                    None => {
                        self.run.consecutive_failures = 0;
                        self.emit(EngineEvent::ActionCompleted { index });
                        true
                    }
                }
            }
            _ => {
                let action = match resolve_action(step, &self.run.variables, &self.expr_engine) {
                    Ok(action) => action,
                    Err(e) => return self.resolve_failed(index, e),
                };

                match self.execute_action(&action, index, doc) {
                    ActionResult::Completed | ActionResult::Timeout => {
                        self.run.consecutive_failures = 0;
                        self.emit(EngineEvent::ActionCompleted { index });
                        true
                    }
                    // The error was already reported; keep going unless failures pile up.
                    ActionResult::Failed => {
                        self.run.consecutive_failures += 1;
                        let failures = self.run.consecutive_failures;
                        if failures >= MAX_CONSECUTIVE_FAILURES {
                            self.emit(EngineEvent::Error {
                                message: format!(
                                    "Stopping after {failures} consecutive injection failures"
                                ),
                            });
                            self.transition_state(EngineState::Stopped);
                            return false;
                        }
                        self.emit(EngineEvent::ActionCompleted { index });
                        true
                    }
                    ActionResult::Stopped | ActionResult::Exit => false,
                }
            }
        }
    }

    fn resolve_failed(&self, index: usize, error: impl std::fmt::Display) -> bool {
        self.emit(EngineEvent::Error {
            message: format!("Failed to resolve action {index}: {error}"),
        });
        false
    }

    /// Run the macro `name` inline, in a scope of its own. Returns false only when the whole run
    /// should stop: a call that cannot be made (cycle, depth, missing macro, bad argument) is
    /// reported and skipped so the caller carries on.
    fn call_macro(&mut self, name: &str, args: &HashMap<String, DslValue>, speed: f32) -> bool {
        if let Err(e) = self.run.call_stack.push(name) {
            self.emit(EngineEvent::Error {
                message: e.to_string(),
            });
            return true;
        }
        let keep_running = self.run_callee(name, args, speed);
        self.run.call_stack.pop();
        keep_running
    }

    fn run_callee(&mut self, name: &str, args: &HashMap<String, DslValue>, speed: f32) -> bool {
        let callee = match (self.loader)(name) {
            Ok(document) => document,
            Err(e) => {
                self.emit(EngineEvent::Error {
                    message: format!("Failed to load macro '{name}': {e}"),
                });
                return true;
            }
        };

        let caller_scope = self.run.variables.clone();
        let args = match prepare_args(args, &caller_scope, &self.expr_engine) {
            Ok(args) => args,
            Err(e) => {
                self.emit(EngineEvent::Error {
                    message: format!("Failed to prepare args for '{name}': {e}"),
                });
                return true;
            }
        };
        self.run.variables = child_scope(&caller_scope, &callee, args);

        let keep_running = self.execute_timeline(&callee.timeline, speed, &callee);

        self.run.variables = caller_scope;

        // The callee also ends on `exit`; only a real stop request carries over to the caller.
        keep_running || !self.should_stop()
    }
}
