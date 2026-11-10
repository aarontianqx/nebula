//! Application wiring: builds the engine, recorder and trackers, and handles the emergency stop.

use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use tap_application::{Coordinator, Player, Recorder};
use tap_platform::{start_mouse_tracker, EnigoInjector, MouseTrackerConfig};
use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info, warn};

use crate::events::poll_events;
use crate::ports::{EnigoExecutor, TauriPlatformProvider};
use crate::state::AppState;

pub fn setup_app(app: &AppHandle) {
    // Create the injector
    let injector = match EnigoInjector::new() {
        Ok(i) => i,
        Err(e) => {
            error!("Failed to create injector: {:?}", e);
            return;
        }
    };

    // Create the platform condition provider
    let platform_provider = TauriPlatformProvider;

    // Create the player with executor and platform provider, wrapped in the
    // application-layer coordinator (owns the engine state machine + document).
    let dry_run = Arc::new(AtomicBool::new(false));
    let executor = EnigoExecutor::new(injector, dry_run.clone());
    let player_handle = Player::spawn(executor, platform_provider);
    let coordinator = Coordinator::new(player_handle);

    // Create the recorder
    let recorder = Recorder::with_defaults();

    // Start global mouse tracking
    let mouse_tracker = start_mouse_tracker(MouseTrackerConfig::default());

    // Store handles in app state
    let state = AppState {
        coordinator,
        dry_run,
        recorder,
        input_hook: None,
        mouse_tracker: Some(mouse_tracker),
        key_click_handle: None,
        key_click_injector: None,
    };

    app.manage(Mutex::new(state));

    // Start event polling loop
    let app_handle = app.clone();
    std::thread::spawn(move || {
        poll_events(app_handle);
    });

    info!("App setup complete");
}

/// Handle emergency stop shortcut
pub fn handle_emergency_stop(app: &AppHandle) {
    warn!("Emergency stop shortcut triggered!");
    let state: State<'_, Mutex<AppState>> = app.state();
    let app_state = state.lock().unwrap();

    // Stop player if running
    app_state.coordinator.emergency_stop();

    // Stop key-click mode if running (just signal, don't take)
    if let Some(ref handle) = app_state.key_click_handle {
        handle.stop();
        info!("Key-click mode stop requested by emergency stop shortcut");
    }

    drop(app_state);
    if let Err(e) = app.emit("emergency-stop", ()) {
        warn!("Failed to emit emergency-stop: {}", e);
    }
}
