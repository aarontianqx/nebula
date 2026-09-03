use super::protocol_script::FieldCondition;
use super::script::{humantime_serde, Condition, OcrRule, Point, StateRule};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

/// Unified task template (schema v2). A task is the full knowledge of one
/// automation job: an ordered list of steps, each with a match predicate and
/// an action sequence. The executor (TaskRunner) is task-agnostic.
///
/// Execution model: a state-matching loop — each iteration runs the first
/// step (template order = priority) whose predicate holds and whose `once`
/// is not yet consumed. Linear flows are the special case where every step
/// is `once`; loops (e.g. battle rounds) are the general case where a
/// predicate stays true across iterations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique identifier for this task
    pub name: String,

    /// Human-readable explanation of what the task does
    #[serde(default)]
    pub description: Option<String>,

    /// What to do when no step's predicate holds
    #[serde(default)]
    pub on_no_match: NoMatchRule,

    /// Steps in template order (= match priority)
    pub steps: Vec<TaskStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStep {
    /// Step name (used in logs and step events)
    pub name: String,

    /// When this step should run
    #[serde(default, rename = "match")]
    pub match_: MatchPredicate,

    /// Optional exact-state rule, evaluated before the actions (same
    /// semantics as in scene scripts: quit / quit_exhausted / skip)
    #[serde(default, rename = "stateRule")]
    pub state_rule: Option<StateRule>,

    /// Optional OCR-based rule (last-resort fallback for values the
    /// protocol layer does not cover)
    #[serde(default, rename = "ocrRule")]
    pub ocr_rule: Option<OcrRule>,

    /// Actions to perform in order
    #[serde(default)]
    pub actions: Vec<TaskAction>,
}

/// Step match predicate: scene recognition and/or exact-state conditions.
/// With both present, both must hold (AND).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MatchPredicate {
    /// Scene name from resources/scenes (screenshot recognition)
    #[serde(default)]
    pub scene: Option<String>,

    /// Exact-state conditions (state./role. paths); all must hold
    #[serde(default)]
    pub conditions: Vec<FieldCondition>,

    /// Run at most once per task run (linearity marker)
    #[serde(default)]
    pub once: bool,
}

/// Behavior when no step matches.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoMatchRule {
    /// quit (default): end the task as Completed; wait: keep waiting
    #[serde(default)]
    pub policy: NoMatchPolicy,

    /// For wait: give up (as Completed) after this long (default 120s)
    #[serde(default, with = "humantime_serde")]
    pub timeout: Option<Duration>,
}

impl Default for NoMatchRule {
    fn default() -> Self {
        Self {
            policy: NoMatchPolicy::Quit,
            timeout: Some(Duration::from_secs(120)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NoMatchPolicy {
    #[default]
    Quit,
    Wait,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskAction {
    /// Click at coordinates (screenshot-driven fallback)
    Click {
        #[serde(default)]
        points: Vec<Point>,
    },

    /// Drag between points or along a path
    Drag {
        #[serde(default)]
        points: Vec<Point>,
    },

    /// Wait for a duration
    Wait {
        #[serde(default, with = "humantime_serde")]
        duration: Option<Duration>,
    },

    /// Loop nested actions (single level)
    Loop {
        #[serde(default = "default_infinite")]
        count: i32,
        #[serde(default, with = "humantime_serde")]
        interval: Option<Duration>,
        /// Scene name that stops the loop when matched
        #[serde(default)]
        until: Option<String>,
        actions: Vec<TaskAction>,
    },

    /// Increment a counter
    Incr { key: String },

    /// Decrement a counter
    Decr { key: String },

    /// Quit the task; optional counter condition and reason
    Quit {
        #[serde(default)]
        condition: Option<Condition>,
        /// completed (default) or exhausted (maps to ResourceExhausted)
        #[serde(default)]
        reason: Option<QuitReason>,
    },

    /// Send a protocol message; payload strings starting with '$' are
    /// resolved as state./role. paths at send time
    SendProtocol {
        protocol: String,
        #[serde(default)]
        payload: Value,
    },

    /// Send + wait for response (+ retries on timeout); payload supports
    /// '$' path references like SendProtocol
    Request {
        protocol: String,
        #[serde(default)]
        payload: Value,
        #[serde(default)]
        expect: Option<String>,
        #[serde(default)]
        expect_any: Vec<String>,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        conditions: Vec<FieldCondition>,
        #[serde(default)]
        retries: u32,
    },

    /// Wait for a downstream protocol message
    WaitProtocol {
        protocol: String,
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        conditions: Vec<FieldCondition>,
    },

    /// Wait until state./role. conditions hold (readiness gate)
    WaitState {
        #[serde(default, with = "humantime_serde")]
        timeout: Option<Duration>,
        #[serde(default)]
        conditions: Vec<FieldCondition>,
    },

    /// Evaluate arbitrary JS in the game page (escape hatch, last resort —
    /// e.g. calling a client function like enterKnightTower())
    EvalJs { script: String },
}

fn default_infinite() -> i32 {
    -1
}

/// Why a task finished early (Quit action with reason).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuitReason {
    Completed,
    Exhausted,
}

/// Task info for frontend display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInfo {
    pub name: String,
    pub description: Option<String>,
}

impl From<&Task> for TaskInfo {
    fn from(task: &Task) -> Self {
        Self {
            name: task.name.clone(),
            description: task.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_task_yaml() {
        let yaml = r#"
name: knight_tower
description: 武魁高塔组队刷塔
on_no_match: { policy: wait, timeout: 120s }
steps:
  - name: finish
    match:
      conditions:
        - { field: role._knightTower._teamNumInfo.num, op: gte, value: 7 }
    actions:
      - { type: quit, reason: exhausted }

  - name: join_team
    match:
      once: true
      conditions:
        - { field: role._knightTower._selfteamId, op: eq, value: -1 }
    stateRule:
      conditions:
        - { field: role._militaryOrder, op: lt, value: 1 }
      action: quit_exhausted
    actions:
      - type: request
        protocol: C_2_S_KNIGHT_TOWER_TEAM_JOIN
        payload:
          ident: 1
          create_id: "$state.X.ary.@max(player_count).create_id"
        expect_any: [S_2_C_A, S_2_C_B]
        timeout: 10s
        retries: 2

  - name: fallback_click
    match:
      scene: some_popup
    actions:
      - { type: click, points: [{x: 540, y: 400}] }
      - { type: wait, duration: 500ms }
      - { type: eval_js, script: "window.x = 1" }
"#;
        let task: Task = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(task.name, "knight_tower");
        assert_eq!(task.on_no_match.policy, NoMatchPolicy::Wait);
        assert_eq!(task.steps.len(), 3);
        assert!(task.steps[1].match_.once);
        assert!(task.steps[1].state_rule.is_some());
        assert_eq!(task.steps[2].match_.scene.as_deref(), Some("some_popup"));
        match &task.steps[0].actions[0] {
            TaskAction::Quit { reason, .. } => {
                assert_eq!(*reason, Some(QuitReason::Exhausted))
            }
            other => panic!("unexpected: {:?}", other),
        }
        match &task.steps[1].actions[0] {
            TaskAction::Request {
                expect_any,
                retries,
                ..
            } => {
                assert_eq!(expect_any.len(), 2);
                assert_eq!(*retries, 2);
            }
            other => panic!("unexpected: {:?}", other),
        }
    }
}
