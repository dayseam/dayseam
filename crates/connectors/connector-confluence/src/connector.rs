//! [`SourceConnector`] implementation + per-source multiplexer for
//! Confluence.
//!
//! The shape mirrors [`connector_jira::JiraMux`] one-for-one; see
//! that type's docs for the "why a mux per kind" rationale. The only
//! scaffold-era difference is that every
//! [`SourceConnector::sync`] variant returns
//! [`DayseamError::Unsupported`]: the per-day CQL walker lands in
//! DAY-80 and flips the `SyncRequest::Day` arm, keeping the scaffold
//! and the walker as two independently-reviewable PRs (mirroring the
//! DAY-76 → DAY-77 split on the Jira side).
//!
//! `healthcheck` is wired up in this scaffold because the Settings
//! "Test connection" button (DAY-83 UI) uses it to prove the stored
//! Basic-auth credential still authenticates, and a green probe here
//! is a precondition the walker assumes at run time.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use connectors_sdk::{ConnCtx, SourceConnector, SyncRequest, SyncResult};
use dayseam_core::{error_codes, DayseamError, SourceHealth, SourceId, SourceKind};
use tokio::sync::RwLock;

use crate::config::ConfluenceConfig;

/// One configured Confluence source. Holds only the per-source
/// configuration that does **not** live on the
/// [`connectors_sdk::BasicAuth`] attached to each [`ConnCtx`].
/// Cloning is cheap — [`ConfluenceConfig`] is one short `String`
/// wrapped in a `Url`.
#[derive(Debug, Clone)]
pub struct ConfluenceConnector {
    config: ConfluenceConfig,
}

impl ConfluenceConnector {
    /// Construct a connector handle for a single Confluence source.
    #[must_use]
    pub fn new(config: ConfluenceConfig) -> Self {
        Self { config }
    }

    /// Borrow the configured workspace URL. Exposed for the Settings
    /// UI (and DAY-80 tests) to render "currently connected to
    /// `<workspace>`" text without having to reach into
    /// `BasicAuth::descriptor`.
    #[must_use]
    pub fn config(&self) -> &ConfluenceConfig {
        &self.config
    }
}

#[async_trait]
impl SourceConnector for ConfluenceConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::Confluence
    }

    async fn healthcheck(&self, ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        // `GET /rest/api/3/myself` — shared with Jira. Any Atlassian
        // Cloud credential that authenticates against this endpoint
        // also authenticates against the `/wiki/*` Confluence
        // surface, so this probe is the right "can we talk to
        // Confluence?" signal. Rationale matches
        // `connector_jira::connector::healthcheck`: we route through
        // `ctx.auth.authenticate(…)` (rather than calling
        // `validate_auth` a second time) so the probe uses whatever
        // auth strategy the orchestrator hands us.
        let url = self
            .config
            .workspace_url
            .join("rest/api/3/myself")
            .map_err(|e| DayseamError::InvalidConfig {
                code: "confluence.config.bad_workspace_url".to_string(),
                message: format!("cannot join `/rest/api/3/myself` onto workspace URL: {e}"),
            })?;
        let request = ctx
            .http
            .reqwest()
            .get(url)
            .header("Accept", "application/json");
        let request = ctx.auth.authenticate(request).await?;
        match ctx
            .http
            .send(request, &ctx.cancel, Some(&ctx.progress), Some(&ctx.logs))
            .await
        {
            Ok(_) => Ok(SourceHealth {
                ok: true,
                checked_at: Some(Utc::now()),
                last_error: None,
            }),
            Err(err) => Ok(SourceHealth {
                ok: false,
                checked_at: Some(Utc::now()),
                last_error: Some(err),
            }),
        }
    }

    async fn sync(&self, ctx: &ConnCtx, _request: SyncRequest) -> Result<SyncResult, DayseamError> {
        // Scaffold PR (DAY-79). DAY-80 flips `SyncRequest::Day` onto
        // the CQL walker; `Range` / `Since` stay `Unsupported` until
        // v0.3's incremental scheduler — identical split to the
        // DAY-76 → DAY-77 Jira sequence. Returning `Unsupported`
        // across the board here is the simplest way to keep the
        // scaffold honest: no silent empty result that looks like
        // success, no panics, just an error code the orchestrator
        // already knows how to surface.
        ctx.bail_if_cancelled()?;
        Err(DayseamError::Unsupported {
            code: error_codes::CONNECTOR_UNSUPPORTED_SYNC_REQUEST.to_string(),
            message: "confluence connector v0.2-scaffold does not yet service any SyncRequest; \
                     DAY-80 adds the CQL walker for SyncRequest::Day"
                .to_string(),
        })
    }
}

/// Per-source configuration the [`ConfluenceMux`] needs to hydrate
/// one [`ConfluenceConnector`]. One entry per
/// [`dayseam_core::SourceConfig::Confluence`] row; populated at
/// startup (boot-only hydration, ARC-01) and updated by the
/// Add-Source / Reconnect flow in DAY-82.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfluenceSourceCfg {
    pub source_id: SourceId,
    pub config: ConfluenceConfig,
}

/// Multiplexing [`SourceConnector`] for Confluence.
///
/// Semantically identical to [`connector_jira::JiraMux`]: an
/// `Arc<RwLock<HashMap<SourceId, ConfluenceConnector>>>` the
/// Add-Source / Reconnect flow can upsert into without rebuilding the
/// registry.
#[derive(Debug, Clone, Default)]
pub struct ConfluenceMux {
    inner: Arc<RwLock<HashMap<SourceId, ConfluenceConnector>>>,
}

impl ConfluenceMux {
    /// Build a mux pre-populated with `sources`. Empty iterators are
    /// the common case at boot on a brand-new install.
    #[must_use]
    pub fn new(sources: impl IntoIterator<Item = ConfluenceSourceCfg>) -> Self {
        let mut map = HashMap::new();
        for cfg in sources {
            map.insert(cfg.source_id, ConfluenceConnector::new(cfg.config));
        }
        Self {
            inner: Arc::new(RwLock::new(map)),
        }
    }

    /// Add or replace the inner connector for `cfg.source_id`.
    pub async fn upsert(&self, cfg: ConfluenceSourceCfg) {
        let conn = ConfluenceConnector::new(cfg.config);
        self.inner.write().await.insert(cfg.source_id, conn);
    }

    /// Remove the inner connector for `source_id`, if any.
    pub async fn remove(&self, source_id: SourceId) {
        self.inner.write().await.remove(&source_id);
    }

    /// Test-only: how many sources are currently registered.
    #[doc(hidden)]
    pub async fn len(&self) -> usize {
        self.inner.read().await.len()
    }

    /// Test-only: whether the mux has any sources registered. Paired
    /// with [`Self::len`] to keep clippy's `len_without_is_empty`
    /// happy.
    #[doc(hidden)]
    pub async fn is_empty(&self) -> bool {
        self.inner.read().await.is_empty()
    }
}

#[async_trait]
impl SourceConnector for ConfluenceMux {
    fn kind(&self) -> SourceKind {
        SourceKind::Confluence
    }

    async fn healthcheck(&self, ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        match self.inner.read().await.get(&ctx.source_id).cloned() {
            Some(c) => c.healthcheck(ctx).await,
            None => Err(DayseamError::InvalidConfig {
                code: error_codes::IPC_SOURCE_NOT_FOUND.to_string(),
                message: format!("no Confluence source registered for id {}", ctx.source_id),
            }),
        }
    }

    async fn sync(&self, ctx: &ConnCtx, request: SyncRequest) -> Result<SyncResult, DayseamError> {
        match self.inner.read().await.get(&ctx.source_id).cloned() {
            Some(c) => c.sync(ctx, request).await,
            None => Err(DayseamError::InvalidConfig {
                code: error_codes::IPC_SOURCE_NOT_FOUND.to_string(),
                message: format!("no Confluence source registered for id {}", ctx.source_id),
            }),
        }
    }
}
