mod app_config;
mod loader;
mod paths;

pub use app_config::*;

use std::sync::OnceLock;

static APP_CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Initialize configuration system (called at app startup)
pub fn init() {
    APP_CONFIG.get_or_init(|| loader::load_config("app"));
    tracing::info!("Configuration initialized");
}

/// Get application configuration
pub fn app() -> &'static AppConfig {
    APP_CONFIG.get().expect("Config not initialized")
}

