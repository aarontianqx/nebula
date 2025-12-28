mod adapter;
mod application;
mod domain;
mod infrastructure;

use std::sync::Arc;

use adapter::tauri::state::{AppState, DynAccountRepository, DynGroupRepository};
use adapter::tauri::{commands, events};
use application::eventbus::create_event_bus;
use domain::repository::AccountRepository;
use infrastructure::config::{self, StorageType};
use infrastructure::logging;
use infrastructure::persistence;

/// Storage backend holder for runtime switching
struct StorageBackend {
    account_repo: DynAccountRepository,
    group_repo: DynGroupRepository,
    coordinator_account_repo: Arc<dyn AccountRepository>,
}

/// Initialize storage based on configuration
fn init_storage() -> StorageBackend {
    let app_config = config::app();

    match app_config.storage.storage_type {
        StorageType::Sqlite => {
            tracing::info!("Using SQLite storage backend");
            let db = persistence::sqlite::init_database()
                .expect("Failed to initialize SQLite database");

            use persistence::sqlite::{SqliteAccountRepository, SqliteGroupRepository};

            let account_repo = Box::new(SqliteAccountRepository::new(db.clone()));
            let group_repo = Box::new(SqliteGroupRepository::new(db.clone()));
            let coordinator_account_repo: Arc<dyn AccountRepository> = Arc::new(SqliteAccountRepository::new(db));

            StorageBackend {
                account_repo,
                group_repo,
                coordinator_account_repo,
            }
        }
        StorageType::Mongodb => {
            tracing::info!("Using MongoDB storage backend");

            // MongoDB requires async initialization
            let mongo_config = &app_config.storage.mongodb;
            let conn = tauri::async_runtime::block_on(async {
                persistence::mongodb::init_mongodb(&mongo_config.uri, &mongo_config.database)
                    .await
                    .expect("Failed to initialize MongoDB connection")
            });

            use persistence::mongodb::{MongoAccountRepository, MongoGroupRepository};

            let account_repo = Box::new(MongoAccountRepository::new(conn.clone()));
            let group_repo = Box::new(MongoGroupRepository::new(conn.clone()));
            let coordinator_account_repo: Arc<dyn AccountRepository> = Arc::new(MongoAccountRepository::new(conn));

            StorageBackend {
                account_repo,
                group_repo,
                coordinator_account_repo,
            }
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    logging::setup(false);

    // Initialize configuration
    config::init();

    // Initialize storage based on config
    let storage = init_storage();

    // Create event bus
    let event_bus = create_event_bus();

    // Create application state
    let state = AppState::new(
        storage.account_repo,
        storage.group_repo,
        storage.coordinator_account_repo,
        event_bus.clone(),
    );

    // Get references before moving state
    let input_processor = state.input_processor.clone();
    let click_rx = state.click_rx.clone();
    let coordinator = state.coordinator.clone();

    // Start coordinator event listener for auto-cleanup of stopped sessions
    coordinator.start_event_listener();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(state)
        .setup(move |app| {
            // Start event forwarder to push events to frontend
            events::start_event_forwarder(app.handle().clone(), event_bus);

            // Start input processing
            let input_proc = input_processor.clone();
            tauri::async_runtime::spawn(async move {
                input_proc.start_processing().await;
            });

            // Start click event forwarder from keyboard passthrough to coordinator
            let coord = coordinator.clone();
            tauri::async_runtime::spawn(async move {
                let mut rx = click_rx.lock().await;
                while let Some(click_event) = rx.recv().await {
                    if let Err(e) = coord
                        .click_session(&click_event.session_id, click_event.x, click_event.y)
                        .await
                    {
                        tracing::warn!("Failed to forward keyboard click: {}", e);
                    }
                }
            });

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
            commands::refresh_session,
            commands::start_screencast,
            commands::stop_screencast,
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
