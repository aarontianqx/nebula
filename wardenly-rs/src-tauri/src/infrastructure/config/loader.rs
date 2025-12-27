use serde::de::DeserializeOwned;
use std::path::Path;

/// Load a YAML configuration file
pub fn load_yaml<T: DeserializeOwned>(path: impl AsRef<Path>) -> anyhow::Result<T> {
    let content = std::fs::read_to_string(path)?;
    let config: T = serde_yaml::from_str(&content)?;
    Ok(config)
}

/// Load configuration from resources/configs/ directory
pub fn load_config<T: DeserializeOwned + Default>(name: &str) -> T {
    let embedded_path = format!("resources/configs/{}.yaml", name);

    if let Ok(config) = load_yaml::<T>(&embedded_path) {
        tracing::debug!("Loaded config from {}", embedded_path);
        return config;
    }

    // Fallback to defaults
    tracing::warn!("Config file {} not found, using defaults", name);
    T::default()
}

