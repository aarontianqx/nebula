mod adapter;
mod application;
mod domain;
mod infrastructure;

use adapter::tauri::commands;
use adapter::tauri::state::AppState;
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

    // Create application state
    let state = AppState::new(db);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_accounts,
            commands::create_account,
            commands::update_account,
            commands::delete_account,
            commands::get_groups,
            commands::create_group,
            commands::update_group,
            commands::delete_group,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

