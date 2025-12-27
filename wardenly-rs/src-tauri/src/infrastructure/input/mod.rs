mod keyboard;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

pub use keyboard::*;

/// Create a platform-specific keyboard listener
pub fn create_keyboard_listener() -> Box<dyn KeyboardListener> {
    #[cfg(target_os = "macos")]
    {
        Box::new(macos::MacOSKeyboardListener::new())
    }

    #[cfg(target_os = "windows")]
    {
        Box::new(windows::WindowsKeyboardListener::new())
    }

    #[cfg(target_os = "linux")]
    {
        Box::new(linux::LinuxKeyboardListener::new())
    }
}

