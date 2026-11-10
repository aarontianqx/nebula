//! Execution engine: state machine + player thread.

use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tap_core::{
    create_expression_engine, Action, Condition, ConditionColor, ConditionEvaluator,
    ConditionResult, DslAction, DslTargetWindow, DslTimedAction, DslValue, ExpressionEngineHandle,
    MacroDocument, MouseButton, Point, VariableStore, VariableValue,
};
use tracing::{debug, error, info, warn};

/// Stop the whole run after this many consecutive injection failures, so a
/// broken injector can never spin a forever-loop indefinitely.
const MAX_CONSECUTIVE_FAILURES: u32 = 5;

/// Target interval between interpolated drag steps (~60 fps).
const DRAG_STEP_MS: u64 = 16;
/// Upper bound on interpolated drag steps, so very long drags stay cheap while
/// remaining interruptible within one step.
const MAX_DRAG_STEPS: u32 = 240;

use crate::clock::RunClock;
use crate::coords::{has_position, shift_action, window_binding};
use crate::humanize::{Humanizer, JitterRng};
use crate::resolve::{resolve_action, resolve_condition};
use crate::storage::load_document;
use crate::submacro::{
    create_child_variable_store, create_submacro_context, prepare_submacro_args,
    SubMacroContextHandle,
};

/// Engine state machine.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    /// Idle, waiting for start command.
    #[default]
    Idle,
    /// Arming (countdown before execution).
    Arming,
    /// Running, executing actions.
    Running,
    /// Paused, can resume.
    Paused,
    /// Stopped (terminal state for current run).
    Stopped,
}

/// Commands sent to the player thread.
#[derive(Debug, Clone)]
pub enum EngineCommand {
    /// Start execution with countdown.
    Start,
    /// Pause execution.
    Pause,
    /// Resume from pause.
    Resume,
    /// Stop execution immediately.
    Stop,
    /// Emergency stop (highest priority).
    EmergencyStop,
    /// Replace the document to execute.
    SetDocument(Box<MacroDocument>),
    /// Replace the run-time variable overrides applied on the next `Start`.
    SetRuntimeVars(HashMap<String, VariableValue>),
}

/// Events emitted by the player.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EngineEvent {
    /// State changed.
    StateChanged { old: EngineState, new: EngineState },
    /// Countdown tick (seconds remaining).
    CountdownTick { remaining_secs: u32 },
    /// About to execute an action.
    ActionStarting { index: usize, action: Action },
    /// Action completed.
    ActionCompleted { index: usize },
    /// Iteration completed.
    IterationCompleted { iteration: u32 },
    /// Execution completed (all iterations done).
    Completed,
    /// Error occurred.
    Error { message: String },
    /// Waiting for condition to be satisfied.
    WaitingForCondition { condition: String },
    /// Condition was satisfied.
    ConditionSatisfied { condition: String },
    /// Condition timed out.
    ConditionTimeout { condition: String },
    /// Counter value changed.
    CounterChanged { key: String, value: i32 },
    /// Target window not focused (pausing).
    TargetWindowUnfocused {
        title: Option<String>,
        process: Option<String>,
    },
    /// Target window focused again (resuming).
    TargetWindowFocused,
}

/// Handle to control the player thread.
pub struct PlayerHandle {
    cmd_tx: Sender<EngineCommand>,
    event_rx: Receiver<EngineEvent>,
    state: Arc<Mutex<EngineState>>,
}

impl PlayerHandle {
    /// Send a command to the player.
    pub fn send(&self, cmd: EngineCommand) {
        if let Err(e) = self.cmd_tx.send(cmd) {
            warn!("Failed to send command to player: {}", e);
        }
    }

    /// Try to receive an event (non-blocking).
    pub fn try_recv(&self) -> Option<EngineEvent> {
        self.event_rx.try_recv().ok()
    }

    /// Get current state.
    pub fn state(&self) -> EngineState {
        *self.state.lock().unwrap()
    }
}

/// Port for performing actions against the OS.
///
/// The application layer depends only on this trait; the concrete injector is
/// supplied by an outer layer (the `src-tauri` adapter wires up the
/// `tap-platform` injector).
pub trait ActionExecutor: Send + Sync {
    fn execute(&self, action: &Action) -> Result<(), String>;
}

/// Trait for platform-level condition evaluation (implemented by tap-platform adapter).
pub trait PlatformConditionProvider: Send + Sync {
    /// Check if a window is focused.
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool;
    /// Check if a window exists.
    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool;
    /// Get the pixel color at the given coordinates.
    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor>;
    /// The screen position of the top-left corner of the window matching `title` / `process`:
    /// the origin of window-relative coordinates. `None` when there is no such window (or it is
    /// minimized).
    fn window_origin(&self, title: Option<&str>, process: Option<&str>) -> Option<(i32, i32)>;
}

/// Loads a sub-macro document by name. Defaults to disk storage; injectable in
/// tests so `call_macro` can be exercised without touching the filesystem.
pub type SubMacroLoader = Arc<dyn Fn(&str) -> Result<MacroDocument, String> + Send + Sync>;

/// Player: runs in a separate thread, executes the document timeline.
pub struct Player<E: ActionExecutor, P: PlatformConditionProvider> {
    executor: Arc<E>,
    platform: Arc<P>,
    document: Arc<Mutex<Option<MacroDocument>>>,
    runtime_vars: Arc<Mutex<HashMap<String, VariableValue>>>,
    state: Arc<Mutex<EngineState>>,
    variables: Arc<Mutex<VariableStore>>,
    expr_engine: ExpressionEngineHandle,
    loader: SubMacroLoader,
    /// Active-time clock for scheduling: pauses and a lost target window are
    /// excluded so resuming never fires the remaining steps as a burst.
    clock: RunClock,
    /// Consecutive injection failures in the current run (reset on success and
    /// at the start of each run); drives the auto-stop safety net.
    consecutive_failures: AtomicU32,
    /// Random position / timing variation (`run.humanize`), restarted with every run.
    humanizer: Mutex<Humanizer>,
    cmd_rx: Receiver<EngineCommand>,
    event_tx: Sender<EngineEvent>,
}

impl<E: ActionExecutor + 'static, P: PlatformConditionProvider + 'static> Player<E, P> {
    /// Create a new player (sub-macros loaded from disk) and return a handle.
    pub fn spawn(executor: E, platform: P) -> PlayerHandle {
        let loader: SubMacroLoader =
            Arc::new(|name: &str| load_document(name).map_err(|e| e.to_string()));
        Self::spawn_with_loader(executor, platform, loader)
    }

    /// Create a new player (sub-macros loaded from disk) whose `run.humanize` variation is drawn
    /// from `seed`, so a run can be reproduced exactly (used by tests).
    pub fn spawn_with_seed(executor: E, platform: P, seed: u64) -> PlayerHandle {
        let loader: SubMacroLoader =
            Arc::new(|name: &str| load_document(name).map_err(|e| e.to_string()));
        Self::spawn_inner(executor, platform, loader, seed)
    }

    /// Create a new player with a custom sub-macro loader (used by tests).
    pub fn spawn_with_loader(executor: E, platform: P, loader: SubMacroLoader) -> PlayerHandle {
        Self::spawn_inner(executor, platform, loader, JitterRng::entropy_seed())
    }

    fn spawn_inner(executor: E, platform: P, loader: SubMacroLoader, seed: u64) -> PlayerHandle {
        let (cmd_tx, cmd_rx) = bounded(32);
        let (event_tx, event_rx) = bounded(256);
        let state = Arc::new(Mutex::new(EngineState::Idle));

        let player = Player {
            executor: Arc::new(executor),
            platform: Arc::new(platform),
            document: Arc::new(Mutex::new(None)),
            runtime_vars: Arc::new(Mutex::new(HashMap::new())),
            state: state.clone(),
            variables: Arc::new(Mutex::new(VariableStore::new())),
            expr_engine: create_expression_engine(),
            loader,
            clock: RunClock::new(),
            consecutive_failures: AtomicU32::new(0),
            humanizer: Mutex::new(Humanizer::new(seed)),
            cmd_rx,
            event_tx,
        };

        thread::spawn(move || player.run_loop());

        PlayerHandle {
            cmd_tx,
            event_rx,
            state,
        }
    }

    fn run_loop(self) {
        info!("Player thread started");

        // Wait for commands until the channel closes or a command requests exit.
        while let Ok(cmd) = self.cmd_rx.recv() {
            if !self.handle_command(cmd) {
                break;
            }
        }

        info!("Player thread exiting");
    }

    /// Handle a command. Returns false if should exit.
    fn handle_command(&self, cmd: EngineCommand) -> bool {
        debug!(?cmd, "handling command");

        match cmd {
            EngineCommand::Start => {
                self.start_execution();
            }
            EngineCommand::Pause => {
                self.transition_state(EngineState::Paused);
            }
            EngineCommand::Resume => {
                let current = self.get_state();
                if current == EngineState::Paused {
                    self.transition_state(EngineState::Running);
                }
            }
            EngineCommand::Stop | EngineCommand::EmergencyStop => {
                self.transition_state(EngineState::Stopped);
                // Don't exit thread, just reset to idle after processing
                self.transition_state(EngineState::Idle);
            }
            EngineCommand::SetDocument(document) => {
                *self.document.lock().unwrap() = Some(*document);
            }
            EngineCommand::SetRuntimeVars(vars) => {
                *self.runtime_vars.lock().unwrap() = vars;
            }
        }

        true
    }

    fn start_execution(&self) {
        let document = {
            let guard = self.document.lock().unwrap();
            match guard.clone() {
                Some(d) => d,
                None => {
                    self.emit(EngineEvent::Error {
                        message: "No document set".into(),
                    });
                    return;
                }
            }
        };

        // Arming (countdown). Honors sub-second delays and is interruptible.
        self.transition_state(EngineState::Arming);
        if !self.arming_countdown(document.run.start_delay_ms) {
            // Stopped during arming: should_stop already moved us to Stopped,
            // so settle back to Idle like a normal stop would.
            self.transition_state(EngineState::Idle);
            return;
        }

        // Build the run scope: variable definition defaults, then run-time
        // overrides on top (overrides win, per the resolution precedence rules).
        {
            let mut vars = self.variables.lock().unwrap();
            *vars = VariableStore::new();
            vars.init_from_definitions(&document.variables);
            let runtime = self.runtime_vars.lock().unwrap().clone();
            vars.set_variables(runtime);
        }

        // Fresh failure budget for this run.
        self.consecutive_failures.store(0, Ordering::Relaxed);

        // The variation settings of the document being run (sub-macros follow the main macro).
        self.humanizer
            .lock()
            .unwrap()
            .begin_run(document.run.humanize);

        // Start running
        self.transition_state(EngineState::Running);

        let speed = document.run.speed;
        let repeat = document.run.repeat; // 0 == forever
        let sub_ctx = create_submacro_context();
        let mut iteration = 0u32;

        loop {
            iteration += 1;

            // Execute one iteration of the timeline
            if !self.execute_timeline(&document.timeline, speed, &document, &sub_ctx) {
                // Stopped during execution
                break;
            }

            self.emit(EngineEvent::IterationCompleted { iteration });

            // Check repeat condition (0 == forever).
            if repeat != 0 && iteration >= repeat {
                self.emit(EngineEvent::Completed);
                break;
            }
        }

        self.transition_state(EngineState::Stopped);
        self.transition_state(EngineState::Idle);
    }

    /// Execute one pass over the document timeline. Returns false when the run
    /// should stop (global Stop/EmergencyStop); `Exit` only ends this macro.
    fn execute_timeline(
        &self,
        timeline: &[DslTimedAction],
        speed: f32,
        doc: &MacroDocument,
        sub_ctx: &SubMacroContextHandle,
    ) -> bool {
        // Scheduling runs on active time (see `crate::clock`): time spent paused
        // or waiting for the target window does not count towards `at_ms`.
        let origin = self.clock.origin();

        for (index, timed_action) in timeline.iter().enumerate() {
            // Check for stop/pause
            loop {
                if self.should_stop() {
                    return false;
                }
                if self.get_state() == EngineState::Paused {
                    thread::sleep(Duration::from_millis(50));
                    continue;
                }
                break;
            }

            if !timed_action.enabled {
                continue;
            }

            // Check target window if configured
            if !self.wait_for_target_window(doc) {
                return false;
            }

            // Wait until the scheduled time
            let target_ms = (timed_action.at_ms as f32 / speed) as u64;
            let target_ms = self.humanizer.lock().unwrap().jitter_start(target_ms);
            let elapsed = self.clock.active_elapsed_ms(origin);
            if target_ms > elapsed {
                let wait_ms = target_ms - elapsed;
                self.interruptible_sleep(wait_ms);
                if self.should_stop() {
                    return false;
                }
            }

            if !self.run_step(&timed_action.action, index, speed, doc, sub_ctx) {
                return false;
            }
        }

        true
    }

    /// Run one timeline step. Returns false when this macro should end: a stop request, `exit`,
    /// or a failure that makes going on pointless.
    ///
    /// A step is decided here, on the DSL action, because only here is everything a branch may
    /// need: `call_macro` is expanded by the engine and `conditional` picks a branch, so a
    /// `call_macro` in a branch runs the same way as one on the timeline. Only the branch that is
    /// taken is resolved, so an expression in the other one is never evaluated.
    fn run_step(
        &self,
        step: &DslAction,
        index: usize,
        speed: f32,
        doc: &MacroDocument,
        sub_ctx: &SubMacroContextHandle,
    ) -> bool {
        match step {
            DslAction::CallMacro { name, args } => {
                if !self.execute_call_macro(name, args, speed, sub_ctx) {
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
                    Err(e) => {
                        self.emit(EngineEvent::Error {
                            message: format!("Failed to resolve action {}: {}", index, e),
                        });
                        return false;
                    }
                };
                let branch = if self.evaluate_condition(&condition, doc).is_satisfied() {
                    Some(then_action.as_ref())
                } else {
                    else_action.as_deref()
                };
                match branch {
                    Some(branch) => self.run_step(branch, index, speed, doc, sub_ctx),
                    None => {
                        self.consecutive_failures.store(0, Ordering::Relaxed);
                        self.emit(EngineEvent::ActionCompleted { index });
                        true
                    }
                }
            }
            // Everything else is resolved against the current scope before being dispatched.
            _ => {
                let action = {
                    let vars = self.variables.lock().unwrap();
                    match resolve_action(step, &vars, &self.expr_engine) {
                        Ok(action) => action,
                        Err(e) => {
                            self.emit(EngineEvent::Error {
                                message: format!("Failed to resolve action {}: {}", index, e),
                            });
                            return false;
                        }
                    }
                };

                match self.execute_action(&action, index, doc) {
                    ActionResult::Completed | ActionResult::Timeout => {
                        self.consecutive_failures.store(0, Ordering::Relaxed);
                        self.emit(EngineEvent::ActionCompleted { index });
                        true
                    }
                    ActionResult::Failed => {
                        // The Error was already emitted by execute_action. Keep
                        // going, but stop the whole run if failures pile up so a
                        // broken injector cannot spin forever.
                        let failures =
                            self.consecutive_failures.fetch_add(1, Ordering::Relaxed) + 1;
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
                    ActionResult::Stopped => false,
                    ActionResult::Exit => false, // Exit ends this macro only
                }
            }
        }
    }

    /// Expand and execute a `call_macro` step with cycle/depth protection.
    ///
    /// Returns false only when the whole run should stop; sub-macro guard
    /// violations (cycle, max depth, load/arg errors) are reported as `Error`
    /// events and skipped so the parent timeline keeps running.
    fn execute_call_macro(
        &self,
        name: &str,
        args: &HashMap<String, DslValue>,
        speed: f32,
        sub_ctx: &SubMacroContextHandle,
    ) -> bool {
        if let Err(e) = sub_ctx.lock().unwrap().push(name) {
            self.emit(EngineEvent::Error {
                message: e.to_string(),
            });
            return true; // skip this call, keep running the parent
        }

        let keep_running = self.run_submacro(name, args, speed, sub_ctx);

        sub_ctx.lock().unwrap().pop();
        keep_running
    }

    /// Inner body of a sub-macro call. Kept separate so the call-stack `pop` in
    /// [`Self::execute_call_macro`] always runs, even on early return.
    fn run_submacro(
        &self,
        name: &str,
        args: &HashMap<String, DslValue>,
        speed: f32,
        sub_ctx: &SubMacroContextHandle,
    ) -> bool {
        let child_doc = match (self.loader)(name) {
            Ok(doc) => doc,
            Err(e) => {
                self.emit(EngineEvent::Error {
                    message: format!("Failed to load macro '{}': {}", name, e),
                });
                return true; // skip this call, keep running the parent
            }
        };

        // Snapshot-and-swap: build an isolated child scope (parent snapshot +
        // child defaults + resolved args), run the child, then restore the
        // parent so sibling/parent scopes stay clean.
        let parent_scope = self.variables.lock().unwrap().clone();
        let resolved_args = match prepare_submacro_args(args, &parent_scope, &self.expr_engine) {
            Ok(a) => a,
            Err(e) => {
                self.emit(EngineEvent::Error {
                    message: format!("Failed to prepare args for '{}': {}", name, e),
                });
                return true;
            }
        };
        let child_scope = create_child_variable_store(&parent_scope, &child_doc, resolved_args);
        *self.variables.lock().unwrap() = child_scope;

        let keep_running = self.execute_timeline(&child_doc.timeline, speed, &child_doc, sub_ctx);

        *self.variables.lock().unwrap() = parent_scope;

        // The child returns false on `Exit` as well; only propagate an actual
        // stop request to the parent.
        if !keep_running && self.should_stop() {
            return false;
        }
        true
    }

    fn get_state(&self) -> EngineState {
        *self.state.lock().unwrap()
    }

    fn should_stop(&self) -> bool {
        // Also check for incoming stop commands
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            match cmd {
                EngineCommand::Stop | EngineCommand::EmergencyStop => {
                    self.transition_state(EngineState::Stopped);
                    return true;
                }
                EngineCommand::Pause => {
                    self.transition_state(EngineState::Paused);
                }
                EngineCommand::Resume => {
                    if self.get_state() == EngineState::Paused {
                        self.transition_state(EngineState::Running);
                    }
                }
                EngineCommand::SetDocument(document) => {
                    *self.document.lock().unwrap() = Some(*document);
                }
                EngineCommand::SetRuntimeVars(vars) => {
                    *self.runtime_vars.lock().unwrap() = vars;
                }
                _ => {}
            }
        }

        matches!(self.get_state(), EngineState::Stopped)
    }

    fn transition_state(&self, new_state: EngineState) {
        let old = {
            let mut guard = self.state.lock().unwrap();
            let old = *guard;
            if old != new_state {
                *guard = new_state;
                debug!(?old, ?new_state, "state transition");
                Some(old)
            } else {
                None
            }
        };

        if let Some(old) = old {
            // A pause is a stall: its duration must not advance the timeline clock.
            if new_state == EngineState::Paused {
                self.clock.begin_stall();
            } else if old == EngineState::Paused {
                self.clock.end_stall();
            }
            self.emit(EngineEvent::StateChanged {
                old,
                new: new_state,
            });
        }
    }

    fn emit(&self, event: EngineEvent) {
        if let Err(e) = self.event_tx.try_send(event) {
            warn!("Failed to emit event: {}", e);
        }
    }

    /// Check if target window is focused (if target_window is set).
    fn check_target_window(&self, doc: &MacroDocument) -> bool {
        if let Some(ref target) = doc.target_window {
            if target.pause_when_unfocused {
                return self
                    .platform
                    .is_window_focused(target.title.as_deref(), target.process.as_deref());
            }
        }
        true // No target window binding, always OK
    }

    /// Wait for target window to be focused.
    fn wait_for_target_window(&self, doc: &MacroDocument) -> bool {
        if let Some(ref target) = doc.target_window {
            if !self.check_target_window(doc) {
                self.emit(EngineEvent::TargetWindowUnfocused {
                    title: target.title.clone(),
                    process: target.process.clone(),
                });

                // Waiting for the window is a stall (ended on every exit path by
                // the guard), so the time away does not advance the timeline clock.
                let _stall = self.clock.stall();

                // Wait until window is focused again or stopped
                loop {
                    if self.should_stop() {
                        return false;
                    }
                    if self.check_target_window(doc) {
                        self.emit(EngineEvent::TargetWindowFocused);
                        return true;
                    }
                    thread::sleep(Duration::from_millis(100));
                }
            }
        }
        true
    }

    /// Where the window a window-relative document is measured from is on the screen.
    ///
    /// When the window is not there (closed, minimized) the run waits for it the same way it
    /// waits for a target window that lost focus, and the time spent does not count towards the
    /// timeline clock. `None` means a stop was requested while waiting.
    fn wait_for_window_origin(&self, target: &DslTargetWindow) -> Option<(i32, i32)> {
        let lookup = || {
            self.platform
                .window_origin(target.title.as_deref(), target.process.as_deref())
        };
        if let Some(origin) = lookup() {
            return Some(origin);
        }

        self.emit(EngineEvent::TargetWindowUnfocused {
            title: target.title.clone(),
            process: target.process.clone(),
        });
        let _stall = self.clock.stall();
        loop {
            if self.should_stop() {
                return None;
            }
            if let Some(origin) = lookup() {
                self.emit(EngineEvent::TargetWindowFocused);
                return Some(origin);
            }
            thread::sleep(Duration::from_millis(100));
        }
    }

    /// Execute a single concrete action.
    ///
    /// `doc` is the document the action belongs to: when its coordinates are window-relative, a
    /// positioned action is moved to its screen position here, the one place every injected
    /// action passes through (also the ones nested in an `if`). `ActionStarting` reports the
    /// screen position, i.e. what is injected.
    fn execute_action(&self, action: &Action, index: usize, doc: &MacroDocument) -> ActionResult {
        // Humanize first, in the document's own coordinates: the window shift below then moves
        // the already varied position, so both settings combine without caring about the order.
        let humanized;
        let action = if has_position(action) {
            humanized = self.humanizer.lock().unwrap().jitter_action(action);
            &humanized
        } else {
            action
        };

        let on_screen;
        let action = match window_binding(doc).filter(|_| has_position(action)) {
            Some(target) => {
                let Some(origin) = self.wait_for_window_origin(target) else {
                    return ActionResult::Stopped;
                };
                on_screen = shift_action(action, origin);
                &on_screen
            }
            None => action,
        };

        match action {
            // Counter actions
            Action::SetCounter { key, value } => {
                self.variables.lock().unwrap().set_counter(key, *value);
                self.emit(EngineEvent::CounterChanged {
                    key: key.clone(),
                    value: *value,
                });
                ActionResult::Completed
            }
            Action::IncrCounter { key } => {
                let new_value = self.variables.lock().unwrap().incr_counter(key);
                self.emit(EngineEvent::CounterChanged {
                    key: key.clone(),
                    value: new_value,
                });
                ActionResult::Completed
            }
            Action::DecrCounter { key } => {
                let new_value = self.variables.lock().unwrap().decr_counter(key);
                self.emit(EngineEvent::CounterChanged {
                    key: key.clone(),
                    value: new_value,
                });
                ActionResult::Completed
            }
            Action::ResetCounter { key } => {
                self.variables.lock().unwrap().reset_counter(key);
                self.emit(EngineEvent::CounterChanged {
                    key: key.clone(),
                    value: 0,
                });
                ActionResult::Completed
            }

            // Exit action
            Action::Exit => ActionResult::Exit,

            // WaitUntil action
            Action::WaitUntil {
                condition,
                timeout_ms,
                poll_interval_ms,
            } => {
                let cond_str = format!("{:?}", condition);
                self.emit(EngineEvent::WaitingForCondition {
                    condition: cond_str.clone(),
                });

                let start = Instant::now();
                loop {
                    if self.should_stop() {
                        return ActionResult::Stopped;
                    }

                    // Evaluate condition
                    let result = self.evaluate_condition(condition, doc);
                    if result.is_satisfied() {
                        self.emit(EngineEvent::ConditionSatisfied {
                            condition: cond_str,
                        });
                        return ActionResult::Completed;
                    }

                    // Check timeout
                    if let Some(timeout) = timeout_ms {
                        if start.elapsed().as_millis() as u64 >= *timeout {
                            self.emit(EngineEvent::ConditionTimeout {
                                condition: cond_str,
                            });
                            return ActionResult::Timeout;
                        }
                    }

                    // Wait before next poll
                    self.interruptible_sleep(*poll_interval_ms);
                }
            }

            // Conditional action
            Action::Conditional {
                condition,
                then_action,
                else_action,
            } => {
                let result = self.evaluate_condition(condition, doc);
                if result.is_satisfied() {
                    self.execute_action(then_action, index, doc)
                } else if let Some(else_act) = else_action {
                    self.execute_action(else_act, index, doc)
                } else {
                    ActionResult::Completed
                }
            }

            // Wait action (special handling for interruptibility)
            Action::Wait { ms } => {
                self.interruptible_sleep(*ms);
                ActionResult::Completed
            }

            // Drag is interpolated over its duration by the engine so it stays
            // interruptible (and releases the button safely on stop/failure).
            Action::Drag {
                from,
                to,
                duration_ms,
            } => {
                self.emit(EngineEvent::ActionStarting {
                    index,
                    action: action.clone(),
                });
                self.execute_drag(*from, *to, *duration_ms)
            }

            // All other actions: delegate to executor
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

    /// Perform a drag as MouseDown → interpolated MouseMoves → MouseUp.
    ///
    /// The cursor is stepped from `from` to `to` over `duration_ms`, checking
    /// for stop requests between steps. On a stop or an injection failure the
    /// button is always released first, so a half-finished drag can never leave
    /// the mouse stuck in a pressed state.
    fn execute_drag(&self, from: Point, to: Point, duration_ms: u64) -> ActionResult {
        if let Err(e) = self.inject(&Action::MouseDown {
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
        let step_ms = duration_ms / steps as u64;

        for k in 1..=steps {
            if step_ms > 0 {
                self.interruptible_sleep(step_ms);
            }
            let (x, y) = lerp_point(from, to, k, steps);

            if self.should_stop() {
                // Release where we are, then abort.
                let _ = self.inject(&Action::MouseUp {
                    x,
                    y,
                    button: MouseButton::Left,
                });
                return ActionResult::Stopped;
            }

            if let Err(e) = self.inject(&Action::MouseMove { x, y }) {
                self.emit(EngineEvent::Error {
                    message: format!("Drag move failed: {e}"),
                });
                let _ = self.inject(&Action::MouseUp {
                    x,
                    y,
                    button: MouseButton::Left,
                });
                return ActionResult::Failed;
            }
        }

        if let Err(e) = self.inject(&Action::MouseUp {
            x: to.x,
            y: to.y,
            button: MouseButton::Left,
        }) {
            self.emit(EngineEvent::Error {
                message: format!("Drag release failed: {e}"),
            });
            return ActionResult::Failed;
        }

        ActionResult::Completed
    }

    /// Send a primitive action to the injector.
    fn inject(&self, action: &Action) -> Result<(), String> {
        self.executor.execute(action)
    }

    /// Run the arming countdown for `total_ms`, sleeping in small interruptible
    /// chunks and emitting a [`EngineEvent::CountdownTick`] whenever the
    /// whole-second value changes. Returns `false` if a stop was requested.
    fn arming_countdown(&self, total_ms: u64) -> bool {
        if total_ms == 0 {
            return true;
        }
        let start = Instant::now();
        let mut last_secs: Option<u32> = None;
        loop {
            if self.should_stop() {
                return false;
            }
            let elapsed = start.elapsed().as_millis() as u64;
            if elapsed >= total_ms {
                return true;
            }
            let remaining = total_ms - elapsed;
            let secs = remaining.div_ceil(1000) as u32;
            if last_secs != Some(secs) {
                self.emit(EngineEvent::CountdownTick {
                    remaining_secs: secs,
                });
                last_secs = Some(secs);
            }
            thread::sleep(Duration::from_millis(remaining.min(50)));
        }
    }

    /// Evaluate a condition using the platform provider and variables. In a window-relative
    /// document, `pixel_color` positions are measured from the target window.
    fn evaluate_condition(&self, condition: &Condition, doc: &MacroDocument) -> ConditionResult {
        // Create an evaluator that combines platform and variables
        let evaluator = RuntimeConditionEvaluator {
            platform: &*self.platform,
            variables: &self.variables,
            window: window_binding(doc),
        };
        evaluator.evaluate(condition)
    }

    /// Sleep for the given duration, but can be interrupted by stop commands.
    fn interruptible_sleep(&self, ms: u64) {
        let mut waited = 0u64;
        while waited < ms {
            if self.should_stop() {
                return;
            }
            if self.get_state() == EngineState::Paused {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            let chunk = (ms - waited).min(50);
            thread::sleep(Duration::from_millis(chunk));
            waited += chunk;
        }
    }
}

/// Result of executing an action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionResult {
    Completed,
    Stopped,
    Timeout,
    Exit,
    /// The injector reported a failure for this action.
    Failed,
}

/// Number of interpolation steps for a drag of the given duration.
fn drag_steps(duration_ms: u64) -> u32 {
    // Clamp before narrowing: a huge duration must not wrap around into a small step count.
    let by_time = (duration_ms / DRAG_STEP_MS).min(u64::from(MAX_DRAG_STEPS)) as u32;
    by_time.max(1)
}

/// Linearly interpolate the cursor position at step `k` of `steps` from
/// `from` to `to` (rounded to the nearest pixel).
fn lerp_point(from: Point, to: Point, k: u32, steps: u32) -> (i32, i32) {
    let t = k as f64 / steps as f64;
    let x = from.x as f64 + (to.x - from.x) as f64 * t;
    let y = from.y as f64 + (to.y - from.y) as f64 * t;
    (x.round() as i32, y.round() as i32)
}

/// Runtime condition evaluator combining platform APIs and variable store.
struct RuntimeConditionEvaluator<'a, P: PlatformConditionProvider> {
    platform: &'a P,
    variables: &'a Mutex<VariableStore>,
    /// The window `pixel_color` positions are relative to, for a window-relative document.
    window: Option<&'a DslTargetWindow>,
}

impl<'a, P: PlatformConditionProvider> ConditionEvaluator for RuntimeConditionEvaluator<'a, P> {
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
        self.variables.lock().unwrap().get_counter(key)
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
