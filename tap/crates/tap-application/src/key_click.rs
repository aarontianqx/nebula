//! The Key->Click tool: hold a letter key to keep clicking, Space to stop.
//!
//! - Pressing a letter (A-Z) clicks once straight away and starts a hold timer.
//! - If the key is still held after `hold_delay_ms`, clicks repeat every `min_interval_ms`.
//! - Releasing that key stops the repeating; the next letter starts over.
//! - Space ends the whole mode.
//!
//! [`KeyClickMachine`] only decides *when* a click is due; reading keys, resolving the target
//! point and injecting are the adapter's job.

use std::time::{Duration, Instant};

use tap_core::MouseButton;

/// Bounds that keep a hand-edited request from hammering the system or never repeating.
const MIN_INTERVAL_RANGE_MS: (u64, u64) = (10, 1000);
const MAX_HOLD_DELAY_MS: u64 = 5000;

/// Where each click lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickLocation {
    /// The live cursor position at click time.
    Cursor,
    /// A fixed point in injection-space coordinates.
    Fixed { x: i32, y: i32 },
}

#[derive(Debug, Clone, Copy)]
pub struct KeyClickConfig {
    min_interval_ms: u64,
    hold_delay_ms: u64,
    pub button: MouseButton,
    pub location: ClickLocation,
}

impl KeyClickConfig {
    /// Clamps the timings into their supported range.
    pub fn new(
        min_interval_ms: u64,
        hold_delay_ms: u64,
        button: MouseButton,
        location: ClickLocation,
    ) -> Self {
        Self {
            min_interval_ms: min_interval_ms
                .clamp(MIN_INTERVAL_RANGE_MS.0, MIN_INTERVAL_RANGE_MS.1),
            hold_delay_ms: hold_delay_ms.min(MAX_HOLD_DELAY_MS),
            button,
            location,
        }
    }

    pub fn min_interval_ms(&self) -> u64 {
        self.min_interval_ms
    }

    pub fn hold_delay_ms(&self) -> u64 {
        self.hold_delay_ms
    }
}

/// What a key press asks of the runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDownOutcome {
    Click,
    Stop,
    Ignored,
}

struct HeldKey {
    key: String,
    next_repeat_at: Instant,
}

pub struct KeyClickMachine {
    hold_delay: Duration,
    repeat_interval: Duration,
    held: Option<HeldKey>,
}

impl KeyClickMachine {
    pub fn new(config: &KeyClickConfig) -> Self {
        Self {
            hold_delay: Duration::from_millis(config.hold_delay_ms),
            repeat_interval: Duration::from_millis(config.min_interval_ms),
            held: None,
        }
    }

    pub fn key_down(&mut self, key: &str, now: Instant) -> KeyDownOutcome {
        if key == "Space" {
            return KeyDownOutcome::Stop;
        }
        // One key drives the clicking at a time; others pressed meanwhile are ignored.
        if !is_letter_key(key) || self.held.is_some() {
            return KeyDownOutcome::Ignored;
        }
        self.held = Some(HeldKey {
            key: key.to_string(),
            next_repeat_at: now + self.hold_delay,
        });
        KeyDownOutcome::Click
    }

    pub fn key_up(&mut self, key: &str) {
        if self
            .held
            .as_ref()
            .is_some_and(|held| held.key.eq_ignore_ascii_case(key))
        {
            self.held = None;
        }
    }

    /// Whether a repeat click is due at `now`; if so, the next one is scheduled.
    pub fn repeat_due(&mut self, now: Instant) -> bool {
        match &mut self.held {
            Some(held) if now >= held.next_repeat_at => {
                held.next_repeat_at = now + self.repeat_interval;
                true
            }
            _ => false,
        }
    }

    /// When the next repeat click falls due, if a key is held.
    pub fn next_repeat_at(&self) -> Option<Instant> {
        self.held.as_ref().map(|held| held.next_repeat_at)
    }
}

fn is_letter_key(key: &str) -> bool {
    key.len() == 1 && key.chars().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(hold_delay_ms: u64, min_interval_ms: u64) -> KeyClickMachine {
        KeyClickMachine::new(&KeyClickConfig::new(
            min_interval_ms,
            hold_delay_ms,
            MouseButton::Left,
            ClickLocation::Cursor,
        ))
    }

    fn after(start: Instant, ms: u64) -> Instant {
        start + Duration::from_millis(ms)
    }

    #[test]
    fn timings_are_clamped_into_their_supported_range() {
        let config = KeyClickConfig::new(1, 999_999, MouseButton::Left, ClickLocation::Cursor);
        assert_eq!(config.min_interval_ms(), 10);
        assert_eq!(config.hold_delay_ms(), 5000);

        let config = KeyClickConfig::new(99_999, 0, MouseButton::Left, ClickLocation::Cursor);
        assert_eq!(config.min_interval_ms(), 1000);
        assert_eq!(config.hold_delay_ms(), 0);
    }

    #[test]
    fn a_letter_clicks_at_once_and_repeats_only_after_the_hold_delay() {
        let mut m = machine(150, 40);
        let t0 = Instant::now();

        assert_eq!(m.key_down("a", t0), KeyDownOutcome::Click);
        assert!(!m.repeat_due(after(t0, 149)));
        assert!(m.repeat_due(after(t0, 150)));
        // Then every interval.
        assert!(!m.repeat_due(after(t0, 189)));
        assert!(m.repeat_due(after(t0, 190)));
    }

    #[test]
    fn releasing_the_held_key_stops_the_repeats_and_frees_the_next_letter() {
        let mut m = machine(100, 40);
        let t0 = Instant::now();
        m.key_down("a", t0);

        m.key_up("A"); // the case of a key name is not significant
        assert!(!m.repeat_due(after(t0, 1000)));
        assert_eq!(m.next_repeat_at(), None);
        assert_eq!(m.key_down("b", after(t0, 1000)), KeyDownOutcome::Click);
    }

    #[test]
    fn releasing_another_key_changes_nothing() {
        let mut m = machine(100, 40);
        let t0 = Instant::now();
        m.key_down("a", t0);

        m.key_up("b");

        assert!(m.repeat_due(after(t0, 100)));
    }

    #[test]
    fn a_second_letter_is_ignored_while_one_is_held() {
        let mut m = machine(100, 40);
        let t0 = Instant::now();
        m.key_down("a", t0);

        assert_eq!(m.key_down("b", after(t0, 10)), KeyDownOutcome::Ignored);
    }

    #[test]
    fn only_single_letters_start_clicking() {
        let mut m = machine(100, 40);
        let t0 = Instant::now();
        for key in ["1", "Enter", "F1", "", "ab"] {
            assert_eq!(m.key_down(key, t0), KeyDownOutcome::Ignored, "{key:?}");
        }
        assert_eq!(m.next_repeat_at(), None);
    }

    #[test]
    fn space_stops_even_while_a_key_is_held() {
        let mut m = machine(100, 40);
        let t0 = Instant::now();
        m.key_down("a", t0);

        assert_eq!(m.key_down("Space", t0), KeyDownOutcome::Stop);
    }
}
