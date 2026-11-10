//! Where the pointer is right now, in the shared injection coordinate space (see `dpi`).

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::cursor_position;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::cursor_position;

/// `None` when the position cannot be read.
#[cfg(not(any(windows, target_os = "macos")))]
pub fn cursor_position() -> Option<(i32, i32)> {
    None
}
