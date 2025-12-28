use include_dir::{include_dir, Dir};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;

use super::paths::config_dir;
use super::settings::UserSettings;

// Embed the entire configs directory at compile time
static CONFIGS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/resources/configs");

/// Load a YAML configuration file from disk
pub fn load_yaml<T: DeserializeOwned>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let config: T = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Parse YAML from string
pub fn parse_yaml<T: DeserializeOwned>(content: &str) -> anyhow::Result<T> {
    let config: T = serde_yaml::from_str(content)?;
    Ok(config)
}

/// Save a configuration to a YAML file
pub fn save_yaml<T: Serialize>(path: impl AsRef<Path>, config: &T) -> anyhow::Result<()> {
    let content = serde_yaml::to_string(config)?;
    std::fs::write(path, content)?;
    Ok(())
}

/// Load embedded configuration by name from the configs directory
/// Files are embedded at compile time using include_dir
pub fn load_embedded_config<T: DeserializeOwned + Default>(name: &str) -> T {
    let file_name = format!("{}.yaml", name);
    
    match CONFIGS_DIR.get_file(&file_name) {
        Some(file) => {
            match file.contents_utf8() {
                Some(content) => {
                    match parse_yaml::<T>(content) {
                        Ok(config) => {
                            tracing::debug!("Loaded embedded config: {}", name);
                            config
                        }
                        Err(e) => {
                            tracing::error!("Failed to parse embedded config {}: {}", name, e);
                            T::default()
                        }
                    }
                }
                None => {
                    tracing::error!("Embedded config {} is not valid UTF-8", name);
                    T::default()
                }
            }
        }
        None => {
            tracing::warn!("Embedded config {} not found, using defaults", name);
            T::default()
        }
    }
}

/// Load user settings from settings.yaml in user config directory
/// Returns default settings if file doesn't exist or is invalid
pub fn load_user_settings() -> UserSettings {
    let settings_path = config_dir().join("settings.yaml");

    if settings_path.exists() {
        match load_yaml::<UserSettings>(&settings_path) {
            Ok(settings) => {
                tracing::info!("Loaded user settings from {:?}", settings_path);
                return settings;
            }
            Err(e) => {
                tracing::warn!("Failed to parse settings.yaml: {}, using defaults", e);
            }
        }
    } else {
        tracing::debug!("No settings.yaml found, using defaults");
    }

    UserSettings::default()
}

/// Save user settings to settings.yaml in user config directory
pub fn save_user_settings(settings: &UserSettings) -> anyhow::Result<()> {
    ensure_config_dir()?;
    let settings_path = config_dir().join("settings.yaml");
    save_yaml(&settings_path, settings)?;
    tracing::info!("Saved user settings to {:?}", settings_path);
    Ok(())
}

/// Ensure user config directory exists
pub fn ensure_config_dir() -> std::io::Result<()> {
    let dir = config_dir();
    if !dir.exists() {
        std::fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Get the path to user settings file (for display purposes)
pub fn settings_file_path() -> std::path::PathBuf {
    config_dir().join("settings.yaml")
}
