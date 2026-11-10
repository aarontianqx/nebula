//! Window queries: the foreground window, the list of visible windows, and which of them a
//! target (title / process pattern) stands for.

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

/// Whether `window` matches the title and process patterns (partial, case-insensitive; a
/// missing pattern matches anything).
fn window_matches(window: &WindowInfo, title: Option<&str>, process: Option<&str>) -> bool {
    let title_match = title
        .map(|t| window.title.to_lowercase().contains(&t.to_lowercase()))
        .unwrap_or(true);

    let process_match = process
        .map(|p| {
            window
                .process_name
                .to_lowercase()
                .contains(&p.to_lowercase())
        })
        .unwrap_or(true);

    title_match && process_match
}

/// Check if a window with the given title is currently focused.
pub fn is_window_focused(title: Option<&str>, process: Option<&str>) -> bool {
    get_foreground_window().is_some_and(|fg| window_matches(&fg, title, process))
}

/// Check if a window with the given title/process exists.
pub fn window_exists(title: Option<&str>, process: Option<&str>) -> bool {
    list_windows()
        .iter()
        .any(|w| window_matches(w, title, process))
}

/// Windows parks minimized windows at (-32000, -32000); anything that far away is not on any
/// display. Real positions (including displays left of or above the main one) are far closer.
const PARKED_OFFSCREEN: i32 = -30_000;

/// Whether a window can be the origin of window-relative coordinates: it has an area and is not
/// minimized.
fn is_usable_origin(window: &WindowInfo) -> bool {
    window.rect.width > 0
        && window.rect.height > 0
        && window.rect.x > PARKED_OFFSCREEN
        && window.rect.y > PARKED_OFFSCREEN
}

/// Choose the window a target (title and / or process pattern) stands for.
///
/// The foreground window wins when it matches, because that is the window the user is working
/// in. Otherwise the largest matching window is taken (an application's main window rather than
/// its helper strips), the topmost one when several are equally large. Minimized and off-screen
/// windows are never chosen.
#[cfg(any(windows, target_os = "macos", test))]
fn pick_target_window(
    windows: Vec<WindowInfo>,
    foreground: Option<WindowInfo>,
    title: Option<&str>,
    process: Option<&str>,
) -> Option<WindowInfo> {
    if title.is_none() && process.is_none() {
        return None; // no pattern would match every window: there is no target
    }
    let usable_match = |w: &WindowInfo| is_usable_origin(w) && window_matches(w, title, process);

    if let Some(fg) = foreground.filter(usable_match) {
        return Some(fg);
    }

    let area = |w: &WindowInfo| i64::from(w.rect.width) * i64::from(w.rect.height);
    let candidates: Vec<WindowInfo> = windows.into_iter().filter(usable_match).collect();
    let largest = candidates.iter().map(area).max()?;
    candidates.into_iter().find(|w| area(w) == largest)
}

/// The window a target (title and / or process pattern) stands for right now, if there is one.
pub fn find_target_window(title: Option<&str>, process: Option<&str>) -> Option<WindowInfo> {
    pick_target_window(list_windows(), get_foreground_window(), title, process)
}

/// The screen position of the target window's top-left corner: the origin of window-relative
/// coordinates. `None` when the window is not there (or is minimized).
pub fn window_origin(title: Option<&str>, process: Option<&str>) -> Option<(i32, i32)> {
    find_target_window(title, process).map(|w| (w.rect.x, w.rect.y))
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

    fn at(pid: u32, title: &str, x: i32, y: i32, width: i32, height: i32) -> WindowInfo {
        let mut w = window(pid, title, width, height);
        w.rect.x = x;
        w.rect.y = y;
        w
    }

    #[test]
    fn the_foreground_window_is_the_target_when_it_matches() {
        let front = at(1, "Game - level 2", 100, 50, 800, 600);
        let windows = vec![at(2, "Game - menu", 0, 0, 1920, 1080), front.clone()];
        let picked = pick_target_window(windows, Some(front), Some("game"), None).unwrap();
        assert_eq!(picked.title, "Game - level 2");
    }

    #[test]
    fn without_a_matching_foreground_window_the_largest_match_is_the_target() {
        let windows = vec![
            at(1, "Game (tooltip)", 10, 10, 200, 40),
            at(1, "Game", 100, 50, 800, 600),
            at(3, "Other", 0, 0, 1920, 1080),
        ];
        let other = at(3, "Other", 0, 0, 1920, 1080);
        let picked = pick_target_window(windows, Some(other), Some("game"), None).unwrap();
        assert_eq!((picked.rect.x, picked.rect.y), (100, 50));
    }

    #[test]
    fn equally_large_matches_resolve_to_the_topmost() {
        let windows = vec![at(1, "A", 5, 5, 400, 300), at(1, "B", 50, 50, 400, 300)];
        let picked = pick_target_window(windows, None, None, Some("app1")).unwrap();
        assert_eq!(picked.title, "A");
    }

    #[test]
    fn a_minimized_window_is_not_a_target() {
        // Windows parks minimized windows at (-32000, -32000).
        let minimized = at(1, "Game", -32000, -32000, 160, 28);
        assert!(
            pick_target_window(vec![minimized.clone()], Some(minimized), Some("game"), None)
                .is_none()
        );
    }

    #[test]
    fn a_window_on_a_display_above_or_left_of_the_main_one_is_a_target() {
        // Negative coordinates are normal in a multi-display layout (seen on macOS: y = -1112).
        let windows = vec![at(1, "Game", -1500, -1112, 1920, 1080)];
        let picked = pick_target_window(windows, None, Some("game"), None).unwrap();
        assert_eq!((picked.rect.x, picked.rect.y), (-1500, -1112));
    }

    #[test]
    fn a_zero_sized_window_is_not_a_target() {
        assert!(
            pick_target_window(vec![at(1, "Game", 0, 0, 0, 0)], None, Some("game"), None).is_none()
        );
    }

    #[test]
    fn no_pattern_means_no_target() {
        let windows = vec![at(1, "Anything", 0, 0, 800, 600)];
        assert!(pick_target_window(windows, None, None, None).is_none());
    }

    #[test]
    fn both_patterns_must_match() {
        let windows = vec![at(1, "Game", 0, 0, 800, 600)];
        assert!(pick_target_window(windows.clone(), None, Some("game"), Some("app1")).is_some());
        assert!(pick_target_window(windows, None, Some("game"), Some("other")).is_none());
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
