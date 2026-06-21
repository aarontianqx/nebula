//! Session state: the canonical [`MacroDocument`] being edited/executed.
//!
//! [`SessionStore`] is the single source of truth for the current macro (see the
//! Phase 6 sync-contract). It owns the lossless [`MacroDocument`] plus the
//! run-time variable overrides applied before a run, and can project a lenient,
//! resolved [`Profile`] view for the (pre-M8) Profile-based IPC surface.

use std::collections::HashMap;

use tap_core::{
    create_expression_engine, DslTimedAction, ExpressionEngineHandle, MacroDocument, Profile,
    Timeline, VariableStore, VariableValue,
};

use crate::resolve::document_to_profile_view;

/// Canonical, in-memory session state for a single macro.
pub struct SessionStore {
    document: MacroDocument,
    runtime_vars: HashMap<String, VariableValue>,
    engine: ExpressionEngineHandle,
}

impl SessionStore {
    /// Create a session seeded with the default document.
    pub fn new() -> Self {
        Self::with_document(MacroDocument::from(&Profile::default()))
    }

    /// Create a session around an existing document.
    pub fn with_document(document: MacroDocument) -> Self {
        Self {
            document,
            runtime_vars: HashMap::new(),
            engine: create_expression_engine(),
        }
    }

    /// Borrow the canonical document.
    pub fn document(&self) -> &MacroDocument {
        &self.document
    }

    /// Replace the canonical document (e.g. on load/import/template apply).
    pub fn set_document(&mut self, document: MacroDocument) {
        self.document = document;
    }

    /// Rename the current document.
    pub fn set_name(&mut self, name: String) {
        self.document.name = name;
    }

    /// Update the document's metadata in place.
    ///
    /// Unlike [`Self::apply_profile_edit`], this is lossless: it touches only the
    /// metadata fields and leaves the timeline, variables and run config intact,
    /// so it is safe to call even for parameterized macros.
    pub fn set_metadata(
        &mut self,
        description: Option<String>,
        author: Option<String>,
        tags: Vec<String>,
    ) {
        self.document.description = description;
        self.document.author = author;
        self.document.tags = tags;
    }

    /// Apply an edit coming from the Profile-based UI.
    ///
    /// `Profile` cannot carry variables or metadata, so this merges the edited
    /// timeline/run/target-window/name over the canonical document while
    /// preserving its variables, description, author, tags and version. This is
    /// the bridge that keeps parameterization alive across visual edits until
    /// the front-end speaks `MacroDocument` directly (M8).
    pub fn apply_profile_edit(&mut self, profile: &Profile) {
        let mut next = MacroDocument::from(profile);
        next.description = self.document.description.clone();
        next.version = self.document.version.clone();
        next.author = self.document.author.clone();
        next.tags = self.document.tags.clone();
        next.variables = self.document.variables.clone();
        self.document = next;
    }

    /// Replace the document's timeline with a freshly recorded one.
    pub fn set_recorded_timeline(&mut self, timeline: Timeline) {
        self.document.timeline = timeline.actions.iter().map(DslTimedAction::from).collect();
    }

    /// Build a lenient, resolved [`Profile`] projection for display/IPC.
    ///
    /// Variable definitions seed the scope with their defaults so parameterized
    /// macros still render sensibly; steps that cannot be resolved (including
    /// `call_macro`) are omitted from the view. The canonical document is
    /// unchanged and still carries the original parameterized steps.
    pub fn profile_view(&self) -> Profile {
        let mut scope = VariableStore::new();
        scope.init_from_definitions(&self.document.variables);
        document_to_profile_view(&self.document, &scope, &self.engine)
    }

    /// Replace the run-time variable overrides applied on the next run.
    pub fn set_runtime_vars(&mut self, vars: HashMap<String, VariableValue>) {
        self.runtime_vars = vars;
    }

    /// Borrow the current run-time variable overrides.
    pub fn runtime_vars(&self) -> &HashMap<String, VariableValue> {
        &self.runtime_vars
    }

    /// Clear all run-time variable overrides.
    pub fn clear_runtime_vars(&mut self) {
        self.runtime_vars.clear();
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}
