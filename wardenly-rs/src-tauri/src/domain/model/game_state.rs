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
    /// The leading `state.` selects the game state as the root. Supports
    /// array selectors (see resolve_path). Returns an owned value.
    pub fn resolve(&self, path: &str) -> Option<Value> {
        let path = path.strip_prefix("state.")?;
        let mut segments = path.split('.');
        let protocol = segments.next()?;
        let payload = self.get(protocol)?;
        let rest = segments.collect::<Vec<_>>().join(".");
        if rest.is_empty() {
            return Some(payload.clone());
        }
        resolve_path(payload, &rest)
    }
}

/// Shared handle to a session's game state. std RwLock is sufficient:
/// critical sections are tiny and never held across an await.
pub type SharedGameState = Arc<RwLock<GameState>>;

pub fn new_shared_game_state() -> SharedGameState {
    Arc::new(RwLock::new(GameState::default()))
}

/// Resolve a path against a JSON value. Segments are dot-separated; a segment
/// is either an object field, an array index, or an array selector starting
/// with `@`:
///
///   `@first` / `@last`                  first/last element
///   `@max(field)` / `@min(field)`       element with max/min numeric field
///   `@where(field, op, value)`          keep elements whose field satisfies
///                                       op/value; value may be a JSON list
///                                       (any-hit). Empty result → None.
///
/// Dot splitting respects brackets/parens so selector values may contain
/// dots (e.g. strings). Returns an owned value (selectors may allocate).
pub fn resolve_path(root: &Value, path: &str) -> Option<Value> {
    let mut value = root.clone();
    for seg in split_segments(path) {
        value = apply_segment(&value, &seg)?;
    }
    Some(value)
}

/// Split a path on dots that are not inside `()`, `[]` or quotes.
pub fn split_segments(path: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    for ch in path.chars() {
        match ch {
            '"' => {
                in_str = !in_str;
                cur.push(ch);
            }
            '.' if depth == 0 && !in_str => out.push(std::mem::take(&mut cur)),
            '(' | '[' if !in_str => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' if !in_str => {
                depth -= 1;
                cur.push(ch);
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn apply_segment(value: &Value, seg: &str) -> Option<Value> {
    if let Some(sel) = seg.strip_prefix('@') {
        return apply_selector(value, sel);
    }
    match value {
        Value::Object(map) => map.get(seg).cloned(),
        Value::Array(arr) => arr.get(seg.parse::<usize>().ok()?).cloned(),
        _ => None,
    }
}

fn apply_selector(value: &Value, sel: &str) -> Option<Value> {
    let arr = value.as_array()?;
    match sel {
        "first" => return arr.first().cloned(),
        "last" => return arr.last().cloned(),
        _ => {}
    }
    if let Some(field) = sel.strip_prefix("max(").and_then(|s| s.strip_suffix(')')) {
        return arr
            .iter()
            .max_by(|a, b| numeric_field(a, b, field, |x, y| x.partial_cmp(y)))
            .cloned();
    }
    if let Some(field) = sel.strip_prefix("min(").and_then(|s| s.strip_suffix(')')) {
        return arr
            .iter()
            .max_by(|a, b| numeric_field(a, b, field, |x, y| y.partial_cmp(x)))
            .cloned();
    }
    if let Some(args) = sel.strip_prefix("where(").and_then(|s| s.strip_suffix(')')) {
        let (field, op, expected) = parse_where_args(args)?;
        let filtered: Vec<Value> = arr
            .iter()
            .filter(|item| {
                resolve_path(item, &field)
                    .map(|actual| {
                        crate::domain::model::FieldCondition::compare_public(
                            &actual, &op, &expected,
                        )
                    })
                    .unwrap_or(false)
            })
            .cloned()
            .collect();
        if filtered.is_empty() {
            return None;
        }
        return Some(Value::Array(filtered));
    }
    None
}

fn numeric_field(
    a: &Value,
    b: &Value,
    field: &str,
    cmp: impl Fn(&f64, &f64) -> Option<std::cmp::Ordering>,
) -> std::cmp::Ordering {
    let fa = resolve_path(a, field).and_then(|v| v.as_f64());
    let fb = resolve_path(b, field).and_then(|v| v.as_f64());
    match (fa, fb) {
        (Some(x), Some(y)) => cmp(&x, &y).unwrap_or(std::cmp::Ordering::Equal),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => std::cmp::Ordering::Equal,
    }
}

/// Parse `field, op, value` — value is JSON if it starts with `[{"` or parses
/// as a scalar literal, otherwise a bare string (quotes stripped).
fn parse_where_args(args: &str) -> Option<(String, String, Value)> {
    let parts = split_top_level_commas(args);
    if parts.len() != 3 {
        return None;
    }
    let field = parts[0].trim().to_string();
    let op = parts[1].trim().to_string();
    let raw = parts[2].trim();
    let value = serde_json::from_str(raw)
        .unwrap_or_else(|_| Value::String(raw.trim_matches('"').to_string()));
    Some((field, op, value))
}

fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '"' => {
                in_str = !in_str;
                cur.push(ch);
            }
            ',' if depth == 0 && !in_str => out.push(std::mem::take(&mut cur)),
            '(' | '[' | '{' if !in_str => {
                depth += 1;
                cur.push(ch);
            }
            ')' | ']' | '}' if !in_str => {
                depth -= 1;
                cur.push(ch);
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
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
            Some(json!(50))
        );
        assert_eq!(
            state.resolve("state.S_2_C_MAILLIST_ID.MailIdTypes.0.mail_id"),
            Some(json!(7))
        );
        assert_eq!(state.resolve("state.S_2_C_UNKNOWN.x"), None);
        assert_eq!(state.resolve("S_2_C_MAILLIST_ID.mailNums"), None);
    }

    #[test]
    fn resolve_json_paths() {
        let v = json!({"a": [{"b": 1}]});
        assert_eq!(resolve_path(&v, "a.0.b"), Some(json!(1)));
        assert_eq!(resolve_path(&v, "a.1.b"), None);
        assert_eq!(resolve_path(&v, "a.b"), None);
    }

    #[test]
    fn array_selectors() {
        let v = json!({"teams": [
            {"name": "a", "server_id": "x-999", "player_count": 1},
            {"name": "b", "server_id": "wly-86-576-tz-lqw-888", "player_count": 3},
            {"name": "c", "server_id": "wly-86-576-tz-lqw-11014", "player_count": 2}
        ]});

        // @first / @max / @min
        assert_eq!(resolve_path(&v, "teams.@first.name"), Some(json!("a")));
        assert_eq!(
            resolve_path(&v, "teams.@max(player_count).name"),
            Some(json!("b"))
        );
        assert_eq!(
            resolve_path(&v, "teams.@min(player_count).name"),
            Some(json!("a"))
        );

        // @where with list value (any-hit) chained with @max
        let p = "teams.@where(server_id, ends_with, [\"-888\", \"-11014\", \"-11020\"]).@max(player_count).name";
        assert_eq!(resolve_path(&v, p), Some(json!("b")));

        // @where with single bare-string value (returns array; take @first)
        assert_eq!(
            resolve_path(&v, "teams.@where(name, eq, c).@first.server_id"),
            Some(json!("wly-86-576-tz-lqw-11014"))
        );

        // empty filter result → None
        assert_eq!(
            resolve_path(&v, "teams.@where(server_id, ends_with, [\"-555\"])"),
            None
        );

        // numeric comparison in @where
        assert_eq!(
            resolve_path(&v, "teams.@where(player_count, gte, 2).@first.name"),
            Some(json!("b"))
        );
    }
}
