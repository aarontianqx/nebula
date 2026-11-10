//! Active-time clock for the timeline scheduler.
//!
//! A step's `at_ms` is an offset from the start of its timeline pass. If the
//! scheduler measured that offset on the wall clock, any time the run could not
//! make progress (a user pause, a bound window that lost focus) would count as
//! elapsed time, and on resume every step that had meanwhile become "overdue"
//! would fire back-to-back with zero delay, collapsing a recorded sequence into
//! a single burst.
//!
//! The scheduler therefore measures *active* time: wall time minus the time
//! spent stalled. Stalls are recorded here as they begin and end, so call sites
//! (sleeps, waits, drags) need no individual handling.
//!
//! Time spent inside a step's own execution (`Wait`, `WaitUntil`, `Drag`,
//! `call_macro`) is deliberately **not** a stall: an overdue step still runs
//! immediately afterwards. Only involuntary stalls are excluded.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Accumulates stalled time. Pure and time-source agnostic: callers pass `now`
/// (as a monotonic offset), which keeps it unit-testable without sleeping.
///
/// Stalls may overlap (e.g. the user pauses while the engine is already waiting
/// for the target window); overlapping time is counted once.
#[derive(Debug, Default)]
pub(crate) struct StallAccounting {
    /// Total length of stalls that have already ended.
    closed: Duration,
    /// How many stalls are currently open.
    open: u32,
    /// When the current run of overlapping stalls began (`Some` iff `open > 0`).
    open_since: Option<Duration>,
}

/// A snapshot of the clock taken when a timeline pass begins.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClockOrigin {
    wall: Duration,
    stalled: Duration,
}

impl StallAccounting {
    pub(crate) fn begin(&mut self, now: Duration) {
        if self.open == 0 {
            self.open_since = Some(now);
        }
        self.open += 1;
    }

    pub(crate) fn end(&mut self, now: Duration) {
        if self.open == 0 {
            return;
        }
        self.open -= 1;
        if self.open == 0 {
            if let Some(since) = self.open_since.take() {
                self.closed += now.saturating_sub(since);
            }
        }
    }

    fn stalled_at(&self, now: Duration) -> Duration {
        self.closed
            + self
                .open_since
                .map_or(Duration::ZERO, |since| now.saturating_sub(since))
    }

    pub(crate) fn origin(&self, now: Duration) -> ClockOrigin {
        ClockOrigin {
            wall: now,
            stalled: self.stalled_at(now),
        }
    }

    /// Wall time since `origin`, minus the time stalled in that interval.
    pub(crate) fn active_elapsed(&self, origin: ClockOrigin, now: Duration) -> Duration {
        let wall = now.saturating_sub(origin.wall);
        let stalled = self.stalled_at(now).saturating_sub(origin.stalled);
        wall.saturating_sub(stalled)
    }
}

/// The engine's run clock: a monotonic epoch plus shared stall accounting.
pub(crate) struct RunClock {
    epoch: Instant,
    stalls: Mutex<StallAccounting>,
}

impl RunClock {
    pub(crate) fn new() -> Self {
        Self {
            epoch: Instant::now(),
            stalls: Mutex::new(StallAccounting::default()),
        }
    }

    fn now(&self) -> Duration {
        self.epoch.elapsed()
    }

    /// Start a stall that is ended explicitly with [`Self::end_stall`] (pause).
    pub(crate) fn begin_stall(&self) {
        let now = self.now();
        self.stalls.lock().unwrap().begin(now);
    }

    pub(crate) fn end_stall(&self) {
        let now = self.now();
        self.stalls.lock().unwrap().end(now);
    }

    /// Start a stall that ends when the returned guard is dropped, on every
    /// exit path (target window wait).
    pub(crate) fn stall(&self) -> StallGuard<'_> {
        self.begin_stall();
        StallGuard { clock: self }
    }

    pub(crate) fn origin(&self) -> ClockOrigin {
        let now = self.now();
        self.stalls.lock().unwrap().origin(now)
    }

    /// Active milliseconds elapsed since `origin`.
    pub(crate) fn active_elapsed_ms(&self, origin: ClockOrigin) -> u64 {
        let now = self.now();
        self.stalls
            .lock()
            .unwrap()
            .active_elapsed(origin, now)
            .as_millis() as u64
    }
}

/// Ends a stall started by [`RunClock::stall`] when dropped.
pub(crate) struct StallGuard<'a> {
    clock: &'a RunClock,
}

impl Drop for StallGuard<'_> {
    fn drop(&mut self) {
        self.clock.end_stall();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn without_stalls_active_time_equals_wall_time() {
        let acc = StallAccounting::default();
        let origin = acc.origin(ms(100));
        assert_eq!(acc.active_elapsed(origin, ms(350)), ms(250));
    }

    #[test]
    fn a_closed_stall_is_removed_from_active_time() {
        let mut acc = StallAccounting::default();
        let origin = acc.origin(ms(0));

        acc.begin(ms(100));
        acc.end(ms(1_100)); // 1s stall

        assert_eq!(acc.active_elapsed(origin, ms(1_300)), ms(300));
    }

    #[test]
    fn an_open_stall_freezes_active_time() {
        let mut acc = StallAccounting::default();
        let origin = acc.origin(ms(0));

        acc.begin(ms(200));

        assert_eq!(acc.active_elapsed(origin, ms(200)), ms(200));
        assert_eq!(acc.active_elapsed(origin, ms(5_000)), ms(200));
    }

    #[test]
    fn overlapping_stalls_are_counted_once() {
        let mut acc = StallAccounting::default();
        let origin = acc.origin(ms(0));

        acc.begin(ms(100)); // user pauses
        acc.begin(ms(300)); // ...and the target window is lost meanwhile
        acc.end(ms(600)); // window is back, still paused
        assert_eq!(acc.active_elapsed(origin, ms(900)), ms(100));
        acc.end(ms(1_000)); // resumed

        // Stalled from 100 to 1000 exactly once.
        assert_eq!(acc.active_elapsed(origin, ms(1_200)), ms(300));
    }

    #[test]
    fn stalls_before_the_origin_do_not_affect_a_later_pass() {
        let mut acc = StallAccounting::default();
        acc.begin(ms(0));
        acc.end(ms(1_000));

        let origin = acc.origin(ms(2_000));

        assert_eq!(acc.active_elapsed(origin, ms(2_400)), ms(400));
    }

    #[test]
    fn a_stall_open_at_the_origin_only_counts_from_the_origin() {
        let mut acc = StallAccounting::default();
        acc.begin(ms(0));
        let origin = acc.origin(ms(500)); // pass begins while still stalled

        acc.end(ms(900));

        // Only 500..900 of the stall lies inside the pass.
        assert_eq!(acc.active_elapsed(origin, ms(1_000)), ms(100));
    }

    #[test]
    fn unbalanced_end_is_ignored() {
        let mut acc = StallAccounting::default();
        acc.end(ms(10));
        let origin = acc.origin(ms(0));
        assert_eq!(acc.active_elapsed(origin, ms(50)), ms(50));
    }

    #[test]
    fn stall_guard_ends_the_stall_on_drop() {
        let clock = RunClock::new();
        let origin = clock.origin();
        {
            let _guard = clock.stall();
            std::thread::sleep(ms(60));
        }
        // The 60ms spent stalled must not be counted as active time.
        assert!(clock.active_elapsed_ms(origin) < 40);
        std::thread::sleep(ms(30));
        assert!(clock.active_elapsed_ms(origin) >= 25);
    }
}
