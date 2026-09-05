use super::game_state::resolve_path;
use super::script::{humantime_serde, Point, ScriptInfo};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

impl From<&ProtocolScript> for ScriptInfo {
    fn from(script: &ProtocolScript) -> Self {
        Self {
            name: script.name.clone(),
            description: script.description.clone(),
        }
    }
}

/// A protocol-driven automation script: an ordered, linear list of steps that
/// send/observe game protocols instead of recognizing scenes and clicking.
///
/// Unlike scene scripts (executed by ScriptRunner as a continuous scene-matching
/// loop), protocol scripts run straight through their steps once; step-level
/// `conditions` decide whether a step executes or is skipped.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolScript {
    /// Unique identifier for this script
    pub name: String,

    /// Human-readable explanation of what the script does
    #[serde(default)]
    pub description: Option<String>,

    /// Ordered execution steps, run once in sequence
    pub steps: Vec<ProtocolStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolStep {
    /// Step name (used in logs and step events)
    pub name: String,

    /// Guards on the structured game state; all must hold for the step to run.
    /// Field paths start with `state.`, e.g. `state.S_2_C_MAILLIST_ID.mailNums`.
    /// A step with unmet conditions is skipped (not an error).
    #[serde(default)]
    pub conditions: Vec<FieldCondition>,

    /// Actions to perform in order
    #[serde(default)]
    pub actions: Vec<ProtocolAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProtocolAction {
    /// Send a protocol message by registry name (e.g. C_2_S_MAIL_INFO).
    /// Framing/encryption/encoding are done by the game itself.
    SendProtocol {
        protocol: String,
        #[serde(default)]
        payload: Value,
    },

    /// Wait for a downstream protocol message. With `conditions`, waits until
    /// a message of this protocol arrives whose fields satisfy all of them
    /// (paths are relative to the message payload, e.g. `err_code`).
    WaitProtocol {
        protocol: String,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        conditions: Vec<FieldCondition>,
    },

    /// Send a protocol message and wait for its response, as one atomic unit.
    /// On timeout the request is retried (re-sent) up to `retries` times —
    /// this is the robust way to talk to the server right after login, when
    /// the entry handshake may still be in flight and early packets can be
    /// dropped silently.
    ///
    /// `expect` names a single response protocol; `expect_any` names several
    /// acceptable ones (the server acknowledges some requests with different
    /// messages depending on content — e.g. mail draw-all returns either its
    /// dedicated ack or a generic resource push). `conditions` apply to
    /// whichever message matched.
    Request {
        protocol: String,
        #[serde(default)]
        payload: Value,
        /// Expected response protocol name
        #[serde(default)]
        expect: Option<String>,
        /// Acceptable response protocol names (alternative to `expect`)
        #[serde(default)]
        expect_any: Vec<String>,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        /// Conditions on the response payload (e.g. `err_code == 0`)
        #[serde(default)]
        conditions: Vec<FieldCondition>,
        /// Additional attempts after the first timeout (0 = no retry)
        #[serde(default)]
        retries: u32,
    },

    /// Wait until all conditions on the structured game state hold.
    /// Use for readiness gates, e.g. waiting until some state field appears
    /// or reaches a value (paths start with `state.`).
    WaitState {
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        conditions: Vec<FieldCondition>,
    },

    /// Wait for a duration
    Wait {
        #[serde(default, with = "humantime_serde")]
        duration: Option<Duration>,
    },

    /// Fallback: click at coordinates (for UI only reachable via canvas)
    Click {
        #[serde(default)]
        points: Vec<Point>,
    },

    /// Fallback: drag between points or along a path
    Drag {
        #[serde(default)]
        points: Vec<Point>,
    },
}

/// A comparison against a numeric/bool/string field of a JSON document.
/// `gt/gte/lt/lte` require both sides numeric; `eq/neq` also work for
/// strings and bools; `contains/ends_with` are string ops. `exists` holds
/// when the path resolves, `missing` when it does not. A list `value` means
/// any-hit for eq/contains/ends_with.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCondition {
    /// Dotted field path (see GameState::resolve / resolve_path)
    pub field: String,

    /// Comparison operator (eq, neq, gt, gte, lt, lte, exists, missing,
    /// contains, ends_with)
    pub op: String,

    /// Value to compare against
    #[serde(default)]
    pub value: Value,

    /// Substitute for the actual value when the field path does not resolve
    /// (e.g. a purchase-count entry that only exists after the first buy).
    /// Ignored by the `missing` op. Use only when absence has a safe meaning
    /// — an unresolvable path otherwise fails the condition.
    #[serde(default)]
    pub default: Option<Value>,
}

impl FieldCondition {
    /// Evaluate against a JSON document root (e.g. one protocol payload).
    pub fn evaluate(&self, root: &Value) -> bool {
        if self.op == "missing" {
            // null is the game client's "no value" (e.g. post-battle model
            // reset) — it must count as missing, or refresh logic keyed on
            // `missing` never refires.
            return resolve_path(root, &self.field)
                .filter(|v| !v.is_null())
                .is_none();
        }
        let Some(actual) = resolve_path(root, &self.field).or_else(|| self.default.clone()) else {
            return false;
        };
        self.evaluate_value(&actual)
    }

    /// Evaluate against an already-resolved value (e.g. from GameState::resolve).
    pub fn evaluate_value(&self, actual: &Value) -> bool {
        Self::compare(actual, &self.op, &self.value)
    }

    /// Evaluate with both sides already resolved — used when `value` was a
    /// `$`-prefixed reference to another field that the caller substituted.
    pub fn evaluate_with(&self, actual: &Value, expected: &Value) -> bool {
        Self::compare(actual, &self.op, expected)
    }

    /// Raw three-way comparison, exposed for array-selector filtering.
    pub(crate) fn compare_public(actual: &Value, op: &str, expected: &Value) -> bool {
        Self::compare(actual, op, expected)
    }

    fn compare(actual: &Value, op: &str, expected: &Value) -> bool {
        if op == "exists" {
            // Only reached when the path resolved; a resolved null still
            // means "no value" (client reset), so it is not "exists".
            return !actual.is_null();
        }
        // List expected value: any-hit semantics for equality/string ops.
        if let Value::Array(items) = expected {
            if matches!(op, "eq" | "contains" | "ends_with") {
                return items.iter().any(|e| Self::compare(actual, op, e));
            }
            return false;
        }
        let numeric = actual.as_f64().zip(expected.as_f64());
        match (op, numeric) {
            ("eq", Some((a, b))) => a == b,
            ("neq", Some((a, b))) => a != b,
            ("gt", Some((a, b))) => a > b,
            ("gte", Some((a, b))) => a >= b,
            ("lt", Some((a, b))) => a < b,
            ("lte", Some((a, b))) => a <= b,
            // Non-numeric equality (strings, bools, null)
            ("eq", None) => actual == expected,
            ("neq", None) => actual != expected,
            ("contains", None) => actual
                .as_str()
                .zip(expected.as_str())
                .map(|(a, b)| a.contains(b))
                .unwrap_or(false),
            ("ends_with", None) => actual
                .as_str()
                .zip(expected.as_str())
                .map(|(a, b)| a.ends_with(b))
                .unwrap_or(false),
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn field_condition_evaluation() {
        let doc = json!({"mailNums": 50, "err_code": 0, "nested": {"flag": true}});

        let cond = |field: &str, op: &str, value: Value| FieldCondition {
            field: field.to_string(),
            op: op.to_string(),
            value,
            default: None,
        };

        assert!(cond("mailNums", "gt", json!(0)).evaluate(&doc));
        assert!(!cond("mailNums", "lt", json!(50)).evaluate(&doc));
        assert!(cond("err_code", "eq", json!(0)).evaluate(&doc));
        assert!(cond("nested.flag", "eq", json!(true)).evaluate(&doc));
        assert!(!cond("missing", "eq", json!(0)).evaluate(&doc));
        assert!(!cond("mailNums", "bogus", json!(0)).evaluate(&doc));
    }

    /// `default` substitutes when the path does not resolve (e.g. a buy-count
    /// entry that only exists after the first purchase); a resolved value
    /// ignores the default.
    #[test]
    fn field_condition_default_substitution() {
        let doc = json!({"bought": 3});
        let mut c = FieldCondition {
            field: "absent".to_string(),
            op: "lt".to_string(),
            value: json!(5),
            default: Some(json!(0)),
        };
        assert!(c.evaluate(&doc)); // absent → default 0 < 5
        c.field = "bought".to_string();
        assert!(c.evaluate(&doc)); // resolved 3 < 5
        c.value = json!(3);
        assert!(!c.evaluate(&doc)); // resolved 3, not < 3
                                    // Without a default, an unresolvable path fails as before.
        c.default = None;
        c.field = "absent".to_string();
        c.value = json!(5);
        assert!(!c.evaluate(&doc));
    }

    #[test]
    fn parse_protocol_script_yaml() {
        let yaml = r#"
name: claim_all_mail
description: 一键领取全部邮件附件
steps:
  - name: fetch
    actions:
      - type: send_protocol
        protocol: C_2_S_MAIL_INFO
      - type: wait_protocol
        protocol: S_2_C_MAILLIST_ID
        timeout: 10s
  - name: draw
    conditions:
      - {field: state.S_2_C_MAILLIST_ID.mailNums, op: gt, value: 0}
    actions:
      - type: send_protocol
        protocol: C_2_S_MAIL_DRAW_ALL_REWARD
        payload: {}
      - type: wait_protocol
        protocol: S_2_C_MAIL_DRAW_ALL_REWARD
        timeout: 10s
        conditions:
          - {field: err_code, op: eq, value: 0}
"#;
        let script: ProtocolScript = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(script.name, "claim_all_mail");
        assert_eq!(script.steps.len(), 2);
        assert_eq!(script.steps[1].conditions.len(), 1);
        match &script.steps[0].actions[1] {
            ProtocolAction::WaitProtocol {
                protocol, timeout, ..
            } => {
                assert_eq!(protocol, "S_2_C_MAILLIST_ID");
                assert_eq!(*timeout, Some(Duration::from_secs(10)));
            }
            other => panic!("unexpected action: {:?}", other),
        }
    }
}
