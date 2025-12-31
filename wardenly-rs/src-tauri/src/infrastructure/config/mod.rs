mod keyboard_config;
pub mod loader;
pub mod paths;
pub mod resources;
pub mod settings;
mod theme_config;

pub use keyboard_config::*;
pub use settings::*;
pub use theme_config::*;

use std::sync::OnceLock;

static KEYBOARD_CONFIG: OnceLock<KeyboardConfig> = OnceLock::new();
static THEME_CONFIG: OnceLock<ThemeConfig> = OnceLock::new();

/// Initialize configuration system (called at app startup)
pub fn init() {
    KEYBOARD_CONFIG.get_or_init(|| loader::load_embedded_config("keyboard"));
    THEME_CONFIG.get_or_init(|| loader::load_embedded_config("themes"));
    tracing::info!("Configuration initialized");
}

/// Get keyboard configuration (embedded)
pub fn keyboard() -> &'static KeyboardConfig {
    KEYBOARD_CONFIG.get().expect("Config not initialized")
}

/// Get theme configuration (embedded official presets)
pub fn themes() -> &'static ThemeConfig {
    THEME_CONFIG.get().expect("Config not initialized")
}

/// Get user settings (loaded from user config directory)
pub fn user_settings() -> UserSettings {
    loader::load_user_settings()
}

