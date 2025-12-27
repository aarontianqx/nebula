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

#[allow(dead_code)]
pub fn log_dir() -> PathBuf {
    config_dir().join("logs")
}

