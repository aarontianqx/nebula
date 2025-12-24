//! Tauri build script.
//!
//! This script runs during cargo build and handles Tauri-specific build steps.
//!
//! Icon handling:
//! - All platform icons should be pre-generated and stored in `icons/` directory
//! - macOS: icon.png (Tauri converts to icns) or icon.icns
//! - Windows: icon.ico (can be generated from icon.png using tools like ImageMagick or online converters)
//! - Linux: icon.png
//!
//! See: https://v2.tauri.app/start/migrate/from-tauri-1/#icons

fn main() {
    tauri_build::build()
}
