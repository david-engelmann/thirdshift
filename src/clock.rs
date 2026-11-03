//! Clocks. Every lease reads time from an injected clock, never from the wall.

use std::cell::Cell;
use std::time::{SystemTime, UNIX_EPOCH};

/// A clock the caller injects, so no lease ever reads the wall by
/// accident. Advancing is part of the surface because the runner
/// moves the run clock as it works.
pub trait Clock {
    fn now(&self) -> f64;
    fn advance(&self, seconds: f64) -> f64;
}

/// The production clock. Tests use FakeClock instead.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> f64 {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        elapsed.as_secs() as f64 + elapsed.subsec_nanos() as f64 / 1_000_000_000.0
    }

    fn advance(&self, _seconds: f64) -> f64 {
        // The wall clock cannot be moved. Advancing it is a read.
        self.now()
    }
}

/// A deterministic clock that moves only when told to.
pub struct FakeClock {
    now: Cell<f64>,
}

impl FakeClock {
    pub fn new(start: f64) -> FakeClock {
        FakeClock {
            now: Cell::new(start),
        }
    }
}

impl Default for FakeClock {
    fn default() -> FakeClock {
        FakeClock::new(0.0)
    }
}

impl Clock for FakeClock {
    fn now(&self) -> f64 {
        self.now.get()
    }

    fn advance(&self, seconds: f64) -> f64 {
        let next = self.now.get() + seconds;
        self.now.set(next);
        next
    }
}
