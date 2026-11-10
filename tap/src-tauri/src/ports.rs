//! Adapter bridges: concrete implementations of the engine's ports.
//!
//! `tap-application` depends only on the `ActionExecutor` / `PlatformConditionProvider` traits;
//! this module wires them to the `tap-platform` injector and window / pixel queries.

use std::sync::Arc;

use tap_application::{ActionExecutor, PlatformConditionProvider};
use tap_core::{Action, ConditionColor};
use tap_platform::{
    get_pixel_color, is_window_focused, window_exists, EnigoInjector, InputInjector,
};
use tracing::debug;

use crate::state::DryRun;

/// Bridges the `tap-platform` injector to the application-layer [`ActionExecutor`]
/// port, keeping `tap-application` free of any platform dependency.
pub struct EnigoExecutor {
    injector: Arc<EnigoInjector>,
    dry_run: DryRun,
}

impl EnigoExecutor {
    pub fn new(injector: Arc<EnigoInjector>, dry_run: DryRun) -> Self {
        Self { injector, dry_run }
    }
}

impl ActionExecutor for EnigoExecutor {
    fn execute(&self, action: &Action) -> Result<(), String> {
        if self.dry_run.get() {
            debug!(?action, "dry-run: skipping injection");
            return Ok(());
        }
        self.injector.inject(action).map_err(|e| e.to_string())
    }
}

/// Platform condition provider for the engine.
pub struct TauriPlatformProvider;

impl PlatformConditionProvider for TauriPlatformProvider {
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool {
        is_window_focused(title, process)
    }

    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool {
        window_exists(title, process)
    }

    fn get_pixel_color(&self, x: i32, y: i32) -> Option<ConditionColor> {
        get_pixel_color(x, y).map(|c| ConditionColor::new(c.r, c.g, c.b))
    }

    fn window_origin(&self, title: Option<&str>, process: Option<&str>) -> Option<(i32, i32)> {
        tap_platform::window_origin(title, process)
    }
}
