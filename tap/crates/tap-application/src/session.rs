//! Session state: the canonical [`MacroDocument`] being edited/executed.
//!
//! [`SessionStore`] is the single source of truth for the current macro (see the
//! sync contract in `AGENTS.md`). It owns the lossless [`MacroDocument`] plus the
//! run-time variable overrides applied before a run. The UI edits the document
//! directly, so nothing here projects it into a lossy side copy.

use std::collections::HashMap;

use tap_core::{
    validate_profile, DslTimedAction, MacroDocument, Profile, Timeline, ValidationError,
    VariableValue,
};

/// Canonical, in-memory session state for a single macro.
pub struct SessionStore {
    document: MacroDocument,
    runtime_vars: HashMap<String, VariableValue>,
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

    /// Replace the canonical document with an edit coming from the UI.
    ///
    /// Edits are **never rejected**: the UI legitimately passes through states
    /// that are not yet runnable (an empty timeline, a half-typed variable
    /// name). The document is stored exactly as given, and the validation
    /// findings are returned so the UI can show them inline.
    pub fn update_document(&mut self, document: MacroDocument) -> Vec<ValidationError> {
        let issues = validate_profile(&document).err().unwrap_or_default();
        self.document = document;
        issues
    }

    /// Rename the current document.
    pub fn set_name(&mut self, name: String) {
        self.document.name = name;
    }

    /// Replace the document's timeline with a freshly recorded one.
    pub fn set_recorded_timeline(&mut self, timeline: Timeline) {
        self.document.timeline = timeline.actions.iter().map(DslTimedAction::from).collect();
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
