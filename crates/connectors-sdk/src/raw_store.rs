//! Connector-facing view of the raw payload store.
//!
//! The canonical implementation lives in `dayseam-db` and writes to a
//! zstd-compressed blob store with a retention policy (see
//! `ARCHITECTURE.md` §11.4). This SDK-level trait is deliberately tiny
//! so:
//!
//! 1. connectors depend on an abstraction, not on `dayseam-db`
//!    directly, which would invert the crate graph;
//! 2. tests can drop in a [`NoopRawStore`] that discards payloads, and
//!    phase-2 work can wire in an in-memory stub for golden fixtures;
//! 3. the "don't store private repos" policy is enforced at a single
//!    layer (the real impl) rather than being rediscovered by each
//!    connector.

use async_trait::async_trait;
use dayseam_core::{DayseamError, RawRef};

/// Pluggable raw-payload storage. Connectors call [`RawStore::put`] as
/// they fetch bytes from their upstream and get back a [`RawRef`] they
/// can attach to the corresponding [`dayseam_core::ActivityEvent`].
///
/// The real impl is responsible for:
/// * compressing with zstd,
/// * honoring the source's retention policy and privacy flags,
/// * deduplicating by content hash where cheap,
/// * refusing to write for sources that opted out of raw storage.
#[async_trait]
pub trait RawStore: Send + Sync + std::fmt::Debug {
    /// Persist `bytes` under a connector-chosen `storage_key` (typically
    /// `<connector>:<kind>:<external_id>`). Returns the [`RawRef`] that
    /// will be embedded in every event derived from this payload.
    async fn put(
        &self,
        storage_key: &str,
        content_type: &str,
        bytes: &[u8],
    ) -> Result<RawRef, DayseamError>;
}

/// Raw store that accepts every write and drops the bytes on the
/// floor. Used by v0.1 connectors (raw payload persistence lands in a
/// later phase alongside the retention runbook) and by tests that do
/// not care about raw storage.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopRawStore;

#[async_trait]
impl RawStore for NoopRawStore {
    async fn put(
        &self,
        storage_key: &str,
        content_type: &str,
        _bytes: &[u8],
    ) -> Result<RawRef, DayseamError> {
        Ok(RawRef {
            storage_key: storage_key.to_string(),
            content_type: content_type.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn noop_returns_a_rawref_with_the_provided_key() {
        let out = NoopRawStore
            .put("gitlab:mr:1234", "application/json", b"{}")
            .await
            .expect("ok");
        assert_eq!(out.storage_key, "gitlab:mr:1234");
        assert_eq!(out.content_type, "application/json");
    }
}
