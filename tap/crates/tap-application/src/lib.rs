//! tap-application: the use cases above the pure domain (`tap-core`) and below the adapter.
//!
//! - `playback`: the player thread that plays a macro, its command/event protocol and the ports
//!   it needs from the OS.
//! - `coordinator`: the adapter's entry point; owns the player and the session.
//! - `session`: the macro being edited and the run-time overrides for its next run.
//! - `resolve`: the Resolve stage, turning a templated `DslAction` into a concrete `Action`.
//! - `recorder`: turns raw input events into macro steps.
//! - `simple`: the simple-repeat tool as an ordinary macro.
//! - `coords`: window-relative coordinates.
//! - `library`: the saved macros on disk and the recently-used list.
//!
//! The crate is platform-agnostic: it defines ports as traits and lets the infrastructure
//! (`tap-platform`) and the adapter (`src-tauri`) supply the implementations.

mod coordinator;
mod coords;
mod library;
mod playback;
mod recorder;
mod resolve;
mod session;
mod simple;

pub use coordinator::Coordinator;
pub use coords::{relativize_raw, window_binding};
pub use library::{default_root, MacroLibrary, StorageError, StorageResult};
pub use playback::{
    ActionExecutor, EngineCommand, EngineEvent, EngineState, PlatformConditionProvider, Player,
    PlayerConfig, PlayerHandle, SubMacroLoader,
};
pub use recorder::{
    BufferedEvent, MouseButtonRaw, RawEventType, Recorder, RecorderConfig, RecorderEvent,
    RecorderState,
};
pub use resolve::{resolve_action, ResolveError};
pub use session::SessionStore;
pub use simple::simple_repeat_document;
