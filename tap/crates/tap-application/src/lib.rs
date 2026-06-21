//! tap-application: the application layer.
//!
//! Owns use-case orchestration that sits above the pure domain (`tap-core`)
//! and below the platform/adapter layers:
//!
//! - `coordinator` — orchestrates the player + session; the adapter's entry point.
//! - `session` — the canonical [`MacroDocument`](tap_core::MacroDocument) source of truth.
//! - `engine` — the playback [`Player`] thread, its command/event protocol, and
//!   the [`ActionExecutor`] / [`PlatformConditionProvider`] ports it depends on.
//! - `resolve` — the Resolve stage: parameterized `DslAction` → concrete `Action`.
//! - `recorder` — captures raw input events into a [`Timeline`](tap_core::Timeline).
//! - `storage` — canonical YAML persistence for macro documents (+ legacy JSON).
//! - `submacro` — sub-macro call-stack tracking and variable scoping.
//!
//! This crate is platform-agnostic: it defines ports as traits and lets the
//! infrastructure (`tap-platform`) and adapter (`src-tauri`) layers supply the
//! concrete implementations.

mod coordinator;
mod engine;
mod recorder;
mod resolve;
mod session;
mod storage;
mod submacro;

pub use coordinator::Coordinator;
pub use engine::{
    ActionExecutor, EngineCommand, EngineEvent, EngineState, PlatformConditionProvider, Player,
    PlayerHandle, SubMacroLoader,
};
pub use recorder::{
    BufferedEvent, MouseButtonRaw, RawEventType, Recorder, RecorderConfig, RecorderEvent,
    RecorderState,
};
pub use resolve::{document_to_profile_view, resolve_action, ResolveError};
pub use session::SessionStore;
pub use storage::{
    delete_profile, ensure_profiles_dir, get_app_data_dir, get_profiles_dir, list_profiles,
    load_document, load_last_used, load_profile, load_recent, save_document, save_last_used,
    save_profile, save_recent, StorageError, StorageResult,
};
pub use submacro::{
    create_child_variable_store, create_submacro_context, prepare_submacro_args, SubMacroContext,
    SubMacroContextHandle, SubMacroError, MAX_CALL_DEPTH,
};
