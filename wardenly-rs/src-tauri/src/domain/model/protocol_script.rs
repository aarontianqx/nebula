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
/// strings and bools. A missing field always evaluates to false.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldCondition {
    /// Dotted field path (see GameState::resolve / resolve_path)
    pub field: String,

    /// Comparison operator (eq, neq, gt, gte, lt, lte, exists).
    /// `exists` ignores `value` and holds when the path resolves.
    pub op: String,

    /// Value to compare against
    #[serde(default)]
    pub value: Value,
}

impl FieldCondition {
    /// Evaluate against a JSON document root (e.g. one protocol payload).
    pub fn evaluate(&self, root: &Value) -> bool {
        let Some(actual) = resolve_path(root, &self.field) else {
            return false;
        };
        self.evaluate_value(actual)
    }

    /// Evaluate against an already-resolved value (e.g. from GameState::resolve).
    pub fn evaluate_value(&self, actual: &Value) -> bool {
        Self::compare(actual, &self.op, &self.value)
    }

    fn compare(actual: &Value, op: &str, expected: &Value) -> bool {
        if op == "exists" {
            return true; // only reached when the path resolved
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
        };

        assert!(cond("mailNums", "gt", json!(0)).evaluate(&doc));
        assert!(!cond("mailNums", "lt", json!(50)).evaluate(&doc));
        assert!(cond("err_code", "eq", json!(0)).evaluate(&doc));
        assert!(cond("nested.flag", "eq", json!(true)).evaluate(&doc));
        assert!(!cond("missing", "eq", json!(0)).evaluate(&doc));
        assert!(!cond("mailNums", "bogus", json!(0)).evaluate(&doc));
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
