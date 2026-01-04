use std::path::PathBuf;

/// Get platform-specific configuration directory
pub fn config_dir() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wardenly")
    }

    #[cfg(target_os = "macos")]
    {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Library/Application Support/wardenly")
    }

    #[cfg(target_os = "linux")]
    {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("wardenly")
    }
}

pub fn default_sqlite_path() -> PathBuf {
    config_dir().join("data.db")
}

/// Get platform-specific log directory
/// - macOS: ~/Library/Application Support/wardenly/logs/
/// - Windows: %APPDATA%\wardenly\logs\
/// - Linux: ~/.config/wardenly/logs/
pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}
