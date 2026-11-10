//! End-to-end integration tests for the playback engine.
//!
//! These tests drive a real [`Player`] thread through its public command/event
//! API, but substitute the platform boundaries with in-memory mocks:
//!
//! - [`RecordingExecutor`] records every action that reaches the injector so we
//!   can assert exactly which concrete actions were "performed" (and in order).
//! - [`MockPlatform`] provides deterministic window/pixel answers so condition
//!   evaluation never touches the real OS.
//!
//! The engine consumes a [`MacroDocument`] directly, so these tests also cover the Resolve
//! stage: `{{ var }}` / `{{ expr }}` and run-time overrides are substituted *before* injection,
//! and `call_macro` is expanded inline with cycle protection and an isolated child scope.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tap_application::{
    ActionExecutor, EngineCommand, EngineEvent, EngineState, PlatformConditionProvider, Player,
    PlayerHandle, SubMacroLoader,
};
use tap_core::{
    Action, DslAction, DslCondition, DslMouseButton, DslRunConfig, DslTimedAction, DslValue,
    MacroDocument, MouseButton, VariableDefinition, VariableType, VariableValue, DSL_VERSION,
};

// === Mocks ===

/// Records every action delegated to the injector.
struct RecordingExecutor {
    injected: Arc<Mutex<Vec<Action>>>,
}

impl ActionExecutor for RecordingExecutor {
    fn execute(&self, action: &Action) -> Result<(), String> {
        self.injected.lock().unwrap().push(action.clone());
        Ok(())
    }
}

/// Executor that always fails, counting how many times it was invoked.
struct FailingExecutor {
    attempts: Arc<Mutex<usize>>,
}

impl ActionExecutor for FailingExecutor {
    fn execute(&self, _action: &Action) -> Result<(), String> {
        *self.attempts.lock().unwrap() += 1;
        Err("boom".to_string())
    }
}

/// Deterministic platform stub: windows always present/focused, no pixels.
struct MockPlatform;

impl PlatformConditionProvider for MockPlatform {
    fn is_window_focused(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn window_exists(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn get_pixel_color(&self, _x: i32, _y: i32) -> Option<tap_core::ConditionColor> {
        None
    }
    fn window_origin(&self, _title: Option<&str>, _process: Option<&str>) -> Option<(i32, i32)> {
        None
    }
}

// === Builders ===

/// Concrete (resolved) click, used for assertions on injected actions.
fn click(x: i32, y: i32) -> Action {
    Action::Click {
        x,
        y,
        button: MouseButton::Left,
    }
}

/// DSL click with literal integer coordinates.
fn dsl_click(x: i64, y: i64) -> DslAction {
    DslAction::Click {
        x: DslValue::Int(x),
        y: DslValue::Int(y),
        button: DslMouseButton::Left,
    }
}

fn dsl_key(k: &str) -> DslAction {
    DslAction::KeyTap { key: k.to_string() }
}

fn timed(action: DslAction) -> DslTimedAction {
    DslTimedAction {
        at_ms: 0,
        action,
        enabled: true,
        note: None,
    }
}

/// Build a document whose actions all fire immediately (at_ms = 0) with no
/// arming countdown, so tests run in milliseconds.
fn document(timeline: Vec<DslTimedAction>, repeat: u32) -> MacroDocument {
    MacroDocument {
        name: "test".to_string(),
        description: None,
        version: DSL_VERSION.to_string(),
        author: None,
        tags: Vec::new(),
        variables: HashMap::new(),
        target_window: None,
        timeline,
        run: DslRunConfig {
            repeat,
            start_delay_ms: 0,
            speed: 1.0,
            humanize: Default::default(),
        },
    }
}

// === Harness ===

fn spawn_recording(loader: Option<SubMacroLoader>) -> (PlayerHandle, Arc<Mutex<Vec<Action>>>) {
    let injected = Arc::new(Mutex::new(Vec::new()));
    let executor = RecordingExecutor {
        injected: injected.clone(),
    };
    let handle = match loader {
        Some(loader) => Player::spawn_with_loader(executor, MockPlatform, loader),
        None => Player::spawn(executor, MockPlatform),
    };
    (handle, injected)
}

/// Pump events until `Completed` or timeout. Returns `(completed, all_events)`.
fn pump_until_complete(handle: &PlayerHandle, timeout: Duration) -> (bool, Vec<EngineEvent>) {
    let mut completed = false;
    let mut events = Vec::new();
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        while let Some(event) = handle.try_recv() {
            if matches!(event, EngineEvent::Completed) {
                completed = true;
            }
            events.push(event);
        }
        if completed {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    (completed, events)
}

/// Run a document to completion with no run-time overrides and the default
/// (disk) sub-macro loader. Returns `(injected_actions, completed)`.
///
/// NOTE: we intentionally drop the handle rather than calling `shutdown()`. The
/// player loops back to `recv()` after a Stop, so `shutdown()` (which sends Stop
/// then joins while still holding the sender) would block forever. Dropping
/// closes the command channel and lets the thread exit.
fn run_until_complete(doc: MacroDocument, timeout: Duration) -> (Vec<Action>, bool) {
    let (handle, injected) = spawn_recording(None);
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, timeout);
    let actions = injected.lock().unwrap().clone();
    drop(handle);
    (actions, completed)
}

// === Tests: scheduling / dispatch ===

#[test]
fn repeats_inject_every_action_each_iteration() {
    let doc = document(
        vec![timed(dsl_click(1, 1)), timed(dsl_key("a"))],
        2, // repeat twice
    );

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(
        completed,
        "engine should report Completed for finite repeat"
    );
    assert_eq!(
        injected,
        vec![
            click(1, 1),
            Action::KeyTap { key: "a".into() },
            click(1, 1),
            Action::KeyTap { key: "a".into() },
        ],
        "every enabled action should be injected once per iteration, in order"
    );
}

#[test]
fn control_actions_are_not_sent_to_the_injector() {
    // Wait and counter actions are handled inside the engine and must never
    // reach the injector; only Click and KeyTap should.
    let doc = document(
        vec![
            timed(dsl_click(5, 5)),
            timed(DslAction::Wait { ms: 5 }),
            timed(DslAction::SetCounter {
                key: "c".to_string(),
                value: DslValue::Int(1),
            }),
            timed(dsl_key("b")),
        ],
        1,
    );

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(
        injected,
        vec![click(5, 5), Action::KeyTap { key: "b".into() }]
    );
}

#[test]
fn disabled_actions_are_skipped() {
    let mut disabled = timed(dsl_click(2, 2));
    disabled.enabled = false;

    let doc = document(vec![timed(dsl_click(1, 1)), disabled], 1);

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(injected, vec![click(1, 1)]);
}

#[test]
fn conditional_takes_then_branch_based_on_counter() {
    // Set counter c=3, then a conditional that injects (7,7) when c == 3,
    // otherwise (8,8). The "then" branch must be taken.
    let conditional = DslAction::Conditional {
        condition: DslCondition::Counter {
            key: "c".to_string(),
            op: "==".to_string(),
            value: 3,
        },
        then_action: Box::new(dsl_click(7, 7)),
        else_action: Some(Box::new(dsl_click(8, 8))),
    };

    let doc = document(
        vec![
            timed(DslAction::SetCounter {
                key: "c".to_string(),
                value: DslValue::Int(3),
            }),
            timed(conditional),
        ],
        1,
    );

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(injected, vec![click(7, 7)]);
}

// === Tests: Resolve stage ===

#[test]
fn runtime_override_beats_default_and_expressions_are_evaluated() {
    // `base_x` defaults to 100 but is overridden to 200 at run time. The click
    // coordinates are expressions referencing it, so the injected action must
    // carry the resolved literals (200, 205) — proving override > default and
    // that `{{ expr }}` is evaluated before injection.
    let mut variables = HashMap::new();
    variables.insert(
        "base_x".to_string(),
        VariableDefinition {
            var_type: VariableType::Number,
            default: Some(serde_json::json!(100)),
            description: None,
        },
    );

    let mut doc = document(
        vec![timed(DslAction::Click {
            x: DslValue::String("{{ base_x }}".to_string()),
            y: DslValue::String("{{ base_x + 5 }}".to_string()),
            button: DslMouseButton::Left,
        })],
        1,
    );
    doc.variables = variables;

    let (handle, injected) = spawn_recording(None);
    let mut overrides = HashMap::new();
    overrides.insert("base_x".to_string(), VariableValue::Number(200.0));
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::SetRuntimeVars(overrides));
    handle.send(EngineCommand::Start);

    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(actions, vec![click(200, 205)]);
}

#[test]
fn text_input_resolves_string_variable() {
    let mut variables = HashMap::new();
    variables.insert(
        "name".to_string(),
        VariableDefinition {
            var_type: VariableType::String,
            default: Some(serde_json::json!("Alice")),
            description: None,
        },
    );

    let mut doc = document(
        vec![timed(DslAction::TextInput {
            text: DslValue::String("{{ name }}".to_string()),
        })],
        1,
    );
    doc.variables = variables;

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(
        injected,
        vec![Action::TextInput {
            text: "Alice".into()
        }]
    );
}

// === Tests: call_macro expansion ===

/// Build an in-memory loader from a name → document map.
fn map_loader(macros: Vec<(&str, MacroDocument)>) -> SubMacroLoader {
    let map: HashMap<String, MacroDocument> = macros
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
    Arc::new(move |name: &str| {
        map.get(name)
            .cloned()
            .ok_or_else(|| format!("macro not found: {name}"))
    })
}

#[test]
fn call_macro_expands_child_timeline_inline() {
    let child = document(vec![timed(dsl_click(7, 7))], 1);
    let loader = map_loader(vec![("child", child)]);

    let parent = document(
        vec![
            timed(dsl_click(1, 1)),
            timed(DslAction::CallMacro {
                name: "child".to_string(),
                args: HashMap::new(),
            }),
            timed(dsl_click(2, 2)),
        ],
        1,
    );

    let (handle, injected) = spawn_recording(Some(loader));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(
        actions,
        vec![click(1, 1), click(7, 7), click(2, 2)],
        "child timeline should be expanded inline between the parent's actions"
    );
}

#[test]
fn call_macro_passes_args_into_child_scope() {
    // The child resolves `{{ who }}`; the parent supplies it as a call arg.
    let child = document(
        vec![timed(DslAction::TextInput {
            text: DslValue::String("{{ who }}".to_string()),
        })],
        1,
    );
    let loader = map_loader(vec![("greet", child)]);

    let mut args = HashMap::new();
    args.insert("who".to_string(), DslValue::String("world".to_string()));
    let parent = document(
        vec![timed(DslAction::CallMacro {
            name: "greet".to_string(),
            args,
        })],
        1,
    );

    let (handle, injected) = spawn_recording(Some(loader));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(
        actions,
        vec![Action::TextInput {
            text: "world".into()
        }]
    );
}

#[test]
fn call_macro_args_are_evaluated_against_the_callers_scope() {
    // The parent bumps a counter, then passes an *expression* over it; the child clicks there.
    let child = document(
        vec![timed(DslAction::Click {
            x: DslValue::String("{{ x }}".to_string()),
            y: DslValue::Int(3),
            button: DslMouseButton::Left,
        })],
        1,
    );
    let loader = map_loader(vec![("child", child)]);

    let mut args = HashMap::new();
    args.insert(
        "x".to_string(),
        DslValue::String("{{ n * 10 + 5 }}".to_string()),
    );
    let parent = document(
        vec![
            timed(DslAction::IncrCounter { key: "n".into() }),
            timed(DslAction::CallMacro {
                name: "child".to_string(),
                args,
            }),
        ],
        1,
    );

    let (handle, injected) = spawn_recording(Some(loader));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(actions, vec![click(15, 3)]);
}

#[test]
fn call_macro_with_an_unresolvable_arg_reports_it_and_skips_only_that_call() {
    let child = document(vec![timed(dsl_click(7, 7))], 1);
    let loader = map_loader(vec![("child", child)]);

    let mut args = HashMap::new();
    args.insert("x".to_string(), DslValue::String("{{ 1 +* }}".to_string()));
    let parent = document(
        vec![
            timed(DslAction::CallMacro {
                name: "child".to_string(),
                args,
            }),
            timed(dsl_click(2, 2)),
        ],
        1,
    );

    let (handle, injected) = spawn_recording(Some(loader));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, events) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(
        actions,
        vec![click(2, 2)],
        "only the broken call is skipped"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            EngineEvent::Error { message } if message.contains("argument `x`")
        )),
        "the error must name the argument: {events:?}"
    );
}

#[test]
fn call_macro_cycle_is_detected_and_does_not_hang() {
    // "loop" injects a click then calls itself. The cycle guard must stop the
    // recursion (emitting an Error) without hanging, and the click should fire
    // exactly once before the self-call is rejected.
    let looping = document(
        vec![
            timed(dsl_click(9, 9)),
            timed(DslAction::CallMacro {
                name: "loop".to_string(),
                args: HashMap::new(),
            }),
        ],
        1,
    );
    let loader = map_loader(vec![("loop", looping)]);

    let parent = document(
        vec![timed(DslAction::CallMacro {
            name: "loop".to_string(),
            args: HashMap::new(),
        })],
        1,
    );

    let (handle, injected) = spawn_recording(Some(loader));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, events) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed, "a cyclic call must still terminate the run");
    assert_eq!(
        actions,
        vec![click(9, 9)],
        "the click should fire once before the self-call is rejected"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::Error { .. })),
        "a cycle should surface as an Error event"
    );
}

// === Tests: call_macro inside an `if` ===

fn call(name: &str) -> DslAction {
    DslAction::CallMacro {
        name: name.to_string(),
        args: HashMap::new(),
    }
}

fn when(
    condition: DslCondition,
    then_action: DslAction,
    else_action: Option<DslAction>,
) -> DslAction {
    DslAction::Conditional {
        condition,
        then_action: Box::new(then_action),
        else_action: else_action.map(Box::new),
    }
}

/// Run `parent` with the given saved macros and return what was injected plus the events.
fn run_with_macros(
    parent: MacroDocument,
    macros: Vec<(&str, MacroDocument)>,
) -> (Vec<Action>, Vec<EngineEvent>, bool) {
    let (handle, injected) = spawn_recording(Some(map_loader(macros)));
    handle.send(EngineCommand::SetDocument(Box::new(parent)));
    handle.send(EngineCommand::Start);
    let (completed, events) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);
    (actions, events, completed)
}

#[test]
fn call_macro_in_the_then_branch_runs_when_the_condition_holds() {
    let child = document(vec![timed(dsl_click(7, 7))], 1);
    let parent = document(
        vec![
            timed(when(DslCondition::Always, call("child"), None)),
            timed(dsl_click(2, 2)),
        ],
        1,
    );

    let (actions, _, completed) = run_with_macros(parent, vec![("child", child)]);

    assert!(completed);
    assert_eq!(actions, vec![click(7, 7), click(2, 2)]);
}

#[test]
fn call_macro_in_the_else_branch_runs_when_the_condition_fails() {
    let then_child = document(vec![timed(dsl_click(7, 7))], 1);
    let else_child = document(vec![timed(dsl_click(8, 8))], 1);
    let parent = document(
        vec![timed(when(
            DslCondition::Never,
            call("then_child"),
            Some(call("else_child")),
        ))],
        1,
    );

    let (actions, _, completed) = run_with_macros(
        parent,
        vec![("then_child", then_child), ("else_child", else_child)],
    );

    assert!(completed);
    assert_eq!(actions, vec![click(8, 8)], "only the branch taken may run");
}

#[test]
fn a_branch_that_is_not_taken_is_not_resolved() {
    // The untaken branch holds an expression that cannot be evaluated: it must not fail the run.
    let broken = DslAction::Click {
        x: DslValue::String("{{ 1 + }}".to_string()),
        y: DslValue::Int(0),
        button: DslMouseButton::Left,
    };
    let parent = document(
        vec![timed(when(
            DslCondition::Always,
            dsl_click(3, 3),
            Some(broken),
        ))],
        1,
    );

    let (actions, events, completed) = run_with_macros(parent, vec![]);

    assert!(completed);
    assert_eq!(actions, vec![click(3, 3)]);
    assert!(
        !events
            .iter()
            .any(|e| matches!(e, EngineEvent::Error { .. })),
        "nothing was wrong with the branch that ran: {events:?}"
    );
}

#[test]
fn call_macro_in_an_if_nested_in_an_if_runs() {
    let child = document(vec![timed(dsl_click(7, 7))], 1);
    let inner = when(DslCondition::Always, call("child"), None);
    let parent = document(vec![timed(when(DslCondition::Always, inner, None))], 1);

    let (actions, _, completed) = run_with_macros(parent, vec![("child", child)]);

    assert!(completed);
    assert_eq!(actions, vec![click(7, 7)]);
}

#[test]
fn a_macro_called_from_a_branch_sees_the_args_and_leaves_the_callers_scope_alone() {
    let child = document(
        vec![timed(DslAction::TextInput {
            text: DslValue::String("{{ who }}".to_string()),
        })],
        1,
    );
    let mut args = HashMap::new();
    args.insert("who".to_string(), DslValue::String("world".to_string()));
    let parent = document(
        vec![timed(when(
            DslCondition::Always,
            DslAction::CallMacro {
                name: "greet".to_string(),
                args,
            },
            None,
        ))],
        1,
    );

    let (actions, _, completed) = run_with_macros(parent, vec![("greet", child)]);

    assert!(completed);
    assert_eq!(
        actions,
        vec![Action::TextInput {
            text: "world".into()
        }]
    );
}

#[test]
fn exit_in_a_macro_called_from_a_branch_only_ends_that_macro() {
    let child = document(
        vec![
            timed(dsl_click(5, 5)),
            timed(DslAction::Exit),
            timed(dsl_click(6, 6)),
        ],
        1,
    );
    let parent = document(
        vec![
            timed(when(DslCondition::Always, call("child"), None)),
            timed(dsl_click(2, 2)),
        ],
        1,
    );

    let (actions, _, completed) = run_with_macros(parent, vec![("child", child)]);

    assert!(completed);
    assert_eq!(actions, vec![click(5, 5), click(2, 2)]);
}

#[test]
fn a_cycle_through_a_branch_is_detected_and_does_not_hang() {
    // "loop" clicks, then calls itself from inside an `if`.
    let looping = document(
        vec![
            timed(dsl_click(9, 9)),
            timed(when(DslCondition::Always, call("loop"), None)),
        ],
        1,
    );
    let parent = document(vec![timed(call("loop"))], 1);

    let (actions, events, completed) = run_with_macros(parent, vec![("loop", looping.clone())]);

    assert!(completed, "a cyclic call must still terminate the run");
    assert_eq!(actions, vec![click(9, 9)]);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::Error { .. })));
}

// === Tests: emergency stop ===

#[test]
fn emergency_stop_halts_a_forever_loop() {
    let (handle, injected) = spawn_recording(None);

    let doc = document(
        vec![timed(dsl_click(1, 1)), timed(DslAction::Wait { ms: 20 })],
        0, // forever
    );

    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);

    // Let a few iterations run, then request an emergency stop.
    std::thread::sleep(Duration::from_millis(100));
    handle.send(EngineCommand::EmergencyStop);

    // The engine must return to Idle after stopping.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reached_idle = false;
    while Instant::now() < deadline {
        if handle.state() == EngineState::Idle {
            reached_idle = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        reached_idle,
        "engine should return to Idle after EmergencyStop"
    );

    // Once stopped, no further actions should be injected.
    let count_after_stop = injected.lock().unwrap().len();
    std::thread::sleep(Duration::from_millis(60));
    let count_later = injected.lock().unwrap().len();
    assert_eq!(
        count_after_stop, count_later,
        "no actions should be injected after the engine is Idle"
    );
    assert!(
        count_after_stop >= 1,
        "at least one action should have been injected before the stop"
    );

    drop(handle);
}

// === Tests: drag interpolation + safety hardening ===

#[test]
fn drag_is_interpolated_into_down_moves_up() {
    // A 48ms drag from (0,0) to (30,30) interpolates in 3 steps (16ms each),
    // bracketed by a press at the start and a release at the end.
    let doc = document(
        vec![timed(DslAction::Drag {
            from_x: DslValue::Int(0),
            from_y: DslValue::Int(0),
            to_x: DslValue::Int(30),
            to_y: DslValue::Int(30),
            duration_ms: 48,
        })],
        1,
    );

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(
        injected,
        vec![
            Action::MouseDown {
                x: 0,
                y: 0,
                button: MouseButton::Left
            },
            Action::MouseMove { x: 10, y: 10 },
            Action::MouseMove { x: 20, y: 20 },
            Action::MouseMove { x: 30, y: 30 },
            Action::MouseUp {
                x: 30,
                y: 30,
                button: MouseButton::Left
            },
        ],
        "drag should expand to press → interpolated moves → release"
    );
}

#[test]
fn instant_drag_still_presses_moves_and_releases() {
    // A zero-duration drag still produces a single move to the target plus the
    // press/release, so the button is never left down.
    let doc = document(
        vec![timed(DslAction::Drag {
            from_x: DslValue::Int(5),
            from_y: DslValue::Int(5),
            to_x: DslValue::Int(9),
            to_y: DslValue::Int(12),
            duration_ms: 0,
        })],
        1,
    );

    let (injected, completed) = run_until_complete(doc, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(
        injected,
        vec![
            Action::MouseDown {
                x: 5,
                y: 5,
                button: MouseButton::Left
            },
            Action::MouseMove { x: 9, y: 12 },
            Action::MouseUp {
                x: 9,
                y: 12,
                button: MouseButton::Left
            },
        ]
    );
}

#[test]
fn consecutive_injection_failures_stop_the_run() {
    // A forever-loop whose injector always fails must auto-stop instead of
    // spinning indefinitely. The engine tolerates a fixed number of consecutive
    // failures (5) before stopping with a dedicated Error.
    const MAX_CONSECUTIVE_FAILURES: usize = 5;

    let attempts = Arc::new(Mutex::new(0usize));
    let executor = FailingExecutor {
        attempts: attempts.clone(),
    };
    let handle = Player::spawn(executor, MockPlatform);

    let doc = document(vec![timed(dsl_click(1, 1))], 0); // forever
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);

    // Wait for the dedicated auto-stop Error.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut auto_stopped = false;
    while Instant::now() < deadline {
        while let Some(event) = handle.try_recv() {
            if let EngineEvent::Error { message } = &event {
                if message.contains("consecutive injection failures") {
                    auto_stopped = true;
                }
            }
        }
        if auto_stopped {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    assert!(
        auto_stopped,
        "the run should auto-stop after repeated injection failures"
    );

    // No more attempts after the stop; exactly the failure budget was spent.
    std::thread::sleep(Duration::from_millis(40));
    assert_eq!(*attempts.lock().unwrap(), MAX_CONSECUTIVE_FAILURES);

    // And the engine settles back to Idle.
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut reached_idle = false;
    while Instant::now() < deadline {
        if handle.state() == EngineState::Idle {
            reached_idle = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(reached_idle, "engine should return to Idle after auto-stop");

    drop(handle);
}

#[test]
fn sub_second_arming_still_runs_and_ticks() {
    // A 200ms start delay must be honored (whole-second countdown ticks of 0
    // would otherwise skip it) and still emit a countdown tick.
    let mut doc = document(vec![timed(dsl_click(1, 1))], 1);
    doc.run.start_delay_ms = 200;

    let (handle, injected) = spawn_recording(None);
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);
    let (completed, events) = pump_until_complete(&handle, Duration::from_secs(5));
    let actions = injected.lock().unwrap().clone();
    drop(handle);

    assert!(completed);
    assert_eq!(actions, vec![click(1, 1)]);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, EngineEvent::CountdownTick { remaining_secs: 1 })),
        "a sub-second arming delay should still emit a countdown tick"
    );
}

// === Tests: timeline clock (pause / lost target window must not cause a burst) ===

/// Executor that records *when* each action reached the injector.
struct TimestampedExecutor {
    injected: Arc<Mutex<Vec<(Instant, Action)>>>,
}

impl ActionExecutor for TimestampedExecutor {
    fn execute(&self, action: &Action) -> Result<(), String> {
        self.injected
            .lock()
            .unwrap()
            .push((Instant::now(), action.clone()));
        Ok(())
    }
}

/// Platform stub whose "target window is focused" answer can be flipped live.
struct SwitchablePlatform {
    focused: Arc<std::sync::atomic::AtomicBool>,
}

impl PlatformConditionProvider for SwitchablePlatform {
    fn is_window_focused(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        self.focused.load(std::sync::atomic::Ordering::SeqCst)
    }
    fn window_exists(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn get_pixel_color(&self, _x: i32, _y: i32) -> Option<tap_core::ConditionColor> {
        None
    }
    fn window_origin(&self, _title: Option<&str>, _process: Option<&str>) -> Option<(i32, i32)> {
        None
    }
}

/// Three clicks 300ms apart: at 0, 300 and 600 ms of timeline time.
fn spaced_clicks_document() -> MacroDocument {
    let at = |at_ms: u64, x: i64| DslTimedAction {
        at_ms,
        action: dsl_click(x, 0),
        enabled: true,
        note: None,
    };
    document(vec![at(0, 1), at(300, 2), at(600, 3)], 1)
}

fn wait_for(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if condition() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(2));
    }
    condition()
}

fn ms(d: Duration) -> u128 {
    d.as_millis()
}

#[test]
fn pausing_does_not_make_the_remaining_steps_fire_as_a_burst() {
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        TimestampedExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
    );
    handle.send(EngineCommand::SetDocument(Box::new(
        spaced_clicks_document(),
    )));
    handle.send(EngineCommand::Start);

    // Pause right after the first click, long enough that both remaining steps
    // would be overdue on the wall clock (600ms) by the time we resume.
    assert!(wait_for(Duration::from_secs(2), || {
        !injected.lock().unwrap().is_empty()
    }));
    handle.send(EngineCommand::Pause);
    std::thread::sleep(Duration::from_millis(900));
    let resumed_at = Instant::now();
    handle.send(EngineCommand::Resume);

    assert!(wait_for(Duration::from_secs(3), || {
        injected.lock().unwrap().len() == 3
    }));
    let stamps: Vec<Instant> = injected.lock().unwrap().iter().map(|(t, _)| *t).collect();
    drop(handle);

    // Spacing is preserved on the active-time clock (expected ~250-300ms).
    // Lower bounds only: a burst would put both gaps at ~0.
    let first_after_resume = stamps[1].duration_since(resumed_at);
    let second_gap = stamps[2].duration_since(stamps[1]);
    assert!(
        ms(first_after_resume) >= 200,
        "step 2 fired {}ms after resume, expected the remaining ~250-300ms of its delay",
        ms(first_after_resume)
    );
    assert!(
        ms(second_gap) >= 200,
        "steps 2 and 3 fired {}ms apart, expected ~300ms",
        ms(second_gap)
    );
}

#[test]
fn losing_the_target_window_does_not_make_the_remaining_steps_fire_as_a_burst() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let focused = Arc::new(AtomicBool::new(false)); // target starts unfocused
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        TimestampedExecutor {
            injected: injected.clone(),
        },
        SwitchablePlatform {
            focused: focused.clone(),
        },
    );

    let mut doc = spaced_clicks_document();
    doc.target_window = Some(tap_core::DslTargetWindow {
        title: Some("Game".to_string()),
        process: None,
        pause_when_unfocused: true,
        coordinates: Default::default(),
    });
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);

    // The engine blocks on the very first step while the window is unfocused.
    // Stay away long enough that every step would be overdue on the wall clock.
    std::thread::sleep(Duration::from_millis(900));
    assert!(
        injected.lock().unwrap().is_empty(),
        "nothing may be injected while the target window is unfocused"
    );
    focused.store(true, Ordering::SeqCst);

    assert!(wait_for(Duration::from_secs(4), || {
        injected.lock().unwrap().len() == 3
    }));
    let stamps: Vec<Instant> = injected.lock().unwrap().iter().map(|(t, _)| *t).collect();
    drop(handle);

    // Full spacing is preserved after refocus (expected ~600ms end to end).
    let total = stamps[2].duration_since(stamps[0]);
    assert!(
        ms(total) >= 500,
        "first and last step fired {}ms apart, expected ~600ms",
        ms(total)
    );
}

#[test]
fn an_overrun_step_still_runs_immediately_without_any_stall() {
    // Documented semantics (D1): `at_ms` is a not-before offset. A step that is
    // already overdue because an earlier step took long (here an explicit Wait)
    // is NOT delayed further. Only pauses / lost windows are excluded.
    let at = |at_ms: u64, action: DslAction| DslTimedAction {
        at_ms,
        action,
        enabled: true,
        note: None,
    };
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        TimestampedExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
    );
    let doc = document(
        vec![
            at(0, dsl_click(1, 0)),
            at(0, DslAction::Wait { ms: 300 }),
            at(100, dsl_click(2, 0)), // due at 100ms, but the Wait ends at ~300ms
        ],
        1,
    );
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);

    assert!(wait_for(Duration::from_secs(3), || {
        injected.lock().unwrap().len() == 2
    }));
    let stamps: Vec<Instant> = injected.lock().unwrap().iter().map(|(t, _)| *t).collect();
    drop(handle);

    let gap = stamps[1].duration_since(stamps[0]);
    assert!(ms(gap) >= 250, "the Wait must still elapse ({}ms)", ms(gap));
    assert!(
        ms(gap) < 400,
        "an overdue step after a Wait must not be delayed further ({}ms)",
        ms(gap)
    );
}

// === Tests: window-relative coordinates ===

type SharedOrigin = Arc<Mutex<Option<(i32, i32)>>>;
type SharedPixels = Arc<Mutex<HashMap<(i32, i32), tap_core::ConditionColor>>>;

/// A platform whose target window can be moved, hidden and shown while a run is in progress, and
/// whose screen holds a few known pixels. The window is always focused (when present).
struct MovableWindowPlatform {
    origin: SharedOrigin,
    pixels: SharedPixels,
}

impl PlatformConditionProvider for MovableWindowPlatform {
    fn is_window_focused(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn window_exists(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn get_pixel_color(&self, x: i32, y: i32) -> Option<tap_core::ConditionColor> {
        self.pixels.lock().unwrap().get(&(x, y)).copied()
    }
    fn window_origin(&self, _title: Option<&str>, _process: Option<&str>) -> Option<(i32, i32)> {
        *self.origin.lock().unwrap()
    }
}

/// Records injected actions and, if asked, drags the target window 200 px to the right each time
/// something is injected (to prove the origin is looked up per action, not once per run).
struct WindowMovingExecutor {
    injected: Arc<Mutex<Vec<Action>>>,
    origin: SharedOrigin,
    move_after_each: bool,
}

impl ActionExecutor for WindowMovingExecutor {
    fn execute(&self, action: &Action) -> Result<(), String> {
        self.injected.lock().unwrap().push(action.clone());
        if self.move_after_each {
            if let Some((x, y)) = self.origin.lock().unwrap().as_mut() {
                *x += 200;
                let _ = y;
            }
        }
        Ok(())
    }
}

struct WindowHarness {
    handle: PlayerHandle,
    injected: Arc<Mutex<Vec<Action>>>,
    origin: SharedOrigin,
    pixels: SharedPixels,
}

fn window_harness(origin: Option<(i32, i32)>, move_after_each: bool) -> WindowHarness {
    let origin: SharedOrigin = Arc::new(Mutex::new(origin));
    let pixels: SharedPixels = Arc::new(Mutex::new(HashMap::new()));
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        WindowMovingExecutor {
            injected: injected.clone(),
            origin: origin.clone(),
            move_after_each,
        },
        MovableWindowPlatform {
            origin: origin.clone(),
            pixels: pixels.clone(),
        },
    );
    WindowHarness {
        handle,
        injected,
        origin,
        pixels,
    }
}

fn targeting(mut doc: MacroDocument, coordinates: tap_core::CoordinateSpace) -> MacroDocument {
    doc.target_window = Some(tap_core::DslTargetWindow {
        title: Some("Game".to_string()),
        process: None,
        pause_when_unfocused: true,
        coordinates,
    });
    doc
}

fn relative(doc: MacroDocument) -> MacroDocument {
    targeting(doc, tap_core::CoordinateSpace::Window)
}

fn start(h: &WindowHarness, doc: MacroDocument) {
    h.handle.send(EngineCommand::SetDocument(Box::new(doc)));
    h.handle.send(EngineCommand::Start);
}

#[test]
fn window_relative_positions_are_moved_to_the_windows_screen_position() {
    let h = window_harness(Some((100, 50)), false);
    start(&h, relative(document(vec![timed(dsl_click(10, 20))], 1)));

    let (completed, _) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    assert_eq!(*h.injected.lock().unwrap(), vec![click(110, 70)]);
}

#[test]
fn a_window_that_moves_between_steps_is_followed() {
    // The executor drags the window 200 px right after every injected action.
    let h = window_harness(Some((100, 50)), true);
    start(
        &h,
        relative(document(
            vec![timed(dsl_click(10, 20)), timed(dsl_click(10, 20))],
            2,
        )),
    );

    let (completed, _) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    assert_eq!(
        *h.injected.lock().unwrap(),
        vec![
            click(110, 70),
            click(310, 70),
            click(510, 70),
            click(710, 70)
        ]
    );
}

#[test]
fn both_ends_of_a_drag_are_window_relative() {
    let h = window_harness(Some((100, 50)), false);
    let drag = DslAction::Drag {
        from_x: DslValue::Int(1),
        from_y: DslValue::Int(2),
        to_x: DslValue::Int(30),
        to_y: DslValue::Int(40),
        duration_ms: 0,
    };
    start(&h, relative(document(vec![timed(drag)], 1)));

    let (completed, _) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    let injected = h.injected.lock().unwrap();
    let press = Action::MouseDown {
        x: 101,
        y: 52,
        button: MouseButton::Left,
    };
    let release = Action::MouseUp {
        x: 130,
        y: 90,
        button: MouseButton::Left,
    };
    assert_eq!(injected.first(), Some(&press));
    assert_eq!(injected.last(), Some(&release));
}

#[test]
fn an_action_nested_in_a_conditional_is_window_relative_too() {
    let h = window_harness(Some((100, 50)), false);
    let branch = DslAction::Conditional {
        condition: DslCondition::Always,
        then_action: Box::new(dsl_click(10, 20)),
        else_action: None,
    };
    start(&h, relative(document(vec![timed(branch)], 1)));

    let (completed, _) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    assert_eq!(*h.injected.lock().unwrap(), vec![click(110, 70)]);
}

#[test]
fn screen_coordinates_are_untouched_even_with_a_target_window() {
    // The window is somewhere, but the document measures from the screen.
    let h = window_harness(Some((100, 50)), false);
    start(
        &h,
        targeting(
            document(vec![timed(dsl_click(10, 20))], 1),
            tap_core::CoordinateSpace::Screen,
        ),
    );

    let (completed, _) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    assert_eq!(*h.injected.lock().unwrap(), vec![click(10, 20)]);
}

#[test]
fn a_missing_window_is_waited_for_and_the_step_then_lands_in_the_window() {
    let h = window_harness(None, false);
    start(&h, relative(document(vec![timed(dsl_click(10, 20))], 1)));

    std::thread::sleep(Duration::from_millis(350));
    assert!(
        h.injected.lock().unwrap().is_empty(),
        "nothing may be injected while there is no window to measure from"
    );

    *h.origin.lock().unwrap() = Some((300, 100));
    let (completed, events) = pump_until_complete(&h.handle, Duration::from_secs(3));

    assert!(completed);
    assert_eq!(*h.injected.lock().unwrap(), vec![click(310, 120)]);
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::TargetWindowUnfocused { .. })));
    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::TargetWindowFocused)));
}

#[test]
fn stopping_while_waiting_for_the_window_ends_the_run() {
    let h = window_harness(None, false);
    start(&h, relative(document(vec![timed(dsl_click(10, 20))], 1)));
    std::thread::sleep(Duration::from_millis(250));

    h.handle.send(EngineCommand::Stop);

    assert!(wait_for(Duration::from_secs(3), || {
        h.handle.state() == EngineState::Idle
    }));
    assert!(h.injected.lock().unwrap().is_empty());
}

fn red_pixel_condition(timeout_ms: u64) -> DslAction {
    DslAction::WaitUntil {
        condition: DslCondition::PixelColor {
            x: 5,
            y: 5,
            color: "#FF0000".to_string(),
            tolerance: 0,
        },
        timeout_ms: Some(timeout_ms),
        poll_interval_ms: 10,
    }
}

fn events_of(h: &WindowHarness, timeout: Duration) -> Vec<EngineEvent> {
    pump_until_complete(&h.handle, timeout).1
}

#[test]
fn pixel_color_positions_are_window_relative() {
    let h = window_harness(Some((100, 50)), false);
    // Red at (5, 5) *in the window*, which is (105, 55) on the screen.
    h.pixels
        .lock()
        .unwrap()
        .insert((105, 55), tap_core::ConditionColor::new(255, 0, 0));
    start(
        &h,
        relative(document(vec![timed(red_pixel_condition(1000))], 1)),
    );

    let events = events_of(&h, Duration::from_secs(3));

    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::ConditionSatisfied { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, EngineEvent::ConditionTimeout { .. })));
}

#[test]
fn a_pixel_condition_without_its_window_never_reads_another_place_on_the_screen() {
    let h = window_harness(None, false);
    // The unshifted position holds the wanted color: reading it would be the wrong answer.
    h.pixels
        .lock()
        .unwrap()
        .insert((5, 5), tap_core::ConditionColor::new(255, 0, 0));
    start(
        &h,
        relative(document(vec![timed(red_pixel_condition(150))], 1)),
    );

    let events = events_of(&h, Duration::from_secs(3));

    assert!(events
        .iter()
        .any(|e| matches!(e, EngineEvent::ConditionTimeout { .. })));
    assert!(!events
        .iter()
        .any(|e| matches!(e, EngineEvent::ConditionSatisfied { .. })));
}

// === Tests: humanize ===

fn with_humanize(mut doc: MacroDocument, position_px: u32, delay_ms: u32) -> MacroDocument {
    doc.run.humanize = tap_core::Humanize {
        position_px,
        delay_ms,
    };
    doc
}

/// Run `doc` on a player whose variation is drawn from `seed`.
fn run_seeded(doc: MacroDocument, seed: u64) -> Vec<Action> {
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn_with_seed(
        RecordingExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
        seed,
    );
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));
    assert!(completed);
    let actions = injected.lock().unwrap().clone();
    actions
}

fn clicks_at(x: i64, y: i64, count: usize) -> Vec<DslTimedAction> {
    (0..count).map(|_| timed(dsl_click(x, y))).collect()
}

fn click_position(action: &Action) -> (i32, i32) {
    match action {
        Action::Click { x, y, .. }
        | Action::MouseDown { x, y, .. }
        | Action::MouseUp { x, y, .. } => (*x, *y),
        other => panic!("not a positioned action: {other:?}"),
    }
}

#[test]
fn humanize_moves_clicks_a_few_pixels_but_never_further_than_asked() {
    let doc = with_humanize(document(clicks_at(500, 300, 40), 1), 6, 0);

    let injected = run_seeded(doc, 11);

    assert_eq!(injected.len(), 40);
    let positions: std::collections::HashSet<_> = injected.iter().map(click_position).collect();
    assert!(
        positions
            .iter()
            .all(|(x, y)| (494..=506).contains(x) && (294..=306).contains(y)),
        "every click stays within ±6 px: {positions:?}"
    );
    assert!(
        positions.len() > 10,
        "the clicks are spread out, not all in one place"
    );
}

#[test]
fn the_same_seed_replays_the_same_variation() {
    let build = || with_humanize(document(clicks_at(100, 100, 12), 1), 8, 0);

    let first = run_seeded(build(), 5);
    assert_eq!(first, run_seeded(build(), 5));
    assert_ne!(first, run_seeded(build(), 6));
}

#[test]
fn without_humanize_every_click_is_exact() {
    let injected = run_seeded(document(clicks_at(7, 8, 5), 1), 1);
    assert_eq!(injected, vec![click(7, 8); 5]);
}

#[test]
fn a_pressed_button_is_released_where_it_was_pressed() {
    let mut timeline = Vec::new();
    for _ in 0..25 {
        timeline.push(timed(DslAction::MouseDown {
            x: DslValue::Int(400),
            y: DslValue::Int(400),
            button: DslMouseButton::Left,
        }));
        timeline.push(timed(DslAction::MouseUp {
            x: DslValue::Int(400),
            y: DslValue::Int(400),
            button: DslMouseButton::Left,
        }));
    }
    let doc = with_humanize(document(timeline, 1), 10, 0);

    let injected = run_seeded(doc, 3);

    assert_eq!(injected.len(), 50);
    let mut distinct = std::collections::HashSet::new();
    for pair in injected.chunks(2) {
        assert_eq!(
            click_position(&pair[0]),
            click_position(&pair[1]),
            "a click must not turn into a tiny drag"
        );
        distinct.insert(click_position(&pair[0]));
    }
    assert!(
        distinct.len() > 5,
        "each press still varies from the others"
    );
}

#[test]
fn humanize_applies_inside_a_window_relative_document_before_the_window_shift() {
    let origin: SharedOrigin = Arc::new(Mutex::new(Some((100, 50))));
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn_with_seed(
        WindowMovingExecutor {
            injected: injected.clone(),
            origin: origin.clone(),
            move_after_each: false,
        },
        MovableWindowPlatform {
            origin,
            pixels: Arc::new(Mutex::new(HashMap::new())),
        },
        9,
    );
    let doc = relative(with_humanize(document(clicks_at(10, 20, 30), 1), 3, 0));
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));

    assert!(completed);
    let injected = injected.lock().unwrap();
    assert_eq!(injected.len(), 30);
    let positions: std::collections::HashSet<_> = injected.iter().map(click_position).collect();
    assert!(
        positions
            .iter()
            .all(|(x, y)| (107..=113).contains(x) && (67..=73).contains(y)),
        "the click varies around (110, 70) on the screen: {positions:?}"
    );
    assert!(positions.len() > 3);
}

#[test]
fn delay_jitter_keeps_steps_in_order_and_never_starts_one_before_its_earliest_time() {
    const GAP_MS: u64 = 60;
    const JITTER_MS: u32 = 50;
    let timeline: Vec<DslTimedAction> = (0..6)
        .map(|i| DslTimedAction {
            at_ms: i * GAP_MS,
            action: dsl_click(i as i64, 0),
            enabled: true,
            note: None,
        })
        .collect();
    let doc = with_humanize(document(timeline, 1), 0, JITTER_MS);

    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn_with_seed(
        TimestampedExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
        21,
    );
    let began = Instant::now();
    handle.send(EngineCommand::SetDocument(Box::new(doc)));
    handle.send(EngineCommand::Start);
    let (completed, _) = pump_until_complete(&handle, Duration::from_secs(5));

    assert!(completed);
    let injected = injected.lock().unwrap();
    let xs: Vec<i32> = injected.iter().map(|(_, a)| click_position(a).0).collect();
    assert_eq!(
        xs,
        vec![0, 1, 2, 3, 4, 5],
        "steps run in their timeline order"
    );
    for (i, (at, _)) in injected.iter().enumerate() {
        let earliest = (i as u64 * GAP_MS).saturating_sub(u64::from(JITTER_MS));
        assert!(
            at.duration_since(began) >= Duration::from_millis(earliest),
            "step {i} started before {earliest} ms"
        );
    }
}
