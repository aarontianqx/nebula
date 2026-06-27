//! Pulsar GUI 适配层入口（Tauri 后端）。

mod commands;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let state = AppState {
        registry: pulsar_app::build_registry(),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::list_tools,
            commands::run_tool,
            commands::search_tools,
            commands::detect,
        ])
        .run(tauri::generate_context!())
        .expect("error while running pulsar");
}
