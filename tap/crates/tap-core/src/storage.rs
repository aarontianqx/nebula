//! Profile storage and persistence.

use crate::Profile;
use std::fs;
use std::path::PathBuf;
use thiserror::Error;
use tracing::{debug, info};

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
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

/// Save a profile to disk.
pub fn save_profile(profile: &Profile) -> StorageResult<PathBuf> {
    let dir = ensure_profiles_dir()?;
    let filename = sanitize_filename(&profile.name);
    let path = dir.join(format!("{}.json", filename));

    let json = serde_json::to_string_pretty(profile)?;
    fs::write(&path, json)?;

    info!(?path, "Saved profile");
    Ok(path)
}

/// Load a profile from disk by name.
pub fn load_profile(name: &str) -> StorageResult<Profile> {
    let dir = get_profiles_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.json", filename));

    if !path.exists() {
        return Err(StorageError::NotFound(name.to_string()));
    }

    let json = fs::read_to_string(&path)?;
    let profile: Profile = serde_json::from_str(&json)?;

    debug!(?path, "Loaded profile");
    Ok(profile)
}

/// Delete a profile from disk.
pub fn delete_profile(name: &str) -> StorageResult<()> {
    let dir = get_profiles_dir();
    let filename = sanitize_filename(name);
    let path = dir.join(format!("{}.json", filename));

    if !path.exists() {
        return Err(StorageError::NotFound(name.to_string()));
    }

    fs::remove_file(&path)?;
    info!(?path, "Deleted profile");
    Ok(())
}

/// List all saved profiles.
pub fn list_profiles() -> StorageResult<Vec<String>> {
    let dir = get_profiles_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut profiles = Vec::new();
    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "json").unwrap_or(false) {
            if let Some(name) = path.file_stem() {
                profiles.push(name.to_string_lossy().to_string());
            }
        }
    }

    profiles.sort();
    Ok(profiles)
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

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("My Profile"), "My Profile");
        assert_eq!(sanitize_filename("test/profile"), "test_profile");
        assert_eq!(sanitize_filename("a:b*c?d"), "a_b_c_d");
    }
}

