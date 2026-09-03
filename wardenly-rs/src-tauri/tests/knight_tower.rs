//! Live validation for the unified TaskRunner via the knight tower.
//!
//! Uses a THROWAWAY high-threshold copy of the knight_tower template
//! (`resources/tasks/zz_test_kt.yaml`, threshold 99) so the finish step does
//! not trigger immediately — today's num may already be ≥7. The file is
//! created for the test run and deleted afterwards; it is never committed.
//!
//! Verifies the full protocol loop with zero Rust changes per task:
//!   reload_tower_info → join_team (server-whitelist selector + $-ref payload)
//!   → fight (move + attack with retries) → battle result → loop.
//!
//! Requires other accounts creating teams and starting battles.
//!
//! ```sh
//! WLY_TEST_USER=... WLY_TEST_PASS=... WLY_TEST_SERVER=888 \
//!   cargo test --test knight_tower -- --ignored --nocapture
//! ```

use std::time::Duration;
use wardenly_lib::application::command::SessionCommand;
use wardenly_lib::application::eventbus::create_event_bus;
use wardenly_lib::application::service::SessionActor;
use wardenly_lib::domain::event::DomainEvent;
use wardenly_lib::domain::model::Account;

const TASK_NAME: &str = "zz_test_kt";

async fn next_event(events: &mut tokio::sync::broadcast::Receiver<DomainEvent>) -> DomainEvent {
    loop {
        match events.recv().await {
            Ok(ev) => return ev,
            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                println!(">>> lagged by {n}");
                continue;
            }
            Err(e) => panic!("event bus closed: {e}"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires real account credentials and other players running teams"]
async fn knight_tower_full_loop() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    // The throwaway template must exist (compile-time embedded resource).
    let tasks = wardenly_lib::infrastructure::config::resources::load_tasks().unwrap();
    assert!(
        tasks.iter().any(|t| t.name == TASK_NAME),
        "create resources/tasks/{TASK_NAME}.yaml (high-threshold copy of knight_tower.yaml) before running this test"
    );

    let user = std::env::var("WLY_TEST_USER").expect("WLY_TEST_USER is required");
    let pass = std::env::var("WLY_TEST_PASS").expect("WLY_TEST_PASS is required");
    let server: i32 = std::env::var("WLY_TEST_SERVER")
        .unwrap_or_else(|_| "888".to_string())
        .parse()
        .expect("WLY_TEST_SERVER must be an integer");

    let account = Account::new("knight-tower-test".to_string(), user, pass, server, 0);
    let profile_dir = wardenly_lib::infrastructure::config::paths::profile_dir(&account.id);

    let event_bus = create_event_bus();
    let mut events = event_bus.subscribe();
    let handle = SessionActor::spawn(account, event_bus);
    handle.cmd_tx.send(SessionCommand::Start).await.unwrap();

    // Wait for login.
    let login = tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            match next_event(&mut events).await {
                DomainEvent::LoginSucceeded { .. } => break,
                DomainEvent::LoginFailed { reason, .. } => panic!("login failed: {reason}"),
                _ => {}
            }
        }
    })
    .await;
    assert!(login.is_ok(), "timed out waiting for LoginSucceeded");

    handle
        .cmd_tx
        .send(SessionCommand::StartScript {
            script_name: TASK_NAME.to_string(),
        })
        .await
        .unwrap();

    // Watch the loop: steps executed, join confirmations, own attacks, results.
    let result = tokio::time::timeout(Duration::from_secs(360), async {
        let mut steps: Vec<String> = Vec::new();
        let mut joined = false;
        let mut my_attacks = 0usize;
        let mut results = 0usize;
        loop {
            match next_event(&mut events).await {
                DomainEvent::ScriptStepExecuted { scene_name, .. } => {
                    println!(">>> step: {scene_name}");
                    steps.push(scene_name);
                }
                DomainEvent::ProtocolMessage {
                    name: Some(n),
                    data,
                    ..
                } => match n.as_str() {
                    "S_2_C_KNIGHT_TOWER_PLAYER_INFO" | "S_2_C_KNIGHT_TOWER_PLAYER_COUNT" => {
                        joined = true;
                    }
                    "S_2_C_KNIGHT_TOWER_PLAYER_ATTACK" => {
                        if data.get("name").and_then(|v| v.as_str()) == Some("") {
                            my_attacks += 1;
                            println!(">>> my attack landed ({my_attacks})");
                        }
                    }
                    "S_2_C_KNIGHT_TOWER_RESULT" => {
                        results += 1;
                        println!(">>> battle result #{results}");
                        if my_attacks > 0 {
                            break (steps, joined, my_attacks, results);
                        }
                    }
                    _ => {}
                },
                DomainEvent::ScriptStopped { .. } => break (steps, joined, my_attacks, results),
                _ => {}
            }
        }
    })
    .await;

    let _ = handle
        .cmd_tx
        .send(SessionCommand::StopScript { run_id: None })
        .await;
    let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
    let _ = std::fs::remove_dir_all(&profile_dir);

    let (steps, joined, my_attacks, results) =
        result.expect("timed out waiting for a full tower round (are any teams running?)");
    println!(
        ">>> summary: steps={steps:?} joined={joined} my_attacks={my_attacks} results={results}"
    );
    assert!(
        steps.iter().any(|s| s == "reload_tower_info"),
        "reload_tower_info never ran"
    );
    assert!(joined, "never joined a team");
    assert!(my_attacks > 0, "no attack of mine landed");
    assert!(results > 0, "no battle result observed");
}
