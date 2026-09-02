//! Humanization helpers: cross-cutting behavioral noise applied to every
//! automation path (never task-specific). Goal: scripted behavior should not
//! look like zero-jitter machine cadence to a server-side behavior analyzer.

use rand::Rng;
use std::time::Duration;

/// Randomized pre-action pacing delay (150–450ms) so scripted actions don't
/// fire back-to-back at machine speed. Applied uniformly to every script
/// action in both runners; YAML needs no changes.
pub async fn pace() {
    // Compute before the await: thread_rng is !Send and must not be held
    // across it.
    let delay = rand::thread_rng().gen_range(150..=450);
    tokio::time::sleep(Duration::from_millis(delay)).await;
}

/// Randomized stagger interval (0.5–2.0s) for starting scripts one-by-one.
pub fn stagger_interval() -> Duration {
    Duration::from_millis(rand::thread_rng().gen_range(500..=2000))
}
