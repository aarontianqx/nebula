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

/// Result of storage initialization
struct StorageInitResult {
    storage: StorageBackend,
    /// If MongoDB connection failed, contains the error message for user notification
    fallback_warning: Option<String>,
}

/// Initialize SQLite storage (always succeeds or fatally fails)
fn init_sqlite_storage() -> Result<StorageBackend, String> {
    tracing::info!("Using SQLite storage backend");
    let db = persistence::sqlite::init_database()
        .map_err(|e| format!("Failed to initialize SQLite database:\n\n{}", e))?;

    use persistence::sqlite::{SqliteAccountRepository, SqliteGroupRepository};

    let account_repo = Box::new(SqliteAccountRepository::new(db.clone()));
    let group_repo = Box::new(SqliteGroupRepository::new(db.clone()));
    let coordinator_account_repo: Arc<dyn AccountRepository> = Arc::new(SqliteAccountRepository::new(db));

    Ok(StorageBackend {
        account_repo,
        group_repo,
        coordinator_account_repo,
    })
}

/// Initialize storage based on user settings.
/// If MongoDB is configured but connection fails, falls back to SQLite with a warning.
fn init_storage() -> Result<StorageInitResult, String> {
    let settings = config::user_settings();

    match settings.storage.storage_type {
        StorageType::Sqlite => {
            let storage = init_sqlite_storage()?;
            Ok(StorageInitResult {
                storage,
                fallback_warning: None,
            })
        }
        StorageType::Mongodb => {
            tracing::info!("Attempting to connect to MongoDB...");

            // Get Tauri's async runtime handle and extract the underlying tokio Handle.
            // This ensures we use the same runtime for all MongoDB operations,
            // avoiding deadlocks that occur when calling block_on from different runtimes.
            let tauri_handle = tauri::async_runtime::handle();
            let runtime = tauri_handle.inner().clone();

            // Try to connect to MongoDB
            let mongo_config = &settings.storage.mongodb;
            let uri = mongo_config.uri.clone();
            let db = mongo_config.database.clone();

            let mongo_result = tauri_handle.block_on(async move {
                persistence::mongodb::init_mongodb(&uri, &db).await
            });

            match mongo_result {
                Ok(conn) => {
                    tracing::info!("MongoDB connection successful");
                    use persistence::mongodb::{MongoAccountRepository, MongoGroupRepository};

                    let account_repo = Box::new(MongoAccountRepository::new(conn.clone(), runtime.clone()));
                    let group_repo = Box::new(MongoGroupRepository::new(conn.clone(), runtime.clone()));
                    let coordinator_account_repo: Arc<dyn AccountRepository> = 
                        Arc::new(MongoAccountRepository::new(conn, runtime));

                    Ok(StorageInitResult {
                        storage: StorageBackend {
                            account_repo,
                            group_repo,
                            coordinator_account_repo,
                        },
                        fallback_warning: None,
                    })
                }
                Err(e) => {
                    // MongoDB connection failed - fallback to SQLite
                    let warning = format!(
                        "MongoDB connection failed, using local SQLite storage.\n\n\
                         Configured URI: {}\n\
                         Database: {}\n\n\
                         Error: {}\n\n\
                         Your data will be stored locally. \
                         Please check your MongoDB configuration in Settings.",
                        mongo_config.uri, mongo_config.database, e
                    );
                    tracing::warn!("{}", warning);

                    let storage = init_sqlite_storage()?;
                    Ok(StorageInitResult {
                        storage,
                        fallback_warning: Some(warning),
                    })
                }
            }
        }
    }
}

/// Show an error dialog and exit the application
fn show_error_and_exit(title: &str, message: &str) -> ! {
    tracing::error!("{}: {}", title, message);

    // Use rfd for native dialogs - it works without Tauri app handle
    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Error)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();

    std::process::exit(1);
}

/// Show a warning dialog (non-blocking notification)
fn show_warning_dialog(title: &str, message: &str) {
    tracing::warn!("{}: {}", title, message);

    rfd::MessageDialog::new()
        .set_title(title)
        .set_description(message)
        .set_level(rfd::MessageLevel::Warning)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging
    logging::setup(false);

    // Initialize embedded configuration (themes, gestures)
    config::init();

    // Initialize storage based on user settings
    // If MongoDB fails, will fallback to SQLite with a warning
    let init_result = match init_storage() {
        Ok(r) => r,
        Err(e) => show_error_and_exit("Storage Initialization Error", &e),
    };

    // Show fallback warning if MongoDB connection failed
    if let Some(warning) = &init_result.fallback_warning {
        show_warning_dialog("Storage Connection Warning", warning);
    }

    let storage = init_result.storage;

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
    let coordinator = state.coordinator.clone();

    // Start coordinator event listener for auto-cleanup of stopped sessions
    coordinator.start_event_listener();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
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
            commands::run_group,
            // Session commands
            commands::get_sessions,
            commands::start_session,
            commands::stop_session,
            commands::stop_all_sessions,
            commands::click_session,
            commands::drag_session,
            commands::click_all_sessions,
            commands::drag_all_sessions,
            commands::refresh_session,
            commands::start_screencast,
            commands::stop_screencast,
            commands::capture_screenshot,
            // Script commands
            commands::get_scripts,
            commands::start_script,
            commands::stop_script,
            commands::start_all_scripts,
            commands::stop_all_scripts,
            // Input commands
            commands::set_keyboard_passthrough,
            commands::get_keyboard_passthrough_status,
            // Settings & Theme commands
            commands::get_settings,
            commands::save_settings,
            commands::test_mongodb_connection,
            commands::get_theme_config,
            commands::get_keyboard_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}


