use std::path::PathBuf;

/// Get platform-specific configuration directory
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wardenly")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/wardenly")
    }

    #[cfg(target_os = "linux")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wardenly")
    }
}

pub fn default_sqlite_path() -> PathBuf {
    config_dir().join("data.db")
}

/// Get platform-specific log directory
/// - macOS: ~/Library/Application Support/wardenly/logs/
/// - Windows: %APPDATA%\wardenly\logs\
/// - Linux: ~/.config/wardenly/logs/
pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}

/// Get platform-specific browser profile directory for an account.
/// Uses data_dir for persistence across app updates.
/// - macOS: ~/Library/Application Support/wardenly/profiles/{account_id}
/// - Windows: %APPDATA%\wardenly\profiles\{account_id}
/// - Linux: ~/.local/share/wardenly/profiles/{account_id}
pub fn profile_dir(account_id: &str) -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("wardenly")
        .join("profiles")
        .join(account_id)
}

/// Delete the browser profile directory for an account.
/// Called when an account is deleted to free disk space.
pub fn delete_profile(account_id: &str) {
    let path = profile_dir(account_id);
    if path.exists() {
        match std::fs::remove_dir_all(&path) {
            Ok(_) => tracing::info!(
                "Deleted browser profile for account {}: {:?}",
                account_id,
                path
            ),
            Err(e) => tracing::warn!(
                "Failed to delete browser profile for account {}: {}",
                account_id,
                e
            ),
        }
    }
}

/// Get the root profiles directory.
pub fn profiles_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("wardenly")
        .join("profiles")
}

/// Get the size of a specific account's profile directory in bytes.
pub fn get_profile_size(account_id: &str) -> u64 {
    let path = profile_dir(account_id);
    dir_size(&path)
}

/// Get the total size of all profile directories in bytes.
pub fn get_all_profiles_size() -> u64 {
    let path = profiles_dir();
    dir_size(&path)
}

/// Recursively calculate directory size.
fn dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }

    let mut size = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                size += dir_size(&entry_path);
            } else if let Ok(metadata) = entry.metadata() {
                size += metadata.len();
            }
        }
    }
    size
}

/// Clear all browser profile directories.
/// Returns the number of profiles deleted.
pub fn clear_all_profiles() -> usize {
    let path = profiles_dir();
    if !path.exists() {
        return 0;
    }

    let mut count = 0;
    if let Ok(entries) = std::fs::read_dir(&path) {
        for entry in entries.flatten() {
            let entry_path = entry.path();
            if entry_path.is_dir() {
                if let Err(e) = std::fs::remove_dir_all(&entry_path) {
                    tracing::warn!("Failed to delete profile {:?}: {}", entry_path, e);
                } else {
                    count += 1;
                }
            }
        }
    }

    tracing::info!("Cleared {} browser profiles", count);
    count
}
