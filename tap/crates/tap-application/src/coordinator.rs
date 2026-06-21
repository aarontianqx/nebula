//! Coordinator: the application-layer entry point for macro execution.
//!
//! Owns the playback [`PlayerHandle`] and the canonical [`SessionStore`], and
//! exposes the use-cases the adapter (`src-tauri`) drives: lifecycle control
//! (start/pause/resume/stop), document/session mutation, run-time variable
//! overrides and engine-event draining. The adapter holds one `Coordinator`
//! and no longer owns the engine state machine itself.

use std::collections::HashMap;

use tap_core::{MacroDocument, Profile, VariableValue};

use crate::engine::{EngineCommand, EngineEvent, EngineState, PlayerHandle};
use crate::session::SessionStore;

/// Orchestrates the player thread and the canonical session document.
pub struct Coordinator {
    player: PlayerHandle,
    session: SessionStore,
    engine_state: EngineState,
}

impl Coordinator {
    /// Create a coordinator around a spawned player, with a default session.
    pub fn new(player: PlayerHandle) -> Self {
        Self::with_session(player, SessionStore::new())
    }

    /// Create a coordinator with an explicit session.
    pub fn with_session(player: PlayerHandle, session: SessionStore) -> Self {
        Self {
            player,
            session,
            engine_state: EngineState::Idle,
        }
    }

    // === Engine lifecycle ===

    /// Current engine state (mirrored from player events via [`Self::drain_events`]).
    pub fn engine_state(&self) -> EngineState {
        self.engine_state
    }

    /// Update the mirrored engine state (called by the adapter event pump).
    pub fn set_engine_state(&mut self, state: EngineState) {
        self.engine_state = state;
    }

    /// Push the canonical document + run-time overrides and start a run.
    pub fn start(&self) -> Result<(), String> {
        if self.engine_state != EngineState::Idle {
            return Err("Cannot start: not in idle state".into());
        }
        self.player.send(EngineCommand::SetDocument(Box::new(
            self.session.document().clone(),
        )));
        self.player.send(EngineCommand::SetRuntimeVars(
            self.session.runtime_vars().clone(),
        ));
        self.player.send(EngineCommand::Start);
        Ok(())
    }

    /// Pause a running macro.
    pub fn pause(&self) -> Result<(), String> {
        if self.engine_state != EngineState::Running {
            return Err("Cannot pause: not running".into());
        }
        self.player.send(EngineCommand::Pause);
        Ok(())
    }

    /// Resume a paused macro.
    pub fn resume(&self) -> Result<(), String> {
        if self.engine_state != EngineState::Paused {
            return Err("Cannot resume: not paused".into());
        }
        self.player.send(EngineCommand::Resume);
        Ok(())
    }

    /// Stop the current run (cooperative).
    pub fn stop(&self) {
        self.player.send(EngineCommand::Stop);
    }

    /// Emergency stop (highest priority).
    pub fn emergency_stop(&self) {
        self.player.send(EngineCommand::EmergencyStop);
    }

    /// Drain all pending engine events (non-blocking).
    pub fn drain_events(&self) -> Vec<EngineEvent> {
        std::iter::from_fn(|| self.player.try_recv()).collect()
    }

    // === Session / document ===

    /// Borrow the session store.
    pub fn session(&self) -> &SessionStore {
        &self.session
    }

    /// Mutably borrow the session store.
    pub fn session_mut(&mut self) -> &mut SessionStore {
        &mut self.session
    }

    /// Borrow the canonical document.
    pub fn document(&self) -> &MacroDocument {
        self.session.document()
    }

    /// Replace the canonical document.
    pub fn set_document(&mut self, document: MacroDocument) {
        self.session.set_document(document);
    }

    /// Apply a Profile edit (variable-preserving merge).
    pub fn apply_profile_edit(&mut self, profile: &Profile) {
        self.session.apply_profile_edit(profile);
    }

    /// Lenient resolved Profile projection for display/IPC.
    pub fn profile_view(&self) -> Profile {
        self.session.profile_view()
    }

    /// Replace the run-time variable overrides applied on the next run.
    pub fn set_runtime_vars(&mut self, vars: HashMap<String, VariableValue>) {
        self.session.set_runtime_vars(vars);
    }

    /// Borrow the current run-time variable overrides.
    pub fn runtime_vars(&self) -> &HashMap<String, VariableValue> {
        self.session.runtime_vars()
    }
}
