use serde::{Deserialize, Serialize};
use super::paths;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub storage: StorageConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    pub sqlite: SqliteConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SqliteConfig {
    /// Leave empty to use platform default path
    pub path: String,
}

impl SqliteConfig {
    pub fn effective_path(&self) -> PathBuf {
        if self.path.is_empty() {
            paths::default_sqlite_path()
        } else {
            PathBuf::from(&self.path)
        }
    }
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            sqlite: SqliteConfig::default(),
        }
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            storage: StorageConfig::default(),
        }
    }
}

