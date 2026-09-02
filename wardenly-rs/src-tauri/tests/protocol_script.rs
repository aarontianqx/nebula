//! Live acceptance test for Phase 3 (protocol-driven scripts) of
//! specs/proposals/protocol-driven-automation.md.
//!
//! Runs the embedded `claim_all_mail` protocol script on a real account and
//! verifies the whole loop: the script is found by name, started through the
//! normal session command path, exchanges protocols with the game (no
//! screenshots, no clicks), claims all mail attachments, and finishes with
//! StopReason::Completed (surfaced as ScriptStopped after ScriptStepExecuted
//! for every step).
//!
//! NOTE: this actually claims mail rewards on the account (adds items, marks
//! mails read) — that is the intended daily task this project automates.
//!
//! ```sh
//! WLY_TEST_USER=... WLY_TEST_PASS=... WLY_TEST_SERVER=888 \
//!   cargo test --test protocol_script -- --ignored --nocapture
//! ```

use std::time::Duration;
use wardenly_lib::application::command::SessionCommand;
use wardenly_lib::application::eventbus::create_event_bus;
use wardenly_lib::application::service::SessionActor;
use wardenly_lib::domain::event::DomainEvent;
use wardenly_lib::domain::model::Account;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires real account credentials and claims live in-game mail rewards"]
async fn claim_all_mail_script_completes() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();

    let user = std::env::var("WLY_TEST_USER").expect("WLY_TEST_USER is required");
    let pass = std::env::var("WLY_TEST_PASS").expect("WLY_TEST_PASS is required");
    let server: i32 = std::env::var("WLY_TEST_SERVER")
        .unwrap_or_else(|_| "888".to_string())
        .parse()
        .expect("WLY_TEST_SERVER must be an integer");

    let account = Account::new("protocol-script-test".to_string(), user, pass, server, 0);
    let profile_dir = wardenly_lib::infrastructure::config::paths::profile_dir(&account.id);

    let event_bus = create_event_bus();
    let mut events = event_bus.subscribe();

    let handle = SessionActor::spawn(account, event_bus);
    handle.cmd_tx.send(SessionCommand::Start).await.unwrap();

    // Wait for login + bridge, then start the protocol script.
    let login = tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::LoginSucceeded { .. }) => break,
                Ok(DomainEvent::LoginFailed { reason, .. }) => panic!("login failed: {reason}"),
                Ok(_) => {}
                Err(e) => panic!("event bus closed during login: {e}"),
            }
        }
    })
    .await;
    assert!(login.is_ok(), "timed out waiting for LoginSucceeded");

    handle
        .cmd_tx
        .send(SessionCommand::StartScript {
            script_name: "claim_all_mail".to_string(),
        })
        .await
        .unwrap();

    // The script must: fetch the list, draw all rewards, observe the server's
    // acknowledgement (dedicated ack or a resource push), and complete.
    let result = tokio::time::timeout(Duration::from_secs(120), async {
        let mut saw_draw_response = false;
        let mut executed_steps = 0usize;
        loop {
            match events.recv().await {
                Ok(DomainEvent::ProtocolMessage { name: Some(n), .. })
                    if executed_steps > 0
                        && (n == "S_2_C_MAIL_DRAW_ALL_REWARD" || n == "S_2_C_UPDATE_BENEFIT") =>
                {
                    saw_draw_response = true;
                }
                Ok(DomainEvent::ScriptStepExecuted { .. }) => executed_steps += 1,
                Ok(DomainEvent::ScriptStopped { script_name, .. }) => {
                    assert_eq!(script_name, "claim_all_mail");
                    break (saw_draw_response, executed_steps);
                }
                Ok(_) => {}
                Err(e) => panic!("event bus closed during script: {e}"),
            }
        }
    })
    .await;

    let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
    let _ = std::fs::remove_dir_all(&profile_dir);

    let (saw_draw_response, executed_steps) =
        result.expect("timed out waiting for claim_all_mail to finish");
    assert!(
        saw_draw_response,
        "never saw a draw-all acknowledgement (S_2_C_MAIL_DRAW_ALL_REWARD or S_2_C_UPDATE_BENEFIT)"
    );
    assert_eq!(executed_steps, 2, "both steps should have executed");
}
