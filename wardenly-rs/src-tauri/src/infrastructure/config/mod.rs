mod gesture_config;
pub mod loader;
pub mod paths;
pub mod resources;
pub mod settings;
mod theme_config;

pub use gesture_config::*;
pub use settings::*;
pub use theme_config::*;

use std::sync::OnceLock;

static GESTURE_CONFIG: OnceLock<GestureConfig> = OnceLock::new();
static THEME_CONFIG: OnceLock<ThemeConfig> = OnceLock::new();

/// Initialize configuration system (called at app startup)
pub fn init() {
    GESTURE_CONFIG.get_or_init(|| loader::load_embedded_config("gesture"));
    THEME_CONFIG.get_or_init(|| loader::load_embedded_config("themes"));
    tracing::info!("Configuration initialized");
}

/// Get gesture configuration (embedded)
pub fn gesture() -> &'static GestureConfig {
    GESTURE_CONFIG.get().expect("Config not initialized")
}

/// Get theme configuration (embedded official presets)
pub fn themes() -> &'static ThemeConfig {
    THEME_CONFIG.get().expect("Config not initialized")
}

/// Get user settings (loaded from user config directory)
pub fn user_settings() -> UserSettings {
    loader::load_user_settings()
}

