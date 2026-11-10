//! The macro library: saved macros as YAML files under a root directory, plus the "last used" and
//! "recent" bookkeeping the UI shows.
//!
//! The root is injected by the adapter, so nothing here reaches for a global location and tests
//! run against a temporary directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use tap_core::{document_to_yaml, MacroDocument};
use thiserror::Error;
use tracing::{debug, info, warn};

use crate::playback::SubMacroLoader;

const EXTENSION: &str = "yaml";
/// Kept from when macros were called profiles, so existing saves keep loading.
const MACROS_DIR: &str = "profiles";
const LAST_USED_FILE: &str = "last_profile.txt";
const RECENT_FILE: &str = "recent_profiles.json";
const RECENT_CAP: usize = 8;

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML error: {0}")]
    Yaml(String),
    #[error("Macro not found: {0}")]
    NotFound(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Where tap keeps its data by default: the platform's per-user data directory.
pub fn default_root() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("tap")
}

#[derive(Debug, Clone)]
pub struct MacroLibrary {
    root: PathBuf,
}

impl MacroLibrary {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Persist `doc` under its name, replacing any macro already saved by that name.
    pub fn save(&self, doc: &MacroDocument) -> StorageResult<PathBuf> {
        let dir = self.macros_dir();
        fs::create_dir_all(&dir)?;
        let path = Self::path_in(&dir, &doc.name);
        let yaml = document_to_yaml(doc).map_err(|e| StorageError::Yaml(e.to_string()))?;
        fs::write(&path, yaml)?;
        info!(?path, "Saved macro");
        Ok(path)
    }

    pub fn load(&self, name: &str) -> StorageResult<MacroDocument> {
        let path = Self::path_in(&self.macros_dir(), name);
        if !path.exists() {
            return Err(StorageError::NotFound(name.to_string()));
        }
        let yaml = fs::read_to_string(&path)?;
        let doc = serde_yaml::from_str(&yaml).map_err(|e| StorageError::Yaml(e.to_string()))?;
        debug!(?path, "Loaded macro");
        Ok(doc)
    }

    pub fn delete(&self, name: &str) -> StorageResult<()> {
        let path = Self::path_in(&self.macros_dir(), name);
        if !path.exists() {
            return Err(StorageError::NotFound(name.to_string()));
        }
        fs::remove_file(&path)?;
        info!(name, "Deleted macro");
        Ok(())
    }

    /// Names of the saved macros, sorted.
    pub fn list(&self) -> StorageResult<Vec<String>> {
        let dir = self.macros_dir();
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(dir)? {
            let path = entry?.path();
            if path.extension().is_some_and(|ext| ext == EXTENSION) {
                if let Some(stem) = path.file_stem() {
                    names.push(stem.to_string_lossy().into_owned());
                }
            }
        }
        names.sort();
        Ok(names)
    }

    pub fn last_used(&self) -> Option<String> {
        fs::read_to_string(self.root.join(LAST_USED_FILE)).ok()
    }

    /// Recently used macro names, most recent first.
    pub fn recent(&self) -> Vec<String> {
        fs::read_to_string(self.root.join(RECENT_FILE))
            .ok()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    /// Note that `name` was just saved or opened. Best effort: losing this bookkeeping must not
    /// fail the save or open that triggered it, so a failure is logged and dropped.
    pub fn remember(&self, name: &str) {
        if let Err(e) = self.try_remember(name) {
            warn!(name, error = %e, "could not record the macro as recently used");
        }
    }

    fn try_remember(&self, name: &str) -> StorageResult<()> {
        fs::create_dir_all(&self.root)?;
        fs::write(self.root.join(LAST_USED_FILE), name)?;
        let recent = promote(self.recent(), name);
        fs::write(self.root.join(RECENT_FILE), serde_json::to_string(&recent)?)?;
        Ok(())
    }

    /// What `call_macro` uses to find the macro it names.
    pub fn loader(&self) -> SubMacroLoader {
        let library = self.clone();
        Arc::new(move |name| library.load(name).map_err(|e| e.to_string()))
    }

    fn macros_dir(&self) -> PathBuf {
        self.root.join(MACROS_DIR)
    }

    fn path_in(dir: &Path, name: &str) -> PathBuf {
        dir.join(format!("{}.{EXTENSION}", sanitize_filename(name)))
    }
}

/// Move `name` to the front of `recent`, without duplicates and within the cap.
fn promote(mut recent: Vec<String>, name: &str) -> Vec<String> {
    recent.retain(|n| n != name);
    recent.insert(0, name.to_string());
    recent.truncate(RECENT_CAP);
    recent
}

fn sanitize_filename(name: &str) -> String {
    name.chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c => c,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tap_core::VariableType;
    use tap_core::{DslAction, DslMouseButton, DslTimedAction, DslValue, VariableDefinition};

    /// Create a unique, isolated temporary directory for a test.
    fn temp_dir(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "tap-library-test-{}-{}-{}-{}",
            tag,
            std::process::id(),
            nanos,
            n
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// A document exercising every lossless field: metadata, tags, a typed
    /// variable, a target window, and a coordinate that is a variable reference.
    fn rich_document() -> MacroDocument {
        let mut variables = std::collections::HashMap::new();
        variables.insert(
            "target_x".to_string(),
            VariableDefinition {
                var_type: VariableType::Number,
                default: Some(serde_json::json!(640)),
                description: Some("X coordinate".to_string()),
            },
        );

        MacroDocument {
            name: "Rich Macro".to_string(),
            description: Some("a fully populated document".to_string()),
            version: "1.0".to_string(),
            author: Some("tester".to_string()),
            tags: vec!["game".to_string(), "demo".to_string()],
            variables,
            target_window: Some(tap_core::DslTargetWindow {
                title: Some("Notepad".to_string()),
                process: None,
                pause_when_unfocused: true,
                coordinates: Default::default(),
            }),
            timeline: vec![DslTimedAction {
                at_ms: 100,
                action: DslAction::Click {
                    x: DslValue::String("{{ target_x }}".to_string()),
                    y: DslValue::Int(360),
                    button: DslMouseButton::Left,
                },
                enabled: true,
                note: Some("click target".to_string()),
            }],
            run: Default::default(),
        }
    }

    fn library(tag: &str) -> MacroLibrary {
        MacroLibrary::new(temp_dir(tag))
    }

    #[test]
    fn promote_orders_dedups_and_caps() {
        assert_eq!(
            promote(vec!["a".into(), "b".into()], "c"),
            vec!["c", "a", "b"]
        );
        assert_eq!(
            promote(vec!["a".into(), "b".into(), "c".into()], "c"),
            vec!["c", "a", "b"]
        );

        let mut recent = Vec::new();
        for i in 0..(RECENT_CAP + 4) {
            recent = promote(recent, &format!("p{i}"));
        }
        assert_eq!(recent.len(), RECENT_CAP);
        assert_eq!(recent[0], format!("p{}", RECENT_CAP + 3));
    }

    #[test]
    fn file_names_cannot_escape_the_library() {
        assert_eq!(sanitize_filename("My Macro"), "My Macro");
        assert_eq!(sanitize_filename("test/macro"), "test_macro");
        assert_eq!(sanitize_filename("a:b*c?d"), "a_b_c_d");
    }

    #[test]
    fn a_saved_document_loads_back_unchanged() {
        let library = library("roundtrip");
        let doc = rich_document();

        let path = library.save(&doc).unwrap();
        assert_eq!(path.extension().unwrap(), EXTENSION);

        assert_eq!(library.load(&doc.name).unwrap(), doc);
    }

    #[test]
    fn listing_is_sorted_and_ignores_other_files() {
        let library = library("list");
        for name in ["Charlie", "Alpha", "Bravo"] {
            let doc = MacroDocument {
                name: name.to_string(),
                ..rich_document()
            };
            library.save(&doc).unwrap();
        }
        fs::write(library.macros_dir().join("notes.txt"), "ignore me").unwrap();

        assert_eq!(library.list().unwrap(), vec!["Alpha", "Bravo", "Charlie"]);
    }

    #[test]
    fn an_empty_library_lists_nothing() {
        assert!(library("empty").list().unwrap().is_empty());
    }

    #[test]
    fn deleting_removes_the_macro_and_a_missing_one_is_not_found() {
        let library = library("delete");
        let doc = rich_document();
        library.save(&doc).unwrap();

        library.delete(&doc.name).unwrap();

        assert!(matches!(
            library.load(&doc.name),
            Err(StorageError::NotFound(_))
        ));
        assert!(matches!(
            library.delete(&doc.name),
            Err(StorageError::NotFound(_))
        ));
    }

    #[test]
    fn remembering_updates_last_used_and_recent() {
        let library = library("remember");
        assert_eq!(library.last_used(), None);
        assert!(library.recent().is_empty());

        library.remember("first");
        library.remember("second");
        library.remember("first");

        assert_eq!(library.last_used().as_deref(), Some("first"));
        assert_eq!(library.recent(), vec!["first", "second"]);
    }

    #[test]
    fn the_loader_finds_saved_macros_and_reports_missing_ones() {
        let library = library("loader");
        let doc = rich_document();
        library.save(&doc).unwrap();
        let loader = library.loader();

        assert_eq!(loader(&doc.name).unwrap(), doc);
        assert!(loader("missing").unwrap_err().contains("Macro not found"));
    }
}
