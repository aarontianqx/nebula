#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tap_core::Profile;

#[tauri::command]
fn get_default_profile() -> Profile {
    Profile::default()
}

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tap_tauri=info,tauri=info".into()),
        )
        .try_init();
}

fn main() {
    init_logging();

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![get_default_profile])
        .run(tauri::generate_context!())
        .expect("error while running tap");
}


