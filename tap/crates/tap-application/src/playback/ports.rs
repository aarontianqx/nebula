//! What playback needs from the outside world; the adapter supplies the implementations.

use std::sync::Arc;

use tap_core::{Action, Color, MacroDocument};

/// Performs a concrete action against the OS.
pub trait ActionExecutor: Send + Sync + 'static {
    fn execute(&self, action: &Action) -> Result<(), String>;
}

/// Answers the questions conditions and window-relative coordinates ask about the screen.
pub trait PlatformConditionProvider: Send + Sync + 'static {
    fn is_window_focused(&self, title: Option<&str>, process: Option<&str>) -> bool;
    fn window_exists(&self, title: Option<&str>, process: Option<&str>) -> bool;
    fn get_pixel_color(&self, x: i32, y: i32) -> Option<Color>;
    /// The screen position of the top-left corner of the window matching `title` / `process`:
    /// the origin of window-relative coordinates. `None` when there is no such window (or it is
    /// minimized).
    fn window_origin(&self, title: Option<&str>, process: Option<&str>) -> Option<(i32, i32)>;
}

/// Finds the macro a `call_macro` names.
pub type SubMacroLoader = Arc<dyn Fn(&str) -> Result<MacroDocument, String> + Send + Sync>;
