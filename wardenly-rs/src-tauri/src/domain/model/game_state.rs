use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Per-session structured game state, aggregated from the protocol stream.
///
/// The page-bridge forwarder is the single writer: every downstream protocol
/// message updates `latest` under its protocol name before the corresponding
/// DomainEvent is published, so readers that react to an event always observe
/// a state at least as fresh as that event.
///
/// This replaces OCR-based judgment for protocol-driven scripts: conditions
/// read exact decoded fields instead of recognizing pixels/text on screen.
#[derive(Debug, Default)]
pub struct GameState {
    /// Latest decoded payload per protocol name (e.g. "S_2_C_MAILLIST_ID").
    latest: HashMap<String, Value>,
}

impl GameState {
    /// Record a downstream message. Called by the bridge forwarder only.
    pub fn update(&mut self, name: &str, data: Value) {
        self.latest.insert(name.to_string(), data);
    }

    /// Latest payload seen for a protocol.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.latest.get(name)
    }

    /// Resolve a dotted path like `state.S_2_C_MAILLIST_ID.mailNums`.
    /// The leading `state.` selects the game state as the root; numeric
    /// segments index into arrays. Returns None when any segment is missing.
    pub fn resolve(&self, path: &str) -> Option<&Value> {
        let path = path.strip_prefix("state.")?;
        let mut segments = path.split('.');
        let protocol = segments.next()?;
        let mut value = self.get(protocol)?;
        for seg in segments {
            value = match value {
                Value::Object(map) => map.get(seg)?,
                Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
                _ => return None,
            };
        }
        Some(value)
    }
}

/// Shared handle to a session's game state. std RwLock is sufficient:
/// critical sections are tiny and never held across an await.
pub type SharedGameState = Arc<RwLock<GameState>>;

pub fn new_shared_game_state() -> SharedGameState {
    Arc::new(RwLock::new(GameState::default()))
}

/// Resolve a dotted path (`a.b.0.c`) within an arbitrary JSON value.
pub fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut value = root;
    for seg in path.split('.') {
        value = match value {
            Value::Object(map) => map.get(seg)?,
            Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolve_state_paths() {
        let mut state = GameState::default();
        state.update(
            "S_2_C_MAILLIST_ID",
            json!({"mailNums": 50, "MailIdTypes": [{"mail_id": 7}]}),
        );

        assert_eq!(
            state.resolve("state.S_2_C_MAILLIST_ID.mailNums"),
            Some(&json!(50))
        );
        assert_eq!(
            state.resolve("state.S_2_C_MAILLIST_ID.MailIdTypes.0.mail_id"),
            Some(&json!(7))
        );
        assert_eq!(state.resolve("state.S_2_C_UNKNOWN.x"), None);
        assert_eq!(state.resolve("S_2_C_MAILLIST_ID.mailNums"), None);
    }

    #[test]
    fn resolve_json_paths() {
        let v = json!({"a": [{"b": 1}]});
        assert_eq!(resolve_path(&v, "a.0.b"), Some(&json!(1)));
        assert_eq!(resolve_path(&v, "a.1.b"), None);
        assert_eq!(resolve_path(&v, "a.b"), None);
    }
}
