//! tap-application: the use cases above the pure domain (`tap-core`) and below the adapter.
//!
//! - `playback`: the player thread that plays a macro, its command/event protocol and the ports
//!   it needs from the OS.
//! - `coordinator`: the adapter's entry point; owns the player and the session.
//! - `session`: the macro being edited and the run-time overrides for its next run.
//! - `resolve`: the Resolve stage, turning a templated `DslAction` into a concrete `Action`.
//! - `recording`, `recorder`: a recording in progress, and how raw input events become macro steps.
//! - `key_click`: when the Key->Click tool is due to click.
//! - `simple`: the simple-repeat tool as an ordinary macro.
//! - `coords`: window-relative coordinates.
//! - `library`: the saved macros on disk and the recently-used list.
//!
//! The crate is platform-agnostic: it defines ports as traits and lets the infrastructure
//! (`tap-platform`) and the adapter (`src-tauri`) supply the implementations.

mod coordinator;
mod coords;
mod key_click;
mod library;
mod playback;
mod recorder;
mod recording;
mod resolve;
mod session;
mod simple;

pub use coordinator::Coordinator;
pub use coords::window_binding;
pub use key_click::{ClickLocation, KeyClickConfig, KeyClickMachine, KeyDownOutcome};
pub use library::{default_root, MacroLibrary, StorageError, StorageResult};
pub use playback::{
    ActionExecutor, EngineCommand, EngineEvent, EngineState, PlatformConditionProvider, Player,
    PlayerConfig, PlayerHandle, SubMacroLoader,
};
pub use recorder::{MouseButtonRaw, RawEventType, RecorderState};
pub use recording::{RecordingSession, RecordingStatus, WindowLocator};
pub use resolve::{resolve_action, ResolveError};
pub use session::SessionStore;
pub use simple::simple_repeat_document;
