//! Playback: a player thread that plays one macro at a time.
//!
//! - `protocol`: the commands the player takes and the events it reports.
//! - `ports`: what it needs from outside (executing actions, asking about windows and pixels).
//! - `player`: the thread, its state machine and its waits.
//! - `run`: a run's passes, its schedule and how each step is decided.
//! - `dispatch`: performing one concrete action.
//!
//! `clock` (active-time scheduling), `humanize` (position and timing variation) and `call_macro`
//! (call stack and scopes) are small collaborators of `run` and `dispatch`.

mod call_macro;
mod clock;
mod dispatch;
mod humanize;
mod player;
mod ports;
mod protocol;
mod run;

pub use player::{Player, PlayerConfig, PlayerHandle};
pub use ports::{ActionExecutor, PlatformConditionProvider, SubMacroLoader};
pub use protocol::{EngineCommand, EngineEvent, EngineState};
