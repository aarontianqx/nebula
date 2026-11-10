#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod events;
mod key_click;
mod picker;
mod ports;
mod state;
mod templates;

use tap_platform::set_dpi_aware;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{error, info};

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                "tap_tauri=debug,tap_core=debug,tap_platform=debug,tauri=info".into()
            }),
        )
        .try_init();
}

fn main() {
    // Set DPI awareness before anything else (Windows)
    set_dpi_aware();

    init_logging();

    // Define emergency stop shortcut: Ctrl+Shift+Backspace
    let emergency_shortcut =
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backspace);

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &emergency_shortcut && event.state == ShortcutState::Pressed {
                        app::handle_emergency_stop(app);
                    }
                })
                .build(),
        )
        .manage(picker::PickerState::default())
        .setup(move |app| {
            // Register the shortcut
            if let Err(e) = app.global_shortcut().register(emergency_shortcut) {
                error!("Failed to register emergency shortcut: {:?}", e);
            } else {
                info!("Emergency stop shortcut registered: Ctrl+Shift+Backspace");
            }

            app::setup_app(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Engine lifecycle
            commands::engine::get_state,
            commands::engine::start_execution,
            commands::engine::pause_execution,
            commands::engine::resume_execution,
            commands::engine::stop_execution,
            commands::engine::emergency_stop,
            commands::engine::cmd_set_dry_run,
            commands::engine::cmd_get_dry_run,
            // Document: edit, persistence, metadata, templates, file import / export
            commands::document::get_document,
            commands::document::update_document,
            commands::document::set_simple_repeat,
            commands::document::cmd_save_profile,
            commands::document::cmd_load_profile,
            commands::document::cmd_delete_profile,
            commands::document::cmd_list_profiles,
            commands::document::cmd_get_last_used,
            commands::document::cmd_get_recent_profiles,
            commands::document::cmd_list_templates,
            commands::document::cmd_apply_template,
            commands::document::cmd_export_yaml_to_path,
            commands::document::cmd_import_yaml_from_path,
            // YAML / DSL
            commands::yaml::cmd_export_yaml,
            commands::yaml::cmd_export_yaml_with_metadata,
            commands::yaml::cmd_import_yaml,
            commands::yaml::cmd_validate_yaml,
            commands::yaml::cmd_set_runtime_variables,
            commands::yaml::cmd_get_runtime_variables,
            // Recording
            commands::recording::start_recording,
            commands::recording::pause_recording,
            commands::recording::resume_recording,
            commands::recording::stop_recording,
            commands::recording::get_recording_status,
            // Key->Click tool mode + key capture
            commands::key_click::start_key_click,
            commands::key_click::stop_key_click,
            commands::key_click::get_key_click_status,
            commands::key_click::cmd_capture_key,
            // System: permissions, mouse tracking, windows, pixels
            commands::system::cmd_check_permissions,
            commands::system::cmd_request_screen_recording,
            commands::system::cmd_open_permission_settings,
            commands::system::start_mouse_tracking,
            commands::system::stop_mouse_tracking,
            commands::system::cmd_list_windows,
            commands::system::cmd_get_foreground_window,
            commands::system::cmd_get_pixel_color,
            commands::system::cmd_check_window_focused,
            commands::system::cmd_check_window_exists,
            // Position picker
            picker::open_picker_window,
            picker::close_picker_window,
            picker::picker_position_selected,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tap");
}
