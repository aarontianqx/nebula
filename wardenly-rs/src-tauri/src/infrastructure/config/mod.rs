mod app_config;
mod gesture_config;
pub mod loader;
mod paths;
pub mod resources;
mod theme_config;

pub use app_config::*;
pub use gesture_config::*;
pub use theme_config::*;

use std::sync::OnceLock;

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();
static GESTURE_CONFIG: OnceLock<GestureConfig> = OnceLock::new();

/// Initialize configuration system (called at app startup)
pub fn init() {
    APP_CONFIG.get_or_init(|| loader::load_config("app"));
    GESTURE_CONFIG.get_or_init(|| loader::load_config("gesture"));
    tracing::info!("Configuration initialized");
}

/// Get application configuration
pub fn app() -> &'static AppConfig {
    APP_CONFIG.get().expect("Config not initialized")
}

/// Get gesture configuration
pub fn gesture() -> &'static GestureConfig {
    GESTURE_CONFIG.get().expect("Config not initialized")
}

