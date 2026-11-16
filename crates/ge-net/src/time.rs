//! Caller-supplied monotonic network time.
//!
//! The library never reads a clock. The application converts whatever
//! monotonic source it owns into microseconds since an origin of its choosing.

use std::time::Duration;

/// A monotonic instant in microseconds since a caller-chosen origin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetInstant(u64);

impl NetInstant {
    pub const ZERO: Self = Self(0);

    pub const fn from_micros(micros: u64) -> Self {
        Self(micros)
    }

    pub const fn from_millis(millis: u64) -> Self {
        Self(millis.saturating_mul(1_000))
    }

    pub const fn as_micros(self) -> u64 {
        self.0
    }

    /// Saturating addition; network time never wraps.
    pub fn after(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration_micros(duration)))
    }

    /// Microseconds elapsed since `earlier`, or zero if `earlier` is later.
    pub const fn micros_since(self, earlier: Self) -> u64 {
        self.0.saturating_sub(earlier.0)
    }
}

pub(crate) fn duration_micros(duration: Duration) -> u64 {
    u64::try_from(duration.as_micros()).unwrap_or(u64::MAX)
}
