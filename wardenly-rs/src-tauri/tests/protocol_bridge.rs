//! Live integration test for the protocol page bridge (Phase 2 of
//! specs/proposals/protocol-driven-automation.md).
//!
//! After login completes, verifies that:
//!   1. downstream traffic is observed (a S_2_C_KEEP_ALIVE heartbeat arrives
//!      as a ProtocolMessage event without any action on our side), and
//!   2. sending C_2_S_MAIL_INFO through the bridge yields the structured
//!      S_2_C_MAILLIST_ID response — no clicking, no decryption.
//!
//! ```sh
//! WLY_TEST_USER=... WLY_TEST_PASS=... WLY_TEST_SERVER=888 \
//!   cargo test --test protocol_bridge -- --ignored --nocapture
//! ```

use std::time::Duration;
use wardenly_lib::application::command::SessionCommand;
use wardenly_lib::application::eventbus::create_event_bus;
use wardenly_lib::application::service::SessionActor;
use wardenly_lib::domain::event::DomainEvent;
use wardenly_lib::domain::model::Account;

/// Receive events until `pred` matches or the timeout elapses.
async fn wait_for_event(
    events: &mut tokio::sync::broadcast::Receiver<DomainEvent>,
    timeout: Duration,
    what: &str,
    pred: impl Fn(&DomainEvent) -> bool,
) {
    let result = tokio::time::timeout(timeout, async {
        loop {
            match events.recv().await {
                Ok(ev) if pred(&ev) => return,
                Ok(_) => {}
                Err(e) => panic!("event bus closed while waiting for {what}: {e}"),
            }
        }
    })
    .await;
    assert!(result.is_ok(), "timed out waiting for {what}");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires real account credentials and logs into the live game"]
async fn protocol_bridge_observes_and_sends() {
    let user = std::env::var("WLY_TEST_USER").expect("WLY_TEST_USER is required");
    let pass = std::env::var("WLY_TEST_PASS").expect("WLY_TEST_PASS is required");
    let server: i32 = std::env::var("WLY_TEST_SERVER")
        .unwrap_or_else(|_| "888".to_string())
        .parse()
        .expect("WLY_TEST_SERVER must be an integer");

    let account = Account::new("protocol-bridge-test".to_string(), user, pass, server, 0);
    let profile_dir = wardenly_lib::infrastructure::config::paths::profile_dir(&account.id);

    let event_bus = create_event_bus();
    let mut events = event_bus.subscribe();

    let handle = SessionActor::spawn(account, event_bus);
    handle.cmd_tx.send(SessionCommand::Start).await.unwrap();

    // 1. Login (which now also installs the page bridge).
    wait_for_event(
        &mut events,
        Duration::from_secs(240),
        "LoginSucceeded",
        |ev| matches!(ev, DomainEvent::LoginSucceeded { .. }),
    )
    .await;

    // 2. Passive observation: the server pushes heartbeats on its own; one of
    //    them must show up as a structured ProtocolMessage.
    wait_for_event(
        &mut events,
        Duration::from_secs(90),
        "S_2_C_KEEP_ALIVE heartbeat",
        |ev| matches!(ev, DomainEvent::ProtocolMessage { name: Some(n), .. } if n == "S_2_C_KEEP_ALIVE"),
    )
    .await;

    // 3. Active driving: request the mail list through the bridge and expect
    //    the structured response.
    handle
        .cmd_tx
        .send(SessionCommand::SendProtocol {
            name: "C_2_S_MAIL_INFO".to_string(),
            payload: serde_json::json!({}),
        })
        .await
        .unwrap();
    wait_for_event(
        &mut events,
        Duration::from_secs(30),
        "S_2_C_MAILLIST_ID response",
        |ev| matches!(ev, DomainEvent::ProtocolMessage { name: Some(n), .. } if n == "S_2_C_MAILLIST_ID"),
    )
    .await;

    let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
    let _ = std::fs::remove_dir_all(&profile_dir);
}
