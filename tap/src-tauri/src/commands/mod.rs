//! Tauri command handlers, grouped by concern. Each command is a thin adapter over the
//! application layer (`tap-application`); registration lives in `main.rs`.

pub mod document;
pub mod engine;
pub mod key_click;
pub mod recording;
pub mod system;
pub mod yaml;
