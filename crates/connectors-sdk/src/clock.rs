//! Injectable wall-clock abstraction.
//!
//! Connectors read "now" and "sleep for N" through [`Clock`], not
//! directly from [`chrono::Utc`] or [`tokio::time`]. This keeps retry
//! tests hermetic — the HTTP retry test can install a deterministic
//! clock and assert that sleeps happen without actually waiting for
//! real wall-clock seconds.
//!
//! v0.1 only ships [`SystemClock`]. A mock clock lives in tests.

use chrono::{DateTime, Utc};
use std::time::Duration;

/// Everything a connector needs from the clock: "what time is it" and
/// "please sleep for this long". Separating these lets tests install
/// a fake clock without a global monkey-patch.
#[async_trait::async_trait]
pub trait Clock: Send + Sync + std::fmt::Debug {
    /// The current UTC instant. `ActivityEvent::occurred_at` is always
    /// UTC, so this is what connectors stamp onto `emitted_at` fields
    /// they generate themselves.
    fn now(&self) -> DateTime<Utc>;

    /// Sleep for `duration`. Tests install a fake impl that records
    /// the requested sleeps and returns immediately.
    async fn sleep(&self, duration: Duration);
}

/// Real clock backed by [`chrono::Utc::now`] and [`tokio::time::sleep`].
/// Used everywhere in production.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

#[async_trait::async_trait]
impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    async fn sleep(&self, duration: Duration) {
        tokio::time::sleep(duration).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn system_clock_now_is_monotonic_within_a_call() {
        let clock = SystemClock;
        let a = clock.now();
        let b = clock.now();
        assert!(b >= a);
    }
}
