mod adapter;
mod application;
mod domain;
mod infrastructure;

use std::sync::Arc;

use adapter::tauri::{commands, events};
use adapter::tauri::state::AppState;
use application::eventbus::create_event_bus;
use infrastructure::config;
use infrastructure::logging;
use infrastructure::persistence;
use rusqlite::Connection;

/// Initialize storage based on configuration
fn init_storage() -> Arc<std::sync::Mutex<Connection>> {
    #[cfg(feature = "mongodb")]
    {
        use config::StorageType;
        let app_config = config::app();
        if app_config.storage.storage_type == StorageType::Mongodb {
            // MongoDB initialization would go here
            // For now, fall back to SQLite as MongoDB support is optional
            tracing::warn!("MongoDB storage selected but not fully implemented, falling back to SQLite");
        }
    }

    // Default: SQLite
    persistence::sqlite::init_database()
        .expect("Failed to initialize SQLite database")
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    logging::setup(false);

    // Initialize configuration
    config::init();

    // Initialize database based on config
    let db = init_storage();

    // Create event bus
    let event_bus = create_event_bus();

    // Create application state
    let state = AppState::new(db, event_bus.clone());

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(move |app| {
            // Start event forwarder to push events to frontend
            events::start_event_forwarder(app.handle().clone(), event_bus);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Account commands
            commands::get_accounts,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            // Group commands
            commands::get_groups,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
            // Session commands
            commands::get_sessions,
            commands::start_session,
            commands::stop_session,
            commands::stop_all_sessions,
            commands::click_session,
            commands::drag_session,
            commands::click_all_sessions,
            // Script commands
            commands::get_scripts,
            commands::start_script,
            commands::stop_script,
            commands::start_all_scripts,
            commands::stop_all_scripts,
            // Input commands
            commands::set_keyboard_passthrough,
            commands::get_keyboard_passthrough_status,
            commands::update_cursor_position,
            commands::set_active_session_for_input,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
