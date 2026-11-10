//! Session state: the canonical [`MacroDocument`] being edited/executed.
//!
//! [`SessionStore`] is the single source of truth for the current macro (see the
//! sync contract in `AGENTS.md`). It owns the lossless [`MacroDocument`] plus the
//! run-time variable overrides applied before a run. The UI edits the document
//! directly, so nothing here projects it into a lossy side copy.

use std::collections::HashMap;

use tap_core::{
    validate_document, DslAction, DslRunConfig, DslTimedAction, MacroDocument, MouseButton,
    ValidationError, VariableValue, DSL_VERSION,
};

/// Canonical, in-memory session state for a single macro.
pub struct SessionStore {
    document: MacroDocument,
    runtime_vars: HashMap<String, VariableValue>,
}

impl SessionStore {
    /// A session that starts with a small sample macro, so the first run shows something.
    pub fn new() -> Self {
        Self::with_document(starter_document())
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
        let issues = validate_document(&document).err().unwrap_or_default();
        self.document = document;
        issues
    }

    /// Rename the current document.
    pub fn set_name(&mut self, name: String) {
        self.document.name = name;
    }

    /// Replace the document's steps with a fresh recording.
    pub fn set_recorded_steps(&mut self, steps: Vec<DslTimedAction>) {
        self.document.timeline = steps;
    }

    /// Replace the run-time variable overrides applied on the next run.
    pub fn set_runtime_vars(&mut self, vars: HashMap<String, VariableValue>) {
        self.runtime_vars = vars;
    }

    /// Borrow the current run-time variable overrides.
    pub fn runtime_vars(&self) -> &HashMap<String, VariableValue> {
        &self.runtime_vars
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new()
    }
}

fn starter_document() -> MacroDocument {
    MacroDocument {
        name: "Default".into(),
        description: None,
        version: DSL_VERSION.into(),
        author: None,
        tags: Vec::new(),
        variables: Default::default(),
        target_window: None,
        timeline: vec![
            DslTimedAction::new(0, DslAction::click(640, 360, MouseButton::Left)),
            DslTimedAction::new(500, DslAction::Wait { ms: 500 }),
            DslTimedAction::new(
                1200,
                DslAction::KeyTap {
                    key: "Space".into(),
                },
            ),
        ],
        run: DslRunConfig {
            repeat: 0,
            ..DslRunConfig::default()
        },
    }
}
