//! Windows implementation of window API using Win32.

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

