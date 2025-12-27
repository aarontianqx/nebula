mod adapter;
mod application;
mod domain;
mod infrastructure;

use adapter::tauri::{commands, events};
use adapter::tauri::state::AppState;
use application::eventbus::create_event_bus;
use infrastructure::config;
use infrastructure::logging;
use infrastructure::persistence;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    logging::setup(false);

    // Initialize configuration
    config::init();

    // Initialize database
    let db = persistence::sqlite::init_database()
        .expect("Failed to initialize database");

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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
