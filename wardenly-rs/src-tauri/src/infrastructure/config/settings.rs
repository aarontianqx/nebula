use serde::{Deserialize, Serialize};

/// User settings stored in settings.yaml in user config directory.
/// All fields are optional - missing values use defaults.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UserSettings {
    /// Selected theme name (must match a key in embedded themes.yaml)
    /// If invalid or missing, uses defaultTheme from themes.yaml
    pub theme: Option<String>,

    /// Storage configuration
    pub storage: StorageSettings,

    /// Keyboard passthrough configuration (optional override)
    /// Values here override embedded keyboard.yaml defaults
    pub keyboard: Option<KeyboardOverride>,
}

/// Optional keyboard passthrough config overrides
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct KeyboardOverride {
    /// Long press detection threshold in milliseconds
    pub long_press_threshold_ms: Option<u64>,
    /// Long press repeat click interval in milliseconds
    pub repeat_interval_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct StorageSettings {
    /// Storage backend: "sqlite" (default) or "mongodb"
    #[serde(rename = "type")]
    pub storage_type: StorageType,

    /// MongoDB connection settings (only used when storage_type is mongodb)
    pub mongodb: MongoDbSettings,
}

impl Default for StorageSettings {
    fn default() -> Self {
        Self {
            storage_type: StorageType::Sqlite,
            mongodb: MongoDbSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum StorageType {
    #[default]
    Sqlite,
    Mongodb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MongoDbSettings {
    pub uri: String,
    pub database: String,
}

impl Default for MongoDbSettings {
    fn default() -> Self {
        Self {
            uri: "mongodb://localhost:27017".to_string(),
            database: "wardenly".to_string(),
        }
    }
}

/// Response sent to frontend with current settings and available options
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsResponse {
    /// Current user settings
    pub settings: UserSettings,
    /// Available theme names from embedded themes.yaml
    pub available_themes: Vec<String>,
    /// Default theme name from embedded themes.yaml
    pub default_theme: String,
}
