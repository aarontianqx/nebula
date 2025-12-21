//! Execution engine: state machine + player thread.

use crate::{Action, Profile, Repeat, TimedAction};
use crossbeam_channel::{bounded, Receiver, Sender};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tracing::{debug, error, info, warn};

/// Engine state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EngineState {
    /// Idle, waiting for start command.
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

impl Default for EngineState {
    fn default() -> Self {
        Self::Idle
    }
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
    /// Update the profile to execute.
    SetProfile(Profile),
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

/// Trait for injecting actions (implemented by tap-platform).
pub trait ActionExecutor: Send + Sync {
    fn execute(&self, action: &Action) -> Result<(), String>;
}

/// Player: runs in a separate thread, executes timeline actions.
pub struct Player<E: ActionExecutor> {
    executor: Arc<E>,
    profile: Arc<Mutex<Option<Profile>>>,
    state: Arc<Mutex<EngineState>>,
    cmd_rx: Receiver<EngineCommand>,
    event_tx: Sender<EngineEvent>,
}

impl<E: ActionExecutor + 'static> Player<E> {
    /// Create a new player and return a handle to control it.
    pub fn spawn(executor: E) -> PlayerHandle {
        let (cmd_tx, cmd_rx) = bounded(32);
        let (event_tx, event_rx) = bounded(256);
        let state = Arc::new(Mutex::new(EngineState::Idle));
        let profile = Arc::new(Mutex::new(None));

        let player = Player {
            executor: Arc::new(executor),
            profile: profile.clone(),
            state: state.clone(),
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

        loop {
            // Wait for a command
            match self.cmd_rx.recv() {
                Ok(cmd) => {
                    if !self.handle_command(cmd) {
                        break;
                    }
                }
                Err(_) => {
                    // Channel closed, exit
                    break;
                }
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
            EngineCommand::SetProfile(profile) => {
                *self.profile.lock().unwrap() = Some(profile);
            }
        }

        true
    }

    fn start_execution(&self) {
        let profile = {
            let guard = self.profile.lock().unwrap();
            match guard.clone() {
                Some(p) => p,
                None => {
                    self.emit(EngineEvent::Error {
                        message: "No profile set".into(),
                    });
                    return;
                }
            }
        };

        // Arming (countdown)
        self.transition_state(EngineState::Arming);

        let countdown_secs = (profile.run.start_delay_ms / 1000) as u32;
        for remaining in (1..=countdown_secs).rev() {
            if self.should_stop() {
                return;
            }
            self.emit(EngineEvent::CountdownTick {
                remaining_secs: remaining,
            });
            thread::sleep(Duration::from_secs(1));
        }

        // Start running
        self.transition_state(EngineState::Running);

        let repeat = profile.run.repeat;
        let speed = profile.run.speed;
        let mut iteration = 0u32;

        loop {
            iteration += 1;

            // Execute one iteration of the timeline
            if !self.execute_timeline(&profile.timeline.actions, speed) {
                // Stopped during execution
                break;
            }

            self.emit(EngineEvent::IterationCompleted { iteration });

            // Check repeat condition
            match repeat {
                Repeat::Times(n) if iteration >= n => {
                    self.emit(EngineEvent::Completed);
                    break;
                }
                Repeat::Times(_) => continue,
                Repeat::Forever => continue,
            }
        }

        self.transition_state(EngineState::Stopped);
        self.transition_state(EngineState::Idle);
    }

    /// Execute a timeline. Returns false if stopped.
    fn execute_timeline(&self, actions: &[TimedAction], speed: f32) -> bool {
        let start = Instant::now();

        for (index, timed_action) in actions.iter().enumerate() {
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

            // Wait until the scheduled time
            let target_ms = (timed_action.at_ms as f32 / speed) as u64;
            let elapsed = start.elapsed().as_millis() as u64;
            if target_ms > elapsed {
                let wait_ms = target_ms - elapsed;
                // Wait in small chunks to allow stop checks
                let mut waited = 0u64;
                while waited < wait_ms {
                    if self.should_stop() {
                        return false;
                    }
                    if self.get_state() == EngineState::Paused {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    let chunk = (wait_ms - waited).min(50);
                    thread::sleep(Duration::from_millis(chunk));
                    waited += chunk;
                }
            }

            // Handle Wait action specially (it's a delay, not an injection)
            if let Action::Wait { ms } = &timed_action.action {
                debug!(ms, "executing wait action");
                let wait_ms = (*ms as f32 / speed) as u64;
                let mut waited = 0u64;
                while waited < wait_ms {
                    if self.should_stop() {
                        return false;
                    }
                    if self.get_state() == EngineState::Paused {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    let chunk = (wait_ms - waited).min(50);
                    thread::sleep(Duration::from_millis(chunk));
                    waited += chunk;
                }
                self.emit(EngineEvent::ActionCompleted { index });
                continue;
            }

            // Execute the action
            self.emit(EngineEvent::ActionStarting {
                index,
                action: timed_action.action.clone(),
            });

            if let Err(e) = self.executor.execute(&timed_action.action) {
                error!(index, error = %e, "action execution failed");
                self.emit(EngineEvent::Error {
                    message: format!("Action {} failed: {}", index, e),
                });
            }

            self.emit(EngineEvent::ActionCompleted { index });
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
                EngineCommand::SetProfile(p) => {
                    *self.profile.lock().unwrap() = Some(p);
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
            self.emit(EngineEvent::StateChanged { old, new: new_state });
        }
    }

    fn emit(&self, event: EngineEvent) {
        if let Err(e) = self.event_tx.try_send(event) {
            warn!("Failed to emit event: {}", e);
        }
    }
}

/// Adapter to use tap-platform's InputInjector as ActionExecutor.
pub struct InjectorExecutor<I> {
    injector: I,
}

impl<I> InjectorExecutor<I> {
    pub fn new(injector: I) -> Self {
        Self { injector }
    }
}

impl<I> ActionExecutor for InjectorExecutor<I>
where
    I: crate::ActionExecutorAdapter + Send + Sync,
{
    fn execute(&self, action: &Action) -> Result<(), String> {
        self.injector.inject(action)
    }
}

/// Adapter trait for external injectors (to avoid circular dependency).
pub trait ActionExecutorAdapter {
    fn inject(&self, action: &Action) -> Result<(), String>;
}

