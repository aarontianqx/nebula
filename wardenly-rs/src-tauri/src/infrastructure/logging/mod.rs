//! Logging infrastructure with file output support for release builds.

use crate::infrastructure::config::paths;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Layer};

/// Initialize logging with console and optional file output.
///
/// In release mode, logs are written to:
/// - macOS: ~/Library/Application Support/wardenly/logs/
/// - Windows: %APPDATA%\wardenly\logs\
/// - Linux: ~/.config/wardenly/logs/
pub fn setup(is_production: bool) {
    let filter = if is_production {
        EnvFilter::new("info")
    } else {
        EnvFilter::new("debug")
    };

    // Console layer (always enabled)
    let console_layer = fmt::layer().with_target(true).with_filter(filter.clone());

    // File layer (for release builds)
    let file_layer = if is_production {
        let log_dir = paths::log_dir();

        // Create log directory if it doesn't exist
        if let Err(e) = std::fs::create_dir_all(&log_dir) {
            eprintln!(
                "Warning: Failed to create log directory {:?}: {}",
                log_dir, e
            );
            None
        } else {
            // Create daily rotating file appender
            let file_appender = RollingFileAppender::new(Rotation::DAILY, &log_dir, "wardenly.log");

            Some(
                fmt::layer()
                    .with_target(true)
                    .with_ansi(false) // No ANSI colors in file output
                    .with_writer(file_appender)
                    .with_filter(EnvFilter::new("info")),
            )
        }
    } else {
        None
    };

    // Initialize subscriber with appropriate layers
    match file_layer {
        Some(file_layer) => {
            tracing_subscriber::registry()
                .with(console_layer)
                .with(file_layer)
                .init();
        }
        None => {
            tracing_subscriber::registry().with(console_layer).init();
        }
    }

    if is_production {
        // Log the log directory path after initialization
        tracing::info!("File logging enabled: {:?}", paths::log_dir());
    }
    tracing::info!("Logging initialized (production={})", is_production);
}
