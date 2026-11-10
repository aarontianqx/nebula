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
//! Because the engine now consumes a [`MacroDocument`] directly (Phase 6 / M3),
//! these tests also assert the Resolve stage: `{{ var }}` / `{{ expr }}` and
//! run-time overrides are substituted *before* injection, and `call_macro`
//! (M4) is expanded inline with cycle protection and an isolated child scope.

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

// === Tests: M3 Resolve stage ===

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

// === Tests: M4 call_macro expansion ===

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

// === Tests: M7 drag interpolation + safety hardening ===

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
