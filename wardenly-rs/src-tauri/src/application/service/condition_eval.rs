use std::sync::Arc;

use crate::domain::model::{FieldCondition, SharedGameState};
use crate::infrastructure::browser::BrowserDriver;
use serde_json::Value;

/// Evaluate exact-state conditions shared by both script engines.
///
/// Field path prefixes:
///   - `state.<PROTO>.<field>` — latest payload of that protocol (GameState,
///     fed by the bridge push stream);
///   - `role.<field>` — the game's own client role model, queried live
///     through the page bridge (always current, no push required).
///
/// A condition's `value` may itself be a reference: a string starting with
/// `$` is resolved as another path (e.g. `"$role._militaryOrder"`).
///
/// Unresolvable paths, unknown prefixes, and bridge errors all count as
/// unmet (fail-open: the rule simply doesn't trigger and may be re-checked
/// on a later iteration).
pub async fn conditions_met(
    conditions: &[FieldCondition],
    game_state: &SharedGameState,
    browser: &Arc<dyn BrowserDriver>,
    any: bool,
) -> bool {
    for condition in conditions {
        let met = condition_met(condition, game_state, browser).await;
        if any && met {
            return true;
        }
        if !any && !met {
            return false;
        }
    }
    // Vacuous truth for AND (empty list); never for OR.
    !any
}

async fn condition_met(
    condition: &FieldCondition,
    game_state: &SharedGameState,
    browser: &Arc<dyn BrowserDriver>,
) -> bool {
    // `missing` holds exactly when the path does not resolve.
    if condition.op == "missing" {
        return resolve_path(&condition.field, game_state, browser)
            .await
            .is_none();
    }
    let Some(actual) = resolve_path(&condition.field, game_state, browser).await else {
        tracing::debug!("Condition field unresolved: {}", condition.field);
        return false;
    };
    let Some(expected) = resolve_expected(condition, game_state, browser).await else {
        tracing::debug!("Condition expected-value unresolved: {}", condition.field);
        return false;
    };
    condition.evaluate_with(&actual, &expected)
}

/// Resolve the expected side of a condition: a `$`-prefixed string is a path
/// reference; anything else is a literal.
async fn resolve_expected(
    condition: &FieldCondition,
    game_state: &SharedGameState,
    browser: &Arc<dyn BrowserDriver>,
) -> Option<Value> {
    if let Value::String(s) = &condition.value {
        if let Some(path) = s.strip_prefix('$') {
            return resolve_path(path, game_state, browser).await;
        }
    }
    Some(condition.value.clone())
}

/// Resolve a `state.`/`role.` path to a concrete JSON value.
async fn resolve_path(
    path: &str,
    game_state: &SharedGameState,
    browser: &Arc<dyn BrowserDriver>,
) -> Option<Value> {
    if path.starts_with("state.") {
        let state = match game_state.read() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        return state.resolve(path);
    }
    if let Some(role_path) = path.strip_prefix("role.") {
        return query_role(role_path, browser).await;
    }
    tracing::warn!(
        "Condition field must start with 'state.' or 'role.': {}",
        path
    );
    None
}

/// Query the client role model through the page bridge. The bridge only
/// understands plain field/index navigation; array selectors (`@...`) are
/// resolved here: the plain prefix is fetched from the page, then the
/// selector suffix is applied locally.
async fn query_role(path: &str, browser: &Arc<dyn BrowserDriver>) -> Option<Value> {
    use crate::domain::model::game_state as gs;

    let segments = gs::split_segments(path);
    let selector_at = segments.iter().position(|s| s.starts_with('@'));
    let (plain, suffix) = match selector_at {
        Some(i) => (segments[..i].join("."), Some(segments[i..].join("."))),
        None => (path.to_string(), None),
    };
    if plain.is_empty() {
        return None;
    }

    let path_literal = serde_json::to_string(&plain).ok()?;
    let script = format!(
        "window.__wardenly ? window.__wardenly.queryRole({}) : 'ERR bridge not installed'",
        path_literal
    );
    let raw = browser.evaluate(&script).await.ok()?;
    let outer: Value = serde_json::from_str(&raw).ok()?;
    let inner = outer.as_str()?;
    if inner.starts_with("ERR") {
        tracing::debug!("queryRole({}) -> {}", plain, inner);
        return None;
    }
    let parsed: Value = serde_json::from_str(inner).ok()?;
    let value = parsed.get("value").cloned()?;

    match suffix {
        None => Some(value),
        Some(suffix) => gs::resolve_path(&value, &suffix),
    }
}

/// Resolve `$`-prefixed references inside a payload (used by send/request
/// actions). Any string value starting with `$` is replaced by the resolved
/// state./role. value; resolution failure aborts the whole payload (None) —
/// sending with a wrong id is worse than failing loudly.
pub async fn resolve_payload_refs(
    payload: &Value,
    game_state: &SharedGameState,
    browser: &Arc<dyn BrowserDriver>,
) -> Option<Value> {
    match payload {
        Value::String(s) if s.starts_with('$') => resolve_path(&s[1..], game_state, browser).await,
        Value::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(Box::pin(resolve_payload_refs(item, game_state, browser)).await?);
            }
            Some(Value::Array(out))
        }
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                out.insert(
                    k.clone(),
                    Box::pin(resolve_payload_refs(v, game_state, browser)).await?,
                );
            }
            Some(Value::Object(out))
        }
        _ => Some(payload.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::model::new_shared_game_state;
    use serde_json::json;

    struct MockDriver {
        response: String,
    }

    #[async_trait::async_trait]
    impl BrowserDriver for MockDriver {
        async fn start(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn stop(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn navigate(&self, _url: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn click(&self, _x: f64, _y: f64) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn drag(&self, _from: (f64, f64), _to: (f64, f64)) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn drag_path(
            &self,
            _points: &[crate::infrastructure::browser::BrowserPoint],
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn start_screencast(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn stop_screencast(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn evaluate(&self, _script: &str) -> anyhow::Result<String> {
            // Mirror what the real driver returns: the JS return value
            // JSON-serialized (a JSON string literal containing JSON text).
            Ok(serde_json::to_string(&self.response).unwrap())
        }
        async fn install_page_bridge(
            &self,
            _binding_name: &str,
            _init_script: &str,
        ) -> anyhow::Result<tokio::sync::mpsc::Receiver<String>> {
            unimplemented!()
        }
        async fn capture_screen(&self) -> anyhow::Result<image::DynamicImage> {
            unimplemented!()
        }
        async fn input_text(&self, _selector: &str, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn click_element(&self, _selector: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn wait_visible(
            &self,
            _selector: &str,
            _timeout: std::time::Duration,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn login_with_password(
            &self,
            _username: &str,
            _password: &str,
            _timeout: std::time::Duration,
        ) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn refresh(&self) -> anyhow::Result<()> {
            unimplemented!()
        }
        async fn insert_text(&self, _text: &str) -> anyhow::Result<()> {
            unimplemented!()
        }
    }

    fn mock_browser(role_value: Value) -> Arc<dyn BrowserDriver> {
        Arc::new(MockDriver {
            response: serde_json::json!({"ok": true, "value": role_value}).to_string(),
        })
    }

    fn cond(field: &str, op: &str, value: Value) -> FieldCondition {
        FieldCondition {
            field: field.to_string(),
            op: op.to_string(),
            value,
        }
    }

    #[tokio::test]
    async fn role_path_conditions() {
        let game_state = new_shared_game_state();
        let browser = mock_browser(json!(7));

        let conds = vec![cond("role._knightTower._teamNumInfo.num", "gte", json!(7))];
        assert!(conditions_met(&conds, &game_state, &browser, false).await);
    }

    #[tokio::test]
    async fn state_path_conditions() {
        let game_state = new_shared_game_state();
        game_state
            .write()
            .unwrap()
            .update("S_2_C_MAILLIST_ID", json!({"mailNums": 50}));
        let browser = mock_browser(json!(null));

        let conds = vec![cond("state.S_2_C_MAILLIST_ID.mailNums", "gt", json!(0))];
        assert!(conditions_met(&conds, &game_state, &browser, false).await);

        let conds = vec![cond("state.S_2_C_MAILLIST_ID.missing", "gt", json!(0))];
        assert!(!conditions_met(&conds, &game_state, &browser, false).await);
    }

    #[tokio::test]
    async fn dollar_reference_comparison() {
        let game_state = new_shared_game_state();
        // actual comes from the mock (7); expected is a $-ref that resolves
        // through the same mock (7) → gte holds
        let browser = mock_browser(json!(7));
        let conds = vec![cond(
            "role._knightTower._teamNumInfo.num",
            "gte",
            json!("$role._militaryOrder"),
        )];
        assert!(conditions_met(&conds, &game_state, &browser, false).await);
    }

    #[tokio::test]
    async fn any_semantics() {
        let game_state = new_shared_game_state();
        let browser = mock_browser(json!(5));
        // field resolves (mock → 5); one condition holds (gte 3), one doesn't (gte 7)
        let conds = vec![
            cond("role.x", "gte", json!(3)),
            cond("role.x", "gte", json!(7)),
        ];
        assert!(conditions_met(&conds, &game_state, &browser, true).await);
        assert!(!conditions_met(&conds, &game_state, &browser, false).await);
    }

    #[tokio::test]
    async fn bridge_error_is_unmet() {
        let game_state = new_shared_game_state();
        let browser: Arc<dyn BrowserDriver> = Arc::new(MockDriver {
            response: "ERR unresolved: _nope".to_string(),
        });
        let conds = vec![cond("role._nope", "exists", json!(null))];
        assert!(!conditions_met(&conds, &game_state, &browser, false).await);
    }
}
