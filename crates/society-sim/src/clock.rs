//! Virtual time, version counters, and scheduled event primitives.

use serde::{Deserialize, Serialize};
use society_domain::{VirtualTime, WorldVersion};

/// A scheduled simulation event ordered by virtual time, priority, then sequence.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Scheduled<T> {
    /// When the event becomes due.
    pub at: VirtualTime,
    /// Lower values run first when timestamps are equal.
    pub priority: u16,
    /// Monotonic tie-breaker assigned by the scheduler.
    pub sequence: u64,
    /// Event payload.
    pub payload: T,
}

/// Minimal authoritative clock and version counter for the foundation runner.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SimulationClock {
    pub(super) now: VirtualTime,
    pub(super) version: WorldVersion,
}

impl SimulationClock {
    /// Current virtual timestamp.
    #[must_use]
    pub const fn now(&self) -> VirtualTime {
        self.now
    }

    /// Current authoritative world version.
    #[must_use]
    pub const fn version(&self) -> WorldVersion {
        self.version
    }

    /// Advances virtual time monotonically.
    pub fn advance_to(&mut self, time: VirtualTime) -> Result<(), ClockError> {
        if time < self.now {
            return Err(ClockError::TimeReversal);
        }
        self.now = time;
        Ok(())
    }

    /// Marks one successful authoritative transition.
    pub fn commit(&mut self) -> Result<WorldVersion, ClockError> {
        self.version.0 = self
            .version
            .0
            .checked_add(1)
            .ok_or(ClockError::VersionExhausted)?;
        Ok(self.version)
    }
}

/// Errors from monotonic simulation bookkeeping.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    /// Requested timestamp precedes the current virtual time.
    TimeReversal,
    /// The virtual timestamp reached its integer limit.
    TimeExhausted,
    /// The state version reached its integer limit.
    VersionExhausted,
}
