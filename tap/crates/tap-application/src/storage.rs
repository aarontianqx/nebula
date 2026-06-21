//! Profile storage and persistence.
//!
//! The canonical on-disk format is YAML carrying a full `MacroDocument`
//! (metadata + variables + timeline), which round-trips losslessly. Legacy
//! profiles serialized as JSON (the resolved runtime `Profile`) are still
//! readable for backward compatibility and are migrated to YAML on the next
//! save.

use std::fs;
use std::path::{Path, PathBuf};
use tap_core::Profile;
use tap_core::{document_to_yaml, MacroDocument};
use thiserror::Error;
use tracing::{debug, info};

/// Canonical document extension.
const DOC_EXT: &str = "yaml";
/// Legacy runtime-profile extension (read-only, migrated on save).
const LEGACY_EXT: &str = "json";

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
///
/// Prefers the canonical YAML form; falls back to a legacy JSON-serialized
/// runtime profile (lifted into a document) for backward compatibility.
pub fn load_document(name: &str) -> StorageResult<MacroDocument> {
    load_document_from(&get_profiles_dir(), name)
}

fn save_document_to(dir: &Path, doc: &MacroDocument) -> StorageResult<PathBuf> {
    let filename = sanitize_filename(&doc.name);
    let path = dir.join(format!("{}.{}", filename, DOC_EXT));

    let yaml = document_to_yaml(doc).map_err(|e| StorageError::Yaml(e.to_string()))?;
    fs::write(&path, yaml)?;

    // Migrate away from a stale legacy JSON file with the same stem.
    let legacy = dir.join(format!("{}.{}", filename, LEGACY_EXT));
    if legacy.exists() {
        let _ = fs::remove_file(&legacy);
    }

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

    // Backward compatibility: a legacy runtime profile serialized as JSON.
    let json_path = dir.join(format!("{}.{}", filename, LEGACY_EXT));
    if json_path.exists() {
        let json = fs::read_to_string(&json_path)?;
        let profile: Profile = serde_json::from_str(&json)?;
        debug!(
            ?json_path,
            "Loaded legacy profile (json) and lifted to document"
        );
        return Ok(MacroDocument::from(&profile));
    }

    Err(StorageError::NotFound(name.to_string()))
}

// ============================================================================
// Profile API (resolved runtime form, thin adapters over the document API)
// ============================================================================

/// Save a runtime profile to disk (as a canonical YAML document).
pub fn save_profile(profile: &Profile) -> StorageResult<PathBuf> {
    save_document(&MacroDocument::from(profile))
}

/// Load a runtime profile from disk by name.
///
/// Loads the canonical document and resolves it to the runtime form. Fails with
/// [`StorageError::Conversion`] if the document cannot be resolved (e.g. it uses
/// unresolved variable references in coordinates).
pub fn load_profile(name: &str) -> StorageResult<Profile> {
    let doc = load_document(name)?;
    Profile::try_from(&doc).map_err(|e| StorageError::Conversion(e.to_string()))
}

/// Delete a profile from disk (both canonical and legacy files).
pub fn delete_profile(name: &str) -> StorageResult<()> {
    delete_profile_in(&get_profiles_dir(), name)
}

fn delete_profile_in(dir: &Path, name: &str) -> StorageResult<()> {
    let filename = sanitize_filename(name);
    let mut removed = false;

    for ext in [DOC_EXT, LEGACY_EXT] {
        let path = dir.join(format!("{}.{}", filename, ext));
        if path.exists() {
            fs::remove_file(&path)?;
            removed = true;
        }
    }

    if !removed {
        return Err(StorageError::NotFound(name.to_string()));
    }

    info!(%name, "Deleted profile");
    Ok(())
}

/// List all saved profiles (canonical + legacy, deduplicated by name).
pub fn list_profiles() -> StorageResult<Vec<String>> {
    list_profiles_in(&get_profiles_dir())
}

fn list_profiles_in(dir: &Path) -> StorageResult<Vec<String>> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    // BTreeSet dedups names shared between a YAML and a legacy JSON file and
    // yields a stable, sorted ordering.
    let mut names = std::collections::BTreeSet::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let is_profile = path
            .extension()
            .map(|e| e == DOC_EXT || e == LEGACY_EXT)
            .unwrap_or(false);
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
    fn test_load_legacy_json_profile() {
        let dir = temp_dir("legacy");

        // Simulate an old install: a runtime Profile serialized as JSON.
        let profile = Profile::default();
        let json = serde_json::to_string_pretty(&profile).unwrap();
        let legacy_path = dir.join(format!(
            "{}.{}",
            sanitize_filename(&profile.name),
            LEGACY_EXT
        ));
        fs::write(&legacy_path, json).unwrap();

        let doc = load_document_from(&dir, &profile.name).unwrap();
        assert_eq!(doc.name, profile.name);
        assert_eq!(doc.timeline.len(), profile.timeline.actions.len());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_save_migrates_legacy_json() {
        let dir = temp_dir("migrate");

        let profile = Profile::default();
        let stem = sanitize_filename(&profile.name);
        let legacy_path = dir.join(format!("{}.{}", stem, LEGACY_EXT));
        fs::write(&legacy_path, serde_json::to_string(&profile).unwrap()).unwrap();
        assert!(legacy_path.exists());

        // Saving the same name as a document should remove the legacy file.
        save_document_to(&dir, &MacroDocument::from(&profile)).unwrap();
        assert!(!legacy_path.exists(), "legacy JSON should be migrated away");
        assert!(dir.join(format!("{}.{}", stem, DOC_EXT)).exists());

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_list_profiles_dedups_yaml_and_json() {
        let dir = temp_dir("list");

        // "Shared" exists as both YAML and JSON; "OnlyYaml" only as YAML.
        save_document_to(
            &dir,
            &MacroDocument {
                name: "Shared".to_string(),
                ..rich_document()
            },
        )
        .unwrap();
        fs::write(dir.join("Shared.json"), "{}").unwrap();
        save_document_to(
            &dir,
            &MacroDocument {
                name: "OnlyYaml".to_string(),
                ..rich_document()
            },
        )
        .unwrap();

        let names = list_profiles_in(&dir).unwrap();
        assert_eq!(names, vec!["OnlyYaml".to_string(), "Shared".to_string()]);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_delete_removes_both_formats() {
        let dir = temp_dir("delete");
        let stem = "Both";
        fs::write(
            dir.join(format!("{stem}.{DOC_EXT}")),
            "name: Both\ntimeline: []\n",
        )
        .unwrap();
        fs::write(dir.join(format!("{stem}.{LEGACY_EXT}")), "{}").unwrap();

        delete_profile_in(&dir, stem).unwrap();
        assert!(!dir.join(format!("{stem}.{DOC_EXT}")).exists());
        assert!(!dir.join(format!("{stem}.{LEGACY_EXT}")).exists());

        // Deleting a missing profile reports NotFound.
        assert!(matches!(
            delete_profile_in(&dir, "missing"),
            Err(StorageError::NotFound(_))
        ));

        fs::remove_dir_all(&dir).ok();
    }
}
