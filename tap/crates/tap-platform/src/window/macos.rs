//! macOS implementation of the window API using Quartz window services.
//!
//! Windows are enumerated via `CGWindowListCopyWindowInfo`, which returns the
//! on-screen windows in front-to-back order. We keep only normal application
//! windows (`kCGWindowLayer == 0`) so menus, the Dock and other system layers
//! do not leak into the results.
//!
//! Coordinates are reported in the global *point* space (top-left origin),
//! which is the same space `enigo`/`rdev` use for injection and capture on
//! macOS. See `dpi/macos.rs` for the coordinate-system rationale.
//!
//! Note: reading a window's title (`kCGWindowName`) requires the Screen
//! Recording permission. Without it the owner/process name is still available,
//! so process-based matching keeps working while title matching may be empty.

use super::{WindowInfo, WindowRect};
use core_foundation::base::TCFType;
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use core_foundation::number::{CFNumber, CFNumberRef};
use core_foundation::string::{CFString, CFStringRef};
use core_graphics::geometry::{CGPoint, CGRect, CGSize};
use core_graphics::window::{
    copy_window_info, kCGNullWindowID, kCGWindowBounds, kCGWindowLayer,
    kCGWindowListExcludeDesktopElements, kCGWindowListOptionOnScreenOnly, kCGWindowName,
    kCGWindowNumber, kCGWindowOwnerName, kCGWindowOwnerPID,
};
use std::ffi::c_void;

extern "C" {
    fn CGRectMakeWithDictionaryRepresentation(dict: CFDictionaryRef, rect: *mut CGRect) -> bool;
}

/// Look up a value pointer for `key` in an (untyped) CF dictionary.
///
/// # Safety
/// `key` must be a valid `CFStringRef` and `dict` a live CF dictionary.
unsafe fn dict_value(dict: &CFDictionary, key: CFStringRef) -> Option<*const c_void> {
    dict.find(key as *const c_void).map(|v| *v)
}

unsafe fn read_string(dict: &CFDictionary, key: CFStringRef) -> Option<String> {
    let ptr = dict_value(dict, key)?;
    if ptr.is_null() {
        return None;
    }
    Some(CFString::wrap_under_get_rule(ptr as CFStringRef).to_string())
}

unsafe fn read_i64(dict: &CFDictionary, key: CFStringRef) -> Option<i64> {
    let ptr = dict_value(dict, key)?;
    if ptr.is_null() {
        return None;
    }
    CFNumber::wrap_under_get_rule(ptr as CFNumberRef).to_i64()
}

unsafe fn read_bounds(dict: &CFDictionary, key: CFStringRef) -> Option<WindowRect> {
    let ptr = dict_value(dict, key)?;
    if ptr.is_null() {
        return None;
    }
    let mut rect = CGRect::new(&CGPoint::new(0.0, 0.0), &CGSize::new(0.0, 0.0));
    if !CGRectMakeWithDictionaryRepresentation(ptr as CFDictionaryRef, &mut rect) {
        return None;
    }
    Some(WindowRect {
        x: rect.origin.x as i32,
        y: rect.origin.y as i32,
        width: rect.size.width as i32,
        height: rect.size.height as i32,
    })
}

/// Parse one window-info dictionary into a [`WindowInfo`].
///
/// Returns `None` for windows we want to ignore (non-normal layers or windows
/// without an owning application).
unsafe fn parse_window(dict: &CFDictionary) -> Option<WindowInfo> {
    // Only normal application windows (layer 0). Menus, the Dock, the menu bar
    // and other chrome live on higher layers.
    if read_i64(dict, kCGWindowLayer).unwrap_or(-1) != 0 {
        return None;
    }

    let process_name = read_string(dict, kCGWindowOwnerName).unwrap_or_default();
    if process_name.is_empty() {
        return None;
    }

    let handle = read_i64(dict, kCGWindowNumber)? as usize;
    let pid = read_i64(dict, kCGWindowOwnerPID).unwrap_or(0) as u32;
    let title = read_string(dict, kCGWindowName).unwrap_or_default();
    let rect = read_bounds(dict, kCGWindowBounds).unwrap_or_default();

    Some(WindowInfo {
        handle,
        title,
        process_name,
        pid,
        rect,
        visible: true,
    })
}

/// Collect on-screen windows (front-to-back) into `WindowInfo` values.
fn on_screen_windows() -> Vec<WindowInfo> {
    let array = match copy_window_info(
        kCGWindowListOptionOnScreenOnly | kCGWindowListExcludeDesktopElements,
        kCGNullWindowID,
    ) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    let mut out = Vec::new();
    for i in 0..array.len() {
        if let Some(item) = array.get(i) {
            let dict_ptr = *item as CFDictionaryRef;
            if dict_ptr.is_null() {
                continue;
            }
            let dict = unsafe { CFDictionary::wrap_under_get_rule(dict_ptr) };
            if let Some(info) = unsafe { parse_window(&dict) } {
                out.push(info);
            }
        }
    }
    out
}

/// Get the currently focused foreground window.
///
/// The window server keeps the on-screen list in front-to-back order, so the
/// first normal window is the frontmost (focused) one.
pub fn get_foreground_window() -> Option<WindowInfo> {
    on_screen_windows().into_iter().next()
}

/// List all visible windows.
pub fn list_windows() -> Vec<WindowInfo> {
    on_screen_windows()
}

/// Get the rectangle of a window by handle (its `CGWindowID`).
pub fn get_window_rect(handle: usize) -> Option<WindowRect> {
    on_screen_windows()
        .into_iter()
        .find(|w| w.handle == handle)
        .map(|w| w.rect)
}
