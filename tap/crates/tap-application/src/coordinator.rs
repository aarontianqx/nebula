//! The adapter's single entry point: it owns the player and the session, and offers the use cases
//! the UI drives (start, pause, resume, stop, edit the document, set run-time overrides).

use std::collections::HashMap;

use tap_core::{MacroDocument, ValidationError, VariableValue};

use crate::playback::{EngineCommand, EngineEvent, EngineState, PlayerHandle};
use crate::session::SessionStore;

pub struct Coordinator {
    player: PlayerHandle,
    session: SessionStore,
}

impl Coordinator {
    /// Create a coordinator around a spawned player, with a default session.
    pub fn new(player: PlayerHandle) -> Self {
        Self::with_session(player, SessionStore::new())
    }

    /// Create a coordinator with an explicit session.
    pub fn with_session(player: PlayerHandle, session: SessionStore) -> Self {
        Self { player, session }
    }

    // === Engine lifecycle ===

    pub fn engine_state(&self) -> EngineState {
        self.player.state()
    }

    /// Run the session's macro with the session's run-time overrides.
    pub fn start(&self) -> Result<(), String> {
        self.begin_run(
            self.session.document().clone(),
            self.session.runtime_vars().clone(),
        )
    }

    /// Run a macro that is not the session's (the simple-repeat tool), leaving the session's
    /// document and overrides alone so the editor and the engine never disagree about it.
    pub fn start_standalone(&self, document: MacroDocument) -> Result<(), String> {
        self.begin_run(document, HashMap::new())
    }

    fn begin_run(
        &self,
        document: MacroDocument,
        runtime_vars: HashMap<String, VariableValue>,
    ) -> Result<(), String> {
        if self.engine_state() != EngineState::Idle {
            return Err("Cannot start: not in idle state".into());
        }
        self.player.send(EngineCommand::Start {
            document: Box::new(document),
            runtime_vars,
        });
        Ok(())
    }

    /// Pause a running macro.
    pub fn pause(&self) -> Result<(), String> {
        if self.engine_state() != EngineState::Running {
            return Err("Cannot pause: not running".into());
        }
        self.player.send(EngineCommand::Pause);
        Ok(())
    }

    /// Resume a paused macro.
    pub fn resume(&self) -> Result<(), String> {
        if self.engine_state() != EngineState::Paused {
            return Err("Cannot resume: not paused".into());
        }
        self.player.send(EngineCommand::Resume);
        Ok(())
    }

    pub fn stop(&self) {
        self.player.send(EngineCommand::Stop);
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

    /// Replace the canonical document with a UI edit. Never rejected; returns
    /// the validation findings for the UI to display.
    pub fn update_document(&mut self, document: MacroDocument) -> Vec<ValidationError> {
        self.session.update_document(document)
    }

    /// Replace the run-time variable overrides applied on the next run.
    pub fn set_runtime_vars(&mut self, vars: HashMap<String, VariableValue>) {
        self.session.set_runtime_vars(vars);
    }
}
