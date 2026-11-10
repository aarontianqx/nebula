//! The player thread: it owns the state machine and the command inbox, and plays one macro at a
//! time. How a run proceeds lives in `run`, how single actions are performed in `dispatch`.

use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, Receiver, Sender};
use tap_core::{create_expression_engine, DslTargetWindow, ExpressionEngineHandle};
use tracing::{debug, info, warn};

use super::clock::RunClock;
use super::humanize::{Humanizer, JitterRng};
use super::ports::{ActionExecutor, PlatformConditionProvider, SubMacroLoader};
use super::protocol::{EngineCommand, EngineEvent, EngineState};
use super::run::RunState;

/// How often a stalled run looks again for its window.
const WINDOW_POLL: Duration = Duration::from_millis(100);
/// Granularity of every wait, so that a stop or pause is noticed within this long.
const WAIT_SLICE_MS: u64 = 50;

pub struct PlayerConfig {
    loader: SubMacroLoader,
    seed: u64,
}

impl PlayerConfig {
    /// `loader` finds the macros `call_macro` names.
    pub fn new(loader: SubMacroLoader) -> Self {
        Self {
            loader,
            seed: JitterRng::entropy_seed(),
        }
    }

    /// Draw `run.humanize` variation from `seed`, so a run can be reproduced exactly.
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }
}

/// The caller's side of a player: commands in, events out, and the current state.
pub struct PlayerHandle {
    cmd_tx: Sender<EngineCommand>,
    event_rx: Receiver<EngineEvent>,
    state: Arc<Mutex<EngineState>>,
}

impl PlayerHandle {
    pub fn send(&self, cmd: EngineCommand) {
        if let Err(e) = self.cmd_tx.send(cmd) {
            warn!("Failed to send command to player: {}", e);
        }
    }

    pub fn try_recv(&self) -> Option<EngineEvent> {
        self.event_rx.try_recv().ok()
    }

    pub fn state(&self) -> EngineState {
        *self.state.lock().unwrap()
    }
}

pub struct Player<E, P> {
    pub(super) executor: E,
    pub(super) platform: P,
    pub(super) loader: SubMacroLoader,
    pub(super) expr_engine: ExpressionEngineHandle,
    /// Lives across runs: the generator keeps going, each run restarts only its settings.
    pub(super) humanizer: Humanizer,
    /// Time spent stalled (paused, window missing) does not count towards a step's `at_ms`.
    pub(super) clock: RunClock,
    pub(super) run: RunState,
    state: Arc<Mutex<EngineState>>,
    cmd_rx: Receiver<EngineCommand>,
    event_tx: Sender<EngineEvent>,
}

impl<E: ActionExecutor, P: PlatformConditionProvider> Player<E, P> {
    /// Start the player thread. It exits when every handle's command sender is gone.
    pub fn spawn(executor: E, platform: P, config: PlayerConfig) -> PlayerHandle {
        let (cmd_tx, cmd_rx) = bounded(32);
        let (event_tx, event_rx) = bounded(256);
        let state = Arc::new(Mutex::new(EngineState::Idle));

        let player = Player {
            executor,
            platform,
            loader: config.loader,
            expr_engine: create_expression_engine(),
            humanizer: Humanizer::new(config.seed),
            clock: RunClock::new(),
            run: RunState::default(),
            state: state.clone(),
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

    fn run_loop(mut self) {
        info!("Player thread started");

        while let Ok(cmd) = self.cmd_rx.recv() {
            debug!(?cmd, "handling command");
            match cmd {
                EngineCommand::Start {
                    document,
                    runtime_vars,
                } => self.play(*document, runtime_vars),
                other => {
                    if self.apply(other) {
                        self.transition_state(EngineState::Idle);
                    }
                }
            }
        }

        info!("Player thread exiting");
    }

    /// Apply a command that means the same whatever the player is doing. Returns whether it
    /// stops the run.
    fn apply(&mut self, cmd: EngineCommand) -> bool {
        match cmd {
            EngineCommand::Pause if self.state() == EngineState::Running => {
                self.transition_state(EngineState::Paused);
            }
            EngineCommand::Resume if self.state() == EngineState::Paused => {
                self.transition_state(EngineState::Running);
            }
            EngineCommand::Stop => {
                self.transition_state(EngineState::Stopped);
                return true;
            }
            // A pause or resume that does not apply, or a start while already playing.
            _ => {}
        }
        false
    }

    /// Take in pending commands. Returns whether the run has to stop.
    pub(super) fn should_stop(&mut self) -> bool {
        while let Ok(cmd) = self.cmd_rx.try_recv() {
            if self.apply(cmd) {
                return true;
            }
        }
        self.state() == EngineState::Stopped
    }

    pub(super) fn state(&self) -> EngineState {
        *self.state.lock().unwrap()
    }

    pub(super) fn transition_state(&mut self, new_state: EngineState) {
        let old = {
            let mut guard = self.state.lock().unwrap();
            let old = *guard;
            if old == new_state {
                return;
            }
            *guard = new_state;
            old
        };
        debug!(?old, ?new_state, "state transition");

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

    pub(super) fn emit(&self, event: EngineEvent) {
        if let Err(e) = self.event_tx.try_send(event) {
            warn!("Failed to emit event: {}", e);
        }
    }

    /// Block while paused. Returns false if a stop came first.
    pub(super) fn wait_while_paused(&mut self) -> bool {
        loop {
            if self.should_stop() {
                return false;
            }
            if self.state() != EngineState::Paused {
                return true;
            }
            thread::sleep(Duration::from_millis(WAIT_SLICE_MS));
        }
    }

    /// Sleep for `ms`, cut short by a stop and frozen while paused.
    pub(super) fn interruptible_sleep(&mut self, ms: u64) {
        let mut waited = 0u64;
        while waited < ms {
            if !self.wait_while_paused() {
                return;
            }
            let chunk = (ms - waited).min(WAIT_SLICE_MS);
            thread::sleep(Duration::from_millis(chunk));
            waited += chunk;
        }
    }

    /// Count down `total_ms`, reporting each whole second. Returns false if a stop came first.
    pub(super) fn arming_countdown(&mut self, total_ms: u64) -> bool {
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
            thread::sleep(Duration::from_millis(remaining.min(WAIT_SLICE_MS)));
        }
    }

    /// Find what `probe` looks for on the screen, waiting for it if it is not there yet. The
    /// wait is a stall: it does not count towards the timeline clock. `None` means a stop was
    /// requested while waiting.
    pub(super) fn wait_for_window<T>(
        &mut self,
        target: &DslTargetWindow,
        probe: impl Fn(&P) -> Option<T>,
    ) -> Option<T> {
        if let Some(found) = probe(&self.platform) {
            return Some(found);
        }

        self.emit(EngineEvent::TargetWindowUnfocused {
            title: target.title.clone(),
            process: target.process.clone(),
        });
        self.clock.begin_stall();
        let found = loop {
            if self.should_stop() {
                break None;
            }
            if let Some(found) = probe(&self.platform) {
                self.emit(EngineEvent::TargetWindowFocused);
                break Some(found);
            }
            thread::sleep(WINDOW_POLL);
        };
        self.clock.end_stall();
        found
    }
}
