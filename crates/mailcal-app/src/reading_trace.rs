//! Timing one reading-open, step by step.
//!
//! Split from `reading.rs`, which holds the open itself: this is the instrumentation that answers
//! "which `await` is a slow one sitting in".

use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Instant,
};

/// Rising id for one reading-open, so two overlapping opens can be told apart in the log and a
/// retried one stays recognisable across its attempts.
static OPEN_SEQ: AtomicU64 = AtomicU64::new(0);

/// Times a reading-open step by step, at `debug`.
///
/// The reading view sits on its spinner until the open publishes, and every step
/// between is an `await`: a store read, the accounts lock, a provider round-trip, the inline-image
/// and attachment reads. When an open does not come back, "which one of those is it in" is the only
/// question worth asking, and one elapsed figure for the whole open cannot answer it.
///
/// Durations, byte counts and a synthetic id only: no key, no address, no subject
/// (`docs/logging.md`).
pub(crate) struct OpenTrace {
    id: u64,
    started: Instant,
    pub(crate) lap: Instant,
}

impl OpenTrace {
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            id: OPEN_SEQ.fetch_add(1, Ordering::Relaxed),
            started: now,
            lap: now,
        }
    }

    /// Logs how long the step just finished took, and starts the next lap.
    pub(crate) fn step(&mut self, what: &str) {
        log::debug!(
            "read[{}]: {what} in {}ms",
            self.id,
            self.lap.elapsed().as_millis()
        );
        self.lap = Instant::now();
    }

    /// Logs an outcome against the whole open rather than the current lap.
    pub(crate) fn total(&self, what: &str) {
        log::debug!(
            "read[{}]: {what} after {}ms",
            self.id,
            self.started.elapsed().as_millis()
        );
    }
}
