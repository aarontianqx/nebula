//! Window API for detecting and interacting with OS windows.
//!
//! Provides functionality for:
//! - Getting foreground window information
//! - Finding windows by title or process name
//! - Getting window rectangles for relative coordinate support
//!
//! Platform implementations:
//! - Windows: Uses Win32 API (`windows.rs`)
//! - macOS: Uses Quartz window services / `CGWindowList` (`macos.rs`)

use serde::{Deserialize, Serialize};

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

/// Information about a window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    /// Window handle (platform-specific identifier).
    pub handle: usize,
    /// Window title.
    pub title: String,
    /// Process name (e.g., "notepad.exe").
    pub process_name: String,
    /// Process ID.
    pub pid: u32,
    /// Window rectangle.
    pub rect: WindowRect,
    /// Whether the window is visible.
    pub visible: bool,
}

/// Window rectangle in screen coordinates.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct WindowRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl WindowRect {
    /// Convert relative coordinates to absolute screen coordinates.
    pub fn to_absolute(&self, rel_x: i32, rel_y: i32) -> (i32, i32) {
        (self.x + rel_x, self.y + rel_y)
    }
}

/// The window that stands for the application `pid`, among `windows` (front to back).
///
/// An application's topmost window is not necessarily its main one (both observed on macOS):
/// - thin or transparent strips (an overlay, a title-bar helper) are ordered above the real
///   window, so the *largest* of its windows sets the scale and anything under half of it is
///   ignored;
/// - Electron-style applications stack an untitled helper window of the same size on top of the
///   titled one, so among the remaining candidates the topmost *titled* window wins (the
///   topmost one when none has a title, e.g. without the Screen Recording permission).
///
/// Between comparably sized, titled windows the z-order decides: the frontmost has focus.
#[cfg(any(target_os = "macos", test))]
fn main_window_of(windows: Vec<WindowInfo>, pid: u32) -> Option<WindowInfo> {
    let area = |w: &WindowInfo| i64::from(w.rect.width.max(0)) * i64::from(w.rect.height.max(0));
    let own: Vec<WindowInfo> = windows.into_iter().filter(|w| w.pid == pid).collect();
    let largest = own.iter().map(area).max()?;
    let mut candidates: Vec<WindowInfo> =
        own.into_iter().filter(|w| area(w) * 2 >= largest).collect();
    let main = candidates
        .iter()
        .position(|w| !w.title.is_empty())
        .unwrap_or(0);
    Some(candidates.swap_remove(main))
}

/// Get the currently focused foreground window.
pub fn get_foreground_window() -> Option<WindowInfo> {
    #[cfg(windows)]
    {
        windows::get_foreground_window()
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_foreground_window()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        None
    }
}

/// Find a window by its title (partial match, case-insensitive).
pub fn find_window_by_title(title: &str) -> Option<WindowInfo> {
    let title_lower = title.to_lowercase();
    list_windows()
        .into_iter()
        .find(|w| w.title.to_lowercase().contains(&title_lower))
}

/// Find a window by process name (partial match, case-insensitive).
pub fn find_window_by_process(process: &str) -> Option<WindowInfo> {
    let process_lower = process.to_lowercase();
    list_windows()
        .into_iter()
        .find(|w| w.process_name.to_lowercase().contains(&process_lower))
}

/// List all visible windows.
pub fn list_windows() -> Vec<WindowInfo> {
    #[cfg(windows)]
    {
        windows::list_windows()
    }
    #[cfg(target_os = "macos")]
    {
        macos::list_windows()
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        Vec::new()
    }
}

/// Get the rectangle of a window by handle.
pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
    #[cfg(windows)]
    {
        windows::get_window_rect(handle)
    }
    #[cfg(target_os = "macos")]
    {
        macos::get_window_rect(handle)
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let _ = handle;
        None
    }
}

/// Check if a window with the given title is currently focused.
pub fn is_window_focused(title: Option<&str>, process: Option<&str>) -> bool {
    let Some(fg) = get_foreground_window() else {
        return false;
    };

    let title_match = title
        .map(|t| fg.title.to_lowercase().contains(&t.to_lowercase()))
        .unwrap_or(true);

    let process_match = process
        .map(|p| fg.process_name.to_lowercase().contains(&p.to_lowercase()))
        .unwrap_or(true);

    title_match && process_match
}

/// Check if a window with the given title/process exists.
pub fn window_exists(title: Option<&str>, process: Option<&str>) -> bool {
    let windows = list_windows();

    windows.iter().any(|w| {
        let title_match = title
            .map(|t| w.title.to_lowercase().contains(&t.to_lowercase()))
            .unwrap_or(true);

        let process_match = process
            .map(|p| w.process_name.to_lowercase().contains(&p.to_lowercase()))
            .unwrap_or(true);

        title_match && process_match
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(pid: u32, title: &str, width: i32, height: i32) -> WindowInfo {
        WindowInfo {
            handle: 0,
            title: title.to_string(),
            process_name: format!("app{pid}"),
            pid,
            rect: WindowRect {
                x: 0,
                y: 0,
                width,
                height,
            },
            visible: true,
        }
    }

    fn title_of(w: Option<WindowInfo>) -> Option<String> {
        w.map(|w| w.title)
    }

    #[test]
    fn a_thin_strip_above_the_main_window_is_skipped() {
        // Observed on macOS: the app's first window in z-order was a 1920x32 strip.
        let windows = vec![window(1, "", 1920, 32), window(1, "Main", 1920, 1080)];
        assert_eq!(title_of(main_window_of(windows, 1)), Some("Main".into()));
    }

    #[test]
    fn an_untitled_helper_above_a_same_sized_titled_window_is_skipped() {
        // Observed on macOS with an Electron app: two 1457x888 windows, the front one untitled.
        let windows = vec![window(1, "", 1457, 888), window(1, "App", 1457, 888)];
        assert_eq!(title_of(main_window_of(windows, 1)), Some("App".into()));
    }

    #[test]
    fn without_any_titles_the_topmost_candidate_is_taken() {
        let windows = vec![window(1, "", 1000, 800), window(1, "", 1100, 900)];
        let main = main_window_of(windows, 1).expect("a window");
        assert_eq!((main.rect.width, main.rect.height), (1000, 800));
    }

    #[test]
    fn only_the_given_applications_windows_count() {
        let windows = vec![
            window(2, "Other, in front", 1512, 41),
            window(1, "Mine", 800, 600),
            window(2, "Other, big", 1512, 900),
        ];
        assert_eq!(title_of(main_window_of(windows, 1)), Some("Mine".into()));
    }

    #[test]
    fn comparably_sized_windows_follow_the_z_order() {
        // Two document windows: the front one has focus even though the other is a bit larger.
        let windows = vec![window(1, "Front", 900, 700), window(1, "Behind", 1000, 800)];
        assert_eq!(title_of(main_window_of(windows, 1)), Some("Front".into()));
    }

    #[test]
    fn an_application_without_windows_has_no_main_window() {
        assert!(main_window_of(vec![window(2, "Other", 800, 600)], 1).is_none());
        assert!(main_window_of(Vec::new(), 1).is_none());
    }

    #[test]
    fn windows_without_an_area_do_not_panic_or_win() {
        let windows = vec![window(1, "Empty", 0, 0), window(1, "Real", 640, 480)];
        assert_eq!(title_of(main_window_of(windows, 1)), Some("Real".into()));
        assert!(main_window_of(vec![window(1, "Empty", 0, 0)], 1).is_some());
    }
}
