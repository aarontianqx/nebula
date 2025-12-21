#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod state;

use state::AppState;
use std::sync::Mutex;
use tap_core::{
    delete_profile, list_profiles, load_last_used, load_profile, save_last_used, save_profile,
    Action, EngineCommand, EngineEvent, EngineState, InjectorExecutor, Player, Profile, Repeat,
    RunConfig, Timeline, TimedAction,
};
use tap_platform::EnigoInjector;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use tracing::{debug, error, info, warn};

// === Tauri Commands ===

#[tauri::command]
fn get_default_profile() -> Profile {
    Profile::default()
}

#[tauri::command]
fn get_state(state: State<'_, Mutex<AppState>>) -> EngineState {
    state.lock().unwrap().engine_state
}

#[tauri::command]
fn start_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Idle {
        return Err("Cannot start: not in idle state".into());
    }

    // Send the current profile to the player
    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::SetProfile(app_state.profile.clone()));
        handle.send(EngineCommand::Start);
        info!("Sent start command to player");
    }

    Ok(())
}

#[tauri::command]
fn pause_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Running {
        return Err("Cannot pause: not running".into());
    }

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Pause);
    }

    Ok(())
}

#[tauri::command]
fn resume_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if app_state.engine_state != EngineState::Paused {
        return Err("Cannot resume: not paused".into());
    }

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Resume);
    }

    Ok(())
}

#[tauri::command]
fn stop_execution(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::Stop);
    }

    Ok(())
}

#[tauri::command]
fn emergency_stop(state: State<'_, Mutex<AppState>>) -> Result<(), String> {
    let app_state = state.lock().unwrap();

    if let Some(ref handle) = app_state.player_handle {
        handle.send(EngineCommand::EmergencyStop);
        warn!("Emergency stop triggered!");
    }

    Ok(())
}

#[tauri::command]
fn update_profile(state: State<'_, Mutex<AppState>>, profile: Profile) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();
    app_state.profile = profile;
    Ok(())
}

#[tauri::command]
fn set_simple_repeat(
    state: State<'_, Mutex<AppState>>,
    action_type: String,
    x: Option<i32>,
    y: Option<i32>,
    key: Option<String>,
    interval_ms: u64,
    repeat_count: Option<u32>,
    countdown_secs: u32,
) -> Result<(), String> {
    let mut app_state = state.lock().unwrap();

    let action = match action_type.as_str() {
        "click" => Action::Click {
            x: x.unwrap_or(0),
            y: y.unwrap_or(0),
            button: tap_core::MouseButton::Left,
        },
        "key" => Action::KeyTap {
            key: key.unwrap_or_else(|| "Space".into()),
        },
        _ => return Err(format!("Unknown action type: {}", action_type)),
    };

    // Simple repeat: one action followed by a wait
    // The wait is the interval between iterations
    let timeline = Timeline {
        actions: vec![
            TimedAction::after_ms(0, action),
            TimedAction::after_ms(0, Action::Wait { ms: interval_ms }),
        ],
    };

    let repeat = match repeat_count {
        Some(n) => Repeat::Times(n),
        None => Repeat::Forever,
    };

    app_state.profile = Profile {
        name: "Simple Repeat".into(),
        timeline,
        run: RunConfig {
            start_delay_ms: countdown_secs as u64 * 1000,
            speed: 1.0,
            repeat,
        },
    };

    info!(?app_state.profile, "Updated profile for simple repeat");

    Ok(())
}

// === Profile Persistence Commands ===

#[tauri::command]
fn cmd_save_profile(state: State<'_, Mutex<AppState>>, name: Option<String>) -> Result<String, String> {
    let mut app_state = state.lock().unwrap();
    
    if let Some(n) = name {
        app_state.profile.name = n;
    }
    
    let path = save_profile(&app_state.profile).map_err(|e| e.to_string())?;
    let _ = save_last_used(&app_state.profile.name);
    
    Ok(path.to_string_lossy().to_string())
}

#[tauri::command]
fn cmd_load_profile(state: State<'_, Mutex<AppState>>, name: String) -> Result<Profile, String> {
    let profile = load_profile(&name).map_err(|e| e.to_string())?;
    
    let mut app_state = state.lock().unwrap();
    app_state.profile = profile.clone();
    let _ = save_last_used(&name);
    
    Ok(profile)
}

#[tauri::command]
fn cmd_delete_profile(name: String) -> Result<(), String> {
    delete_profile(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_list_profiles() -> Result<Vec<String>, String> {
    list_profiles().map_err(|e| e.to_string())
}

#[tauri::command]
fn cmd_get_last_used() -> Option<String> {
    load_last_used()
}

#[tauri::command]
fn get_current_profile(state: State<'_, Mutex<AppState>>) -> Profile {
    state.lock().unwrap().profile.clone()
}

// === Initialization ===

fn init_logging() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "tap_tauri=debug,tap_core=debug,tap_platform=debug,tauri=info".into()),
        )
        .try_init();
}

fn setup_app(app: &AppHandle) {
    // Create the injector
    let injector = match EnigoInjector::new() {
        Ok(i) => i,
        Err(e) => {
            error!("Failed to create injector: {:?}", e);
            return;
        }
    };

    // Create the player
    let executor = InjectorExecutor::new(injector);
    let player_handle = Player::spawn(executor);

    // Store handles in app state (no more rdev hotkey listener)
    let state = AppState {
        engine_state: EngineState::Idle,
        profile: Profile::default(),
        player_handle: Some(player_handle),
        executed_count: 0,
        current_action_index: None,
    };

    app.manage(Mutex::new(state));

    // Start event polling loop
    let app_handle = app.clone();
    std::thread::spawn(move || {
        poll_events(app_handle);
    });

    info!("App setup complete");
}

fn poll_events(app: AppHandle) {
    loop {
        std::thread::sleep(std::time::Duration::from_millis(50));

        let state: State<'_, Mutex<AppState>> = app.state();

        // Collect player events
        let player_events: Vec<_> = {
            let app_state = state.lock().unwrap();
            app_state
                .player_handle
                .as_ref()
                .map(|h| std::iter::from_fn(|| h.try_recv()).collect())
                .unwrap_or_default()
        };

        // Process player events
        for event in player_events {
            debug!(?event, "received engine event");

            // Update state
            {
                let mut app_state = state.lock().unwrap();
                match &event {
                    EngineEvent::StateChanged { new, .. } => {
                        app_state.engine_state = *new;
                    }
                    EngineEvent::ActionCompleted { index } => {
                        app_state.current_action_index = Some(*index);
                        app_state.executed_count += 1;
                    }
                    EngineEvent::IterationCompleted { iteration } => {
                        debug!(iteration, "iteration completed");
                    }
                    _ => {}
                }
            }

            // Emit to frontend
            if let Err(e) = app.emit("engine-event", &event) {
                warn!("Failed to emit event to frontend: {}", e);
            }
        }
    }
}

/// Handle emergency stop shortcut
fn handle_emergency_stop(app: &AppHandle) {
    warn!("Emergency stop shortcut triggered!");
    let state: State<'_, Mutex<AppState>> = app.state();
    let app_state = state.lock().unwrap();
    if let Some(ref player) = app_state.player_handle {
        player.send(EngineCommand::EmergencyStop);
    }
    drop(app_state);
    if let Err(e) = app.emit("emergency-stop", ()) {
        warn!("Failed to emit emergency-stop: {}", e);
    }
}

fn main() {
    init_logging();

    // Define emergency stop shortcut: Ctrl+Shift+Backspace
    let emergency_shortcut =
        Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Backspace);

    tauri::Builder::default()
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(move |app, shortcut, event| {
                    if shortcut == &emergency_shortcut && event.state == ShortcutState::Pressed {
                        handle_emergency_stop(app);
                    }
                })
                .build(),
        )
        .setup(move |app| {
            // Register the shortcut
            if let Err(e) = app.global_shortcut().register(emergency_shortcut.clone()) {
                error!("Failed to register emergency shortcut: {:?}", e);
            } else {
                info!("Emergency stop shortcut registered: Ctrl+Shift+Backspace");
            }

            setup_app(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_default_profile,
            get_state,
            start_execution,
            pause_execution,
            resume_execution,
            stop_execution,
            emergency_stop,
            update_profile,
            set_simple_repeat,
            cmd_save_profile,
            cmd_load_profile,
            cmd_delete_profile,
            cmd_list_profiles,
            cmd_get_last_used,
            get_current_profile,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tap");
}
