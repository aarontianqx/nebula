//! Live integration test for the three-layer login chain (Phase 1 of
//! specs/proposals/protocol-driven-automation.md).
//!
//! Drives a real session end-to-end: layer-1 login page → layer-2 server entry
//! → layer-3 game page, until the game's own Connection reports connected
//! (`LoginSucceeded`). Launches a real (headless) Chrome and logs into the game
//! with a real account, so it is ignored by default.
//!
//! Credentials come from env vars and are never persisted:
//!
//! ```sh
//! WLY_TEST_USER=... WLY_TEST_PASS=... WLY_TEST_SERVER=888 \
//!   cargo test --test login_chain -- --ignored --nocapture
//! ```

use std::time::Duration;
use wardenly_lib::application::command::SessionCommand;
use wardenly_lib::application::eventbus::create_event_bus;
use wardenly_lib::application::service::SessionActor;
use wardenly_lib::domain::event::DomainEvent;
use wardenly_lib::domain::model::Account;

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires real account credentials and logs into the live game"]
async fn login_chain_reaches_ready() {
    let user = std::env::var("WLY_TEST_USER").expect("WLY_TEST_USER is required");
    let pass = std::env::var("WLY_TEST_PASS").expect("WLY_TEST_PASS is required");
    let server: i32 = std::env::var("WLY_TEST_SERVER")
        .unwrap_or_else(|_| "888".to_string())
        .parse()
        .expect("WLY_TEST_SERVER must be an integer");

    let account = Account::new("login-chain-test".to_string(), user, pass, server, 0);
    let profile_dir = wardenly_lib::infrastructure::config::paths::profile_dir(&account.id);

    let event_bus = create_event_bus();
    let mut events = event_bus.subscribe();

    let handle = SessionActor::spawn(account, event_bus);
    handle.cmd_tx.send(SessionCommand::Start).await.unwrap();

    // The full chain (browser start + login + game load) can take well over a
    // minute on a cold profile; give it generous headroom.
    let result = tokio::time::timeout(Duration::from_secs(240), async {
        loop {
            match events.recv().await {
                Ok(DomainEvent::LoginSucceeded { .. }) => break,
                Ok(DomainEvent::LoginFailed { reason, .. }) => {
                    panic!("login failed: {reason}")
                }
                Ok(_) => {}
                Err(e) => panic!("event bus closed before login completed: {e}"),
            }
        }
    })
    .await;

    let _ = handle.cmd_tx.send(SessionCommand::Stop).await;
    // Throwaway profile for a throwaway account id; don't leave it on disk.
    let _ = std::fs::remove_dir_all(&profile_dir);

    assert!(result.is_ok(), "timed out waiting for LoginSucceeded");
}
