//! Window API for detecting and interacting with OS windows.
//!
//! Provides functionality for:
//! - Getting foreground window information
//! - Finding windows by title or process name
//! - Getting window rectangles for relative coordinate support

use serde::{Deserialize, Serialize};

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

/// Get the currently focused foreground window.
pub fn get_foreground_window() -> Option<WindowInfo> {
    #[cfg(windows)]
    {
        windows_impl::get_foreground_window()
    }
    #[cfg(not(windows))]
    {
        // TODO: macOS implementation
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
        windows_impl::list_windows()
    }
    #[cfg(not(windows))]
    {
        // TODO: macOS implementation
        Vec::new()
    }
}

/// Get the rectangle of a window by handle.
pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
    #[cfg(windows)]
    {
        windows_impl::get_window_rect(handle)
    }
    #[cfg(not(windows))]
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

#[cfg(windows)]
mod windows_impl {
    use super::{WindowInfo, WindowRect};
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;
    use windows_sys::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
    use windows_sys::Win32::System::ProcessStatus::GetModuleBaseNameW;
    use windows_sys::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetForegroundWindow, GetWindowRect as WinGetWindowRect,
        GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
    };

    pub fn get_foreground_window() -> Option<WindowInfo> {
        unsafe {
            let hwnd = GetForegroundWindow();
            if hwnd.is_null() {
                return None;
            }
            get_window_info(hwnd as usize)
        }
    }

    pub fn list_windows() -> Vec<WindowInfo> {
        let mut windows: Vec<WindowInfo> = Vec::new();

        unsafe {
            EnumWindows(
                Some(enum_window_callback),
                &mut windows as *mut Vec<WindowInfo> as LPARAM,
            );
        }

        windows
    }

    unsafe extern "system" fn enum_window_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let windows = &mut *(lparam as *mut Vec<WindowInfo>);

        // Skip invisible windows
        if IsWindowVisible(hwnd) == 0 {
            return TRUE;
        }

        // Skip windows with empty titles
        let title_len = GetWindowTextLengthW(hwnd);
        if title_len == 0 {
            return TRUE;
        }

        if let Some(info) = get_window_info(hwnd as usize) {
            windows.push(info);
        }

        TRUE
    }

    fn get_window_info(handle: usize) -> Option<WindowInfo> {
        unsafe {
            let hwnd = handle as HWND;

            // Get window title
            let title_len = GetWindowTextLengthW(hwnd);
            if title_len == 0 {
                return None;
            }

            let mut title_buf: Vec<u16> = vec![0; (title_len + 1) as usize];
            let copied = GetWindowTextW(hwnd, title_buf.as_mut_ptr(), title_buf.len() as i32);
            if copied == 0 {
                return None;
            }
            title_buf.truncate(copied as usize);
            let title = OsString::from_wide(&title_buf)
                .to_string_lossy()
                .into_owned();

            // Get process ID
            let mut pid: u32 = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);

            // Get process name
            let process_name = get_process_name(pid).unwrap_or_default();

            // Get window rect
            let rect = get_window_rect(handle).unwrap_or_default();

            // Check visibility
            let visible = IsWindowVisible(hwnd) != 0;

            Some(WindowInfo {
                handle,
                title,
                process_name,
                pid,
                rect,
                visible,
            })
        }
    }

    fn get_process_name(pid: u32) -> Option<String> {
        unsafe {
            let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, 0, pid);
            if handle.is_null() {
                return None;
            }

            let mut name_buf: Vec<u16> = vec![0; 260];
            let len = GetModuleBaseNameW(
                handle,
                ptr::null_mut(),
                name_buf.as_mut_ptr(),
                name_buf.len() as u32,
            );

            // Close handle
            windows_sys::Win32::Foundation::CloseHandle(handle);

            if len == 0 {
                return None;
            }

            name_buf.truncate(len as usize);
            Some(
                OsString::from_wide(&name_buf)
                    .to_string_lossy()
                    .into_owned(),
            )
        }
    }

    pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
        unsafe {
            let hwnd = handle as HWND;
            let mut rect: RECT = std::mem::zeroed();

            if WinGetWindowRect(hwnd, &mut rect) == 0 {
                return None;
            }

            Some(WindowRect {
                x: rect.left,
                y: rect.top,
                width: rect.right - rect.left,
                height: rect.bottom - rect.top,
            })
        }
    }
}

