//! Profile storage and persistence.
//!
//! The on-disk format is YAML carrying a full `MacroDocument` (metadata +
//! variables + timeline), which round-trips losslessly.

use std::fs;
use std::path::{Path, PathBuf};
use tap_core::Profile;
use tap_core::{document_to_yaml, MacroDocument};
use thiserror::Error;
use tracing::{debug, info};

/// Canonical document extension.
const DOC_EXT: &str = "yaml";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("YAML error: {0}")]
    Yaml(String),
    #[error("Document conversion error: {0}")]
    Conversion(String),
    #[error("Profile not found: {0}")]
    NotFound(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Get the app data directory for tap.
pub fn get_app_data_dir() -> PathBuf {
    let base = dirs_next::data_dir().unwrap_or_else(|| PathBuf::from("."));
    base.join("tap")
}

/// Get the profiles directory.
pub fn get_profiles_dir() -> PathBuf {
    get_app_data_dir().join("profiles")
}

/// Ensure the profiles directory exists.
pub fn ensure_profiles_dir() -> StorageResult<PathBuf> {
    let dir = get_profiles_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
        info!(?dir, "Created profiles directory");
    }
    Ok(dir)
}

// ============================================================================
// Document API (canonical, lossless)
// ============================================================================

/// Save a macro document to disk as YAML.
pub fn save_document(doc: &MacroDocument) -> StorageResult<PathBuf> {
    let dir = ensure_profiles_dir()?;
    save_document_to(&dir, doc)
}

/// Load a macro document from disk by name.
pub fn load_document(name: &str) -> StorageResult<MacroDocument> {
    load_document_from(&get_profiles_dir(), name)
}

fn save_document_to(dir: &Path, doc: &MacroDocument) -> StorageResult<PathBuf> {
    let filename = sanitize_filename(&doc.name);
    let path = dir.join(format!("{}.{}", filename, DOC_EXT));

    let yaml = document_to_yaml(doc).map_err(|e| StorageError::Yaml(e.to_string()))?;
    fs::write(&path, yaml)?;

    info!(?path, "Saved macro document");
    Ok(path)
}

fn load_document_from(dir: &Path, name: &str) -> StorageResult<MacroDocument> {
    let filename = sanitize_filename(name);

    let yaml_path = dir.join(format!("{}.{}", filename, DOC_EXT));
    if yaml_path.exists() {
        let yaml = fs::read_to_string(&yaml_path)?;
        let doc: MacroDocument =
            serde_yaml::from_str(&yaml).map_err(|e| StorageError::Yaml(e.to_string()))?;
        debug!(?yaml_path, "Loaded macro document");
        return Ok(doc);
    }

    Err(StorageError::NotFound(name.to_string()))
}

// ============================================================================
// Profile API (resolved runtime form, thin adapters over the document API)
// ============================================================================

/// Load a runtime profile from disk by name.
///
/// Loads the canonical document and resolves it to the runtime form. Fails with
/// [`StorageError::Conversion`] if the document cannot be resolved (e.g. it uses
/// unresolved variable references in coordinates).
pub fn load_profile(name: &str) -> StorageResult<Profile> {
    let doc = load_document(name)?;
    Profile::try_from(&doc).map_err(|e| StorageError::Conversion(e.to_string()))
}

/// Delete a profile from disk.
pub fn delete_profile(name: &str) -> StorageResult<()> {
    delete_profile_in(&get_profiles_dir(), name)
}

fn delete_profile_in(dir: &Path, name: &str) -> StorageResult<()> {
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.{}", filename, DOC_EXT));

    if !path.exists() {
        return Err(StorageError::NotFound(name.to_string()));
    }

    fs::remove_file(&path)?;
    info!(%name, "Deleted profile");
    Ok(())
}

/// List all saved profiles, sorted by name.
pub fn list_profiles() -> StorageResult<Vec<String>> {
    list_profiles_in(&get_profiles_dir())
}

fn list_profiles_in(dir: &Path) -> StorageResult<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    // BTreeSet yields a stable, sorted ordering.
    let mut names = std::collections::BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_profile = path.extension().map(|e| e == DOC_EXT).unwrap_or(false);
        if is_profile {
            if let Some(stem) = path.file_stem() {
                names.insert(stem.to_string_lossy().to_string());
            }
        }
    }

    Ok(names.into_iter().collect())
}

/// Get the path to the "last used" profile marker.
fn get_last_used_path() -> PathBuf {
    get_app_data_dir().join("last_profile.txt")
}

/// Save the name of the last used profile.
pub fn save_last_used(name: &str) -> StorageResult<()> {
    let path = get_last_used_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, name)?;
    debug!(?name, "Saved last used profile");
    Ok(())
}

/// Load the name of the last used profile.
pub fn load_last_used() -> Option<String> {
    let path = get_last_used_path();
    if path.exists() {
        fs::read_to_string(&path).ok()
    } else {
        None
    }
}

/// Maximum number of entries kept in the recently-used list.
const RECENTS_CAP: usize = 8;

/// Path to the recently-used profiles list.
fn get_recents_path() -> PathBuf {
    get_app_data_dir().join("recent_profiles.json")
}

/// Load the recently-used profile names (most recent first).
pub fn load_recent() -> Vec<String> {
    let path = get_recents_path();
    if !path.exists() {
        return Vec::new();
    }
    fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
        .unwrap_or_default()
}

/// Promote `name` to the front of the recents list (de-duplicated, capped).
fn promote_recent(mut recents: Vec<String>, name: &str) -> Vec<String> {
    recents.retain(|n| n != name);
    recents.insert(0, name.to_string());
    recents.truncate(RECENTS_CAP);
    recents
}

/// Record a profile as recently used (most recent first, de-duplicated, capped).
pub fn save_recent(name: &str) -> StorageResult<()> {
    let recents = promote_recent(load_recent(), name);

    let path = get_recents_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string(&recents)?)?;
    debug!(?name, "Recorded recently-used profile");
    Ok(())
}

/// Sanitize a profile name to be a valid filename.
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
            "tap-storage-test-{}-{}-{}-{}",
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

    #[test]
    fn test_promote_recent_orders_dedups_and_caps() {
        // Fresh entry goes to the front.
        let r = promote_recent(vec!["a".into(), "b".into()], "c");
        assert_eq!(r, vec!["c", "a", "b"]);

        // Re-using an existing entry moves it to the front without duplicating.
        let r = promote_recent(vec!["a".into(), "b".into(), "c".into()], "c");
        assert_eq!(r, vec!["c", "a", "b"]);

        // The list is capped at RECENTS_CAP, dropping the oldest entries.
        let mut acc: Vec<String> = Vec::new();
        for i in 0..(RECENTS_CAP + 4) {
            acc = promote_recent(acc, &format!("p{i}"));
        }
        assert_eq!(acc.len(), RECENTS_CAP);
        assert_eq!(acc.first().unwrap(), &format!("p{}", RECENTS_CAP + 3));
    }

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Profile"), "My Profile");
        assert_eq!(sanitize_filename("test/profile"), "test_profile");
        assert_eq!(sanitize_filename("a:b*c?d"), "a_b_c_d");
    }

    #[test]
    fn test_document_roundtrip_is_lossless() {
        let dir = temp_dir("roundtrip");
        let doc = rich_document();

        let path = save_document_to(&dir, &doc).unwrap();
        assert_eq!(path.extension().unwrap(), DOC_EXT);

        let loaded = load_document_from(&dir, &doc.name).unwrap();
        assert_eq!(loaded, doc, "document must round-trip without loss");

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_profiles_sorted() {
        let dir = temp_dir("list");

        for name in ["Charlie", "Alpha", "Bravo"] {
            save_document_to(
                &dir,
                &MacroDocument {
                    name: name.to_string(),
                    ..rich_document()
                },
            )
            .unwrap();
        }
        // A non-profile file is ignored.
        fs::write(dir.join("notes.txt"), "ignore me").unwrap();

        let names = list_profiles_in(&dir).unwrap();
        assert_eq!(names, vec!["Alpha", "Bravo", "Charlie"]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_delete_profile() {
        let dir = temp_dir("delete");
        let stem = "Doomed";
        fs::write(
            dir.join(format!("{stem}.{DOC_EXT}")),
            "name: Doomed\ntimeline: []\n",
        )
        .unwrap();

        delete_profile_in(&dir, stem).unwrap();
        assert!(!dir.join(format!("{stem}.{DOC_EXT}")).exists());

        // Deleting a missing profile reports NotFound.
        assert!(matches!(
            delete_profile_in(&dir, "missing"),
            Err(StorageError::NotFound(_))
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
