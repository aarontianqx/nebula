//! End-to-end integration tests for the playback engine.
//!
//! These tests drive a real [`Player`] thread through its public command/event
//! API, but substitute the platform boundaries with in-memory mocks:
//!
//! - [`RecordingExecutor`] records every action that reaches the injector so we
//!   can assert exactly which actions were "performed" (and in what order).
//! - [`MockPlatform`] provides deterministic window/pixel answers so condition
//!   evaluation never touches the real OS.
//!
//! This harness is the safety net for the larger engine refactors: it exercises
//! repeat handling, control-action dispatch, conditional branching, and
//! emergency-stop without any OS side effects.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tap_core::{
    Action, ActionExecutor, CompareOp, Condition, ConditionColor, EngineCommand, EngineEvent,
    EngineState, MouseButton, PlatformConditionProvider, Player, Profile, Repeat, RunConfig,
    TimedAction, Timeline,
};

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

/// Deterministic platform stub: windows always present/focused, no pixels.
struct MockPlatform;

impl PlatformConditionProvider for MockPlatform {
    fn is_window_focused(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn window_exists(&self, _title: Option<&str>, _process: Option<&str>) -> bool {
        true
    }
    fn get_pixel_color(&self, _x: i32, _y: i32) -> Option<ConditionColor> {
        None
    }
}

fn click(x: i32, y: i32) -> Action {
    Action::Click {
        x,
        y,
        button: MouseButton::Left,
    }
}

fn key(k: &str) -> Action {
    Action::KeyTap { key: k.to_string() }
}

/// Build a profile whose actions all fire immediately (at_ms = 0), with no
/// arming countdown, so tests run in milliseconds.
fn immediate_profile(actions: Vec<TimedAction>, repeat: Repeat) -> Profile {
    Profile {
        name: "test".to_string(),
        timeline: Timeline { actions },
        run: RunConfig {
            start_delay_ms: 0,
            speed: 1.0,
            repeat,
        },
        target_window: None,
    }
}

/// Spawn a player, run the profile, and collect the injected actions once the
/// engine reports `Completed`. Returns `(injected_actions, completed)`.
fn run_until_complete(profile: Profile, timeout: Duration) -> (Vec<Action>, bool) {
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        RecordingExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
    );

    handle.send(EngineCommand::SetProfile(profile));
    handle.send(EngineCommand::Start);

    let mut completed = false;
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        while let Some(event) = handle.try_recv() {
            if matches!(event, EngineEvent::Completed) {
                completed = true;
            }
        }
        if completed {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
    }

    let actions = injected.lock().unwrap().clone();
    // NOTE: intentionally drop the handle rather than calling `shutdown()`.
    // The player loops back to `recv()` after a Stop command, so `shutdown()`
    // (which sends Stop then joins while still holding the sender) would block
    // forever. Dropping closes the command channel and lets the thread exit.
    drop(handle);
    (actions, completed)
}

#[test]
fn repeats_inject_every_action_each_iteration() {
    let profile = immediate_profile(
        vec![
            TimedAction::after_ms(0, click(1, 1)),
            TimedAction::after_ms(0, key("a")),
        ],
        Repeat::Times(2),
    );

    let (injected, completed) = run_until_complete(profile, Duration::from_secs(5));

    assert!(
        completed,
        "engine should report Completed for a finite repeat"
    );
    assert_eq!(
        injected,
        vec![click(1, 1), key("a"), click(1, 1), key("a")],
        "every enabled action should be injected once per iteration, in order"
    );
}

#[test]
fn control_actions_are_not_sent_to_the_injector() {
    // Wait and counter actions are handled inside the engine and must never
    // reach the injector; only Click and KeyTap should.
    let profile = immediate_profile(
        vec![
            TimedAction::after_ms(0, click(5, 5)),
            TimedAction::after_ms(0, Action::Wait { ms: 5 }),
            TimedAction::after_ms(
                0,
                Action::SetCounter {
                    key: "c".to_string(),
                    value: 1,
                },
            ),
            TimedAction::after_ms(0, key("b")),
        ],
        Repeat::Times(1),
    );

    let (injected, completed) = run_until_complete(profile, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(injected, vec![click(5, 5), key("b")]);
}

#[test]
fn disabled_actions_are_skipped() {
    let mut disabled = TimedAction::after_ms(0, click(2, 2));
    disabled.enabled = false;

    let profile = immediate_profile(
        vec![TimedAction::after_ms(0, click(1, 1)), disabled],
        Repeat::Times(1),
    );

    let (injected, completed) = run_until_complete(profile, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(injected, vec![click(1, 1)]);
}

#[test]
fn conditional_takes_then_branch_based_on_counter() {
    // Set counter c=3, then a conditional that injects (7,7) when c == 3,
    // otherwise (8,8). The "then" branch must be taken.
    let conditional = Action::Conditional {
        condition: Condition::Counter {
            key: "c".to_string(),
            op: CompareOp::Eq,
            value: 3,
        },
        then_action: Box::new(click(7, 7)),
        else_action: Some(Box::new(click(8, 8))),
    };

    let profile = immediate_profile(
        vec![
            TimedAction::after_ms(
                0,
                Action::SetCounter {
                    key: "c".to_string(),
                    value: 3,
                },
            ),
            TimedAction::after_ms(0, conditional),
        ],
        Repeat::Times(1),
    );

    let (injected, completed) = run_until_complete(profile, Duration::from_secs(5));

    assert!(completed);
    assert_eq!(injected, vec![click(7, 7)]);
}

#[test]
fn emergency_stop_halts_a_forever_loop() {
    let injected = Arc::new(Mutex::new(Vec::new()));
    let handle = Player::spawn(
        RecordingExecutor {
            injected: injected.clone(),
        },
        MockPlatform,
    );

    let profile = immediate_profile(
        vec![
            TimedAction::after_ms(0, click(1, 1)),
            TimedAction::after_ms(0, Action::Wait { ms: 20 }),
        ],
        Repeat::Forever,
    );

    handle.send(EngineCommand::SetProfile(profile));
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
