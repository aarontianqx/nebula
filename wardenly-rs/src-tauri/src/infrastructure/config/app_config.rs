use serde::{Deserialize, Serialize};
use super::paths;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub storage: StorageConfig,
}

/// Storage type selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    #[default]
    Sqlite,
    Mongodb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageConfig {
    /// Storage backend type: "sqlite" (default) or "mongodb"
    #[serde(rename = "type")]
    pub storage_type: StorageType,
    pub sqlite: SqliteConfig,
    pub mongodb: MongoDbConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SqliteConfig {
    /// Leave empty to use platform default path
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoDbConfig {
    pub uri: String,
    pub database: String,
}

impl Default for MongoDbConfig {
    fn default() -> Self {
        Self {
            uri: "mongodb://localhost:27017".to_string(),
            database: "wardenly".to_string(),
        }
    }
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
            storage_type: StorageType::default(),
            sqlite: SqliteConfig::default(),
            mongodb: MongoDbConfig::default(),
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

