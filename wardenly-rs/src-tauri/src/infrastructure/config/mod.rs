mod app_config;
mod gesture_config;
mod loader;
mod paths;
pub mod resources;

pub use app_config::*;
pub use gesture_config::*;

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

