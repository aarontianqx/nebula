//! Execution engine: state machine + player thread.

use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tap_core::{
    create_expression_engine, Action, Condition, ConditionColor, ConditionEvaluator,
    ConditionResult, DslAction, DslTimedAction, DslValue, ExpressionEngineHandle, MacroDocument,
    VariableStore, VariableValue,
};
use tracing::{debug, error, info, warn};

use crate::resolve::resolve_action;
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
    thread: Option<JoinHandle<()>>,
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

    /// Stop and wait for thread to finish.
    pub fn shutdown(mut self) {
        let _ = self.cmd_tx.send(EngineCommand::Stop);
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
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

    /// Create a new player with a custom sub-macro loader (used by tests).
    pub fn spawn_with_loader(executor: E, platform: P, loader: SubMacroLoader) -> PlayerHandle {
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
            cmd_rx,
            event_tx,
        };

        let thread = thread::spawn(move || {
            player.run_loop();
        });

        PlayerHandle {
            cmd_tx,
            event_rx,
            state,
            thread: Some(thread),
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

        // Arming (countdown)
        self.transition_state(EngineState::Arming);

        let countdown_secs = (document.run.start_delay_ms / 1000) as u32;
        for remaining in (1..=countdown_secs).rev() {
            if self.should_stop() {
                return;
            }
            self.emit(EngineEvent::CountdownTick {
                remaining_secs: remaining,
            });
            thread::sleep(Duration::from_secs(1));
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
        let start = Instant::now();

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
            let elapsed = start.elapsed().as_millis() as u64;
            if target_ms > elapsed {
                let wait_ms = target_ms - elapsed;
                self.interruptible_sleep(wait_ms);
                if self.should_stop() {
                    return false;
                }
            }

            // `call_macro` is expanded inline by the engine. Everything else is
            // resolved against the current scope before being dispatched.
            if let DslAction::CallMacro { name, args } = &timed_action.action {
                if !self.execute_call_macro(name, args, speed, sub_ctx) {
                    return false;
                }
                self.emit(EngineEvent::ActionCompleted { index });
                continue;
            }

            let action = {
                let vars = self.variables.lock().unwrap();
                match resolve_action(&timed_action.action, &vars, &self.expr_engine) {
                    Ok(action) => action,
                    Err(e) => {
                        self.emit(EngineEvent::Error {
                            message: format!("Failed to resolve action {}: {}", index, e),
                        });
                        return false;
                    }
                }
            };

            match self.execute_action(&action, index) {
                ActionResult::Completed | ActionResult::Timeout => {
                    self.emit(EngineEvent::ActionCompleted { index });
                }
                ActionResult::Stopped => return false,
                ActionResult::Exit => return false, // Exit ends this macro only
            }
        }

        true
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
        let resolved_args = match prepare_submacro_args(args, &parent_scope) {
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

    /// Execute a single action (including new Phase 3 actions).
    fn execute_action(&self, action: &Action, index: usize) -> ActionResult {
        match action {
            // Phase 3: Counter actions
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

            // Phase 3: Exit action
            Action::Exit => ActionResult::Exit,

            // Phase 3: WaitUntil action
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
                    let result = self.evaluate_condition(condition);
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

            // Phase 3: Conditional action
            Action::Conditional {
                condition,
                then_action,
                else_action,
            } => {
                let result = self.evaluate_condition(condition);
                if result.is_satisfied() {
                    self.execute_action(then_action, index)
                } else if let Some(else_act) = else_action {
                    self.execute_action(else_act, index)
                } else {
                    ActionResult::Completed
                }
            }

            // Wait action (special handling for interruptibility)
            Action::Wait { ms } => {
                self.interruptible_sleep(*ms);
                ActionResult::Completed
            }

            // All other actions: delegate to executor
            _ => {
                self.emit(EngineEvent::ActionStarting {
                    index,
                    action: action.clone(),
                });

                if let Err(e) = self.executor.execute(action) {
                    error!(index, error = %e, "action execution failed");
                    self.emit(EngineEvent::Error {
                        message: format!("Action {} failed: {}", index, e),
                    });
                }

                ActionResult::Completed
            }
        }
    }

    /// Evaluate a condition using the platform provider and variables.
    fn evaluate_condition(&self, condition: &Condition) -> ConditionResult {
        // Create an evaluator that combines platform and variables
        let evaluator = RuntimeConditionEvaluator {
            platform: &*self.platform,
            variables: &self.variables,
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
}

/// Runtime condition evaluator combining platform APIs and variable store.
struct RuntimeConditionEvaluator<'a, P: PlatformConditionProvider> {
    platform: &'a P,
    variables: &'a Mutex<VariableStore>,
}

impl<'a, P: PlatformConditionProvider> ConditionEvaluator for RuntimeConditionEvaluator<'a, P> {
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool {
        self.platform.is_window_focused(title, process)
    }

    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool {
        self.platform.window_exists(title, process)
    }

    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor> {
        self.platform.get_pixel_color(x, y)
    }

    fn get_counter(&self, key: &str) -> i32 {
        self.variables.lock().unwrap().get_counter(key)
    }
}
