//! Application wiring: builds the player, library and trackers, hands each to Tauri as its own
//! managed state, and handles the emergency stop.

use std::sync::{Arc, Mutex};

use tap_application::{default_root, Coordinator, MacroLibrary, Player, PlayerConfig};
use tap_platform::{start_mouse_tracker, EnigoInjector, MouseTrackerConfig};
use tauri::{AppHandle, Manager};
use tracing::{error, info, warn};

use crate::forward::{emit_to_frontend, forward_engine_events, forward_mouse_position};
use crate::key_click::KeyClickState;
use crate::ports::{EnigoExecutor, TauriPlatformProvider};
use crate::recording::RecordingState;
use crate::state::{CoordinatorState, DryRun};

pub fn setup_app(app: &AppHandle) {
    let injector = match EnigoInjector::new() {
        Ok(injector) => Arc::new(injector),
        Err(e) => {
            error!("Failed to create injector: {:?}", e);
            return;
        }
    };
    let dry_run = DryRun::default();
    let library = MacroLibrary::new(default_root());

    let player = Player::spawn(
        EnigoExecutor::new(injector.clone(), dry_run.clone()),
        TauriPlatformProvider,
        PlayerConfig::new(library.loader()),
    );
    let coordinator = Coordinator::new(player);

    forward_engine_events(app.clone(), coordinator.events());
    forward_mouse_position(
        app.clone(),
        start_mouse_tracker(MouseTrackerConfig::default()),
    );

    app.manage::<CoordinatorState>(Mutex::new(coordinator));
    app.manage(library);
    app.manage(dry_run);
    app.manage(injector);
    app.manage(RecordingState::default());
    app.manage(KeyClickState::default());

    info!("App setup complete");
}

/// Stop everything that injects input: the player and key-click mode.
pub fn stop_all(coordinator: &CoordinatorState, key_click: &KeyClickState) {
    coordinator.lock().unwrap().stop();
    key_click.stop();
}

pub fn handle_emergency_stop(app: &AppHandle) {
    warn!("Emergency stop shortcut triggered!");
    stop_all(
        &app.state::<CoordinatorState>(),
        &app.state::<KeyClickState>(),
    );
    emit_to_frontend(app, "emergency-stop", ());
}
