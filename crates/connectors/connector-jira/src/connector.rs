//! [`SourceConnector`] implementation + per-source multiplexer.
//!
//! The shape mirrors [`connector_gitlab::GitlabMux`] one-for-one:
//!
//! 1. The orchestrator registry is keyed by [`SourceKind`] and stores
//!    a single trait-object handle per kind. Jira, like GitLab, needs
//!    one inner handle per configured source (each carries its own
//!    `workspace_url` + email), so the registered value is a
//!    [`JiraMux`] that dispatches [`SourceConnector::sync`] by
//!    `ctx.source_id` to the right [`JiraConnector`] instance.
//!
//! 2. `JiraConnector::sync` returns `DayseamError::Unsupported` for
//!    every [`SyncRequest`] variant in this scaffold PR. DAY-77
//!    flips the `SyncRequest::Day` arm onto the JQL walker
//!    introduced in that task; keeping the unsupported-today wiring
//!    in this diff lets the scaffold and the walker land as two
//!    independently-reviewable PRs.
//!
//! 3. `healthcheck` runs the [`crate::auth::validate_auth`] probe.
//!    A green probe proves the stored Basic-auth credential still
//!    authenticates and the workspace URL still resolves — exactly
//!    what the "Test connection" button in Settings (DAY-83 UI) will
//!    want.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{FixedOffset, Utc};
use connectors_sdk::{ConnCtx, SourceConnector, SyncRequest, SyncResult, SyncStats};
use dayseam_core::{error_codes, DayseamError, ProgressPhase, SourceHealth, SourceId, SourceKind};
use tokio::sync::RwLock;

use crate::config::JiraConfig;
use crate::walk::walk_day;

/// One configured Jira source. Holds only the per-source
/// configuration that does **not** live in the [`connectors_sdk::BasicAuth`]
/// attached to each `ConnCtx`. Cloning is cheap — `JiraConfig` is a
/// `Clone` of two short strings.
///
/// `local_tz` is the user's configured timezone, threaded through from
/// `JiraMux::new` so the JQL walker can compute the correct UTC window
/// for a local day (DAY-77).
#[derive(Debug, Clone)]
pub struct JiraConnector {
    config: JiraConfig,
    local_tz: FixedOffset,
}

impl JiraConnector {
    /// Construct a connector handle for a single Jira source. `local_tz`
    /// defaults to UTC when the connector is built outside a
    /// [`JiraMux`]; production paths always go through the mux and
    /// inherit the orchestrator's configured offset.
    #[must_use]
    pub fn new(config: JiraConfig) -> Self {
        Self::with_local_tz(config, FixedOffset::east_opt(0).expect("0 offset"))
    }

    /// Construct a connector handle with an explicit `local_tz`. The
    /// mux uses this variant so every connector in the map shares
    /// whatever timezone the orchestrator was booted with.
    #[must_use]
    pub fn with_local_tz(config: JiraConfig, local_tz: FixedOffset) -> Self {
        Self { config, local_tz }
    }

    /// Borrow the configured workspace URL + email. Exposed for the
    /// Settings UI (and DAY-77 tests) to render "currently connected
    /// to `<workspace>`" text without having to reach into
    /// `BasicAuth::descriptor`.
    #[must_use]
    pub fn config(&self) -> &JiraConfig {
        &self.config
    }
}

#[async_trait]
impl SourceConnector for JiraConnector {
    fn kind(&self) -> SourceKind {
        SourceKind::Jira
    }

    async fn healthcheck(&self, ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        // Issue `GET /rest/api/3/myself` against the configured
        // workspace using whatever auth strategy the IPC layer
        // attached to this context. We deliberately do **not** reach
        // for `crate::auth::validate_auth` here: that helper is
        // specialised to the Add-Source flow (it consumes a
        // freshly-built `&BasicAuth`), while `healthcheck` has to
        // operate on the generic `Arc<dyn AuthStrategy>` the
        // orchestrator hands us — identical to how
        // `connector_gitlab::GitlabConnector::healthcheck` uses
        // `ctx.auth.authenticate(…)` rather than calling
        // `validate_pat` a second time.
        let url = self
            .config
            .workspace_url
            .join("rest/api/3/myself")
            .map_err(|e| DayseamError::InvalidConfig {
                code: "jira.config.bad_workspace_url".to_string(),
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

    async fn sync(&self, ctx: &ConnCtx, request: SyncRequest) -> Result<SyncResult, DayseamError> {
        ctx.bail_if_cancelled()?;

        let day = match request {
            SyncRequest::Day(d) => d,
            SyncRequest::Range { .. } | SyncRequest::Since(_) => {
                return Err(DayseamError::Unsupported {
                    code: error_codes::CONNECTOR_UNSUPPORTED_SYNC_REQUEST.to_string(),
                    message: "jira connector v0.2 only services SyncRequest::Day; \
                             Range + Since land with v0.3's incremental scheduler"
                        .to_string(),
                });
            }
        };

        ctx.progress.send(
            Some(ctx.source_id),
            ProgressPhase::Starting {
                message: format!("Fetching Jira activity for {day}"),
            },
        );

        let outcome = walk_day(
            &ctx.http,
            ctx.auth.clone(),
            &self.config.workspace_url,
            ctx.source_id,
            &ctx.source_identities,
            day,
            self.local_tz,
            &ctx.cancel,
            Some(&ctx.progress),
            Some(&ctx.logs),
        )
        .await?;

        ctx.progress.send(
            Some(ctx.source_id),
            ProgressPhase::Completed {
                message: format!(
                    "Jira fetched {} issue(s), emitted {} event(s)",
                    outcome.fetched_count,
                    outcome.events.len(),
                ),
            },
        );

        let stats = SyncStats {
            fetched_count: outcome.fetched_count,
            filtered_by_identity: outcome.filtered_by_identity,
            filtered_by_date: outcome.filtered_by_date,
            http_retries: 0,
        };

        Ok(SyncResult {
            events: outcome.events,
            artifacts: Vec::new(),
            checkpoint: None,
            stats,
            warnings: Vec::new(),
            raw_refs: Vec::new(),
        })
    }
}

/// Per-source configuration the [`JiraMux`] needs to hydrate one
/// [`JiraConnector`]. One entry per [`dayseam_core::SourceConfig::Jira`]
/// row; populated at startup (boot-only hydration, ARC-01) and
/// updated by the Add-Source / Reconnect flow in DAY-82.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JiraSourceCfg {
    pub source_id: SourceId,
    pub config: JiraConfig,
}

/// Multiplexing [`SourceConnector`] for Jira.
///
/// Semantically identical to [`connector_gitlab::GitlabMux`]: an
/// `Arc<RwLock<HashMap<SourceId, JiraConnector>>>` the Add-Source /
/// Reconnect flow can upsert into without rebuilding the registry.
/// `local_tz` is shared by every inner connector so a single user
/// timezone applies across all Jira workspaces.
#[derive(Debug, Clone)]
pub struct JiraMux {
    local_tz: FixedOffset,
    inner: Arc<RwLock<HashMap<SourceId, JiraConnector>>>,
}

impl Default for JiraMux {
    fn default() -> Self {
        Self::new(
            FixedOffset::east_opt(0).expect("0 offset"),
            std::iter::empty(),
        )
    }
}

impl JiraMux {
    /// Build a mux pre-populated with `sources`. Empty iterators are
    /// the common case at boot on a brand-new install.
    #[must_use]
    pub fn new(local_tz: FixedOffset, sources: impl IntoIterator<Item = JiraSourceCfg>) -> Self {
        let mut map = HashMap::new();
        for cfg in sources {
            map.insert(
                cfg.source_id,
                JiraConnector::with_local_tz(cfg.config, local_tz),
            );
        }
        Self {
            local_tz,
            inner: Arc::new(RwLock::new(map)),
        }
    }

    /// Add or replace the inner connector for `cfg.source_id`.
    pub async fn upsert(&self, cfg: JiraSourceCfg) {
        let conn = JiraConnector::with_local_tz(cfg.config, self.local_tz);
        self.inner.write().await.insert(cfg.source_id, conn);
    }

    /// Remove the inner connector for `source_id`, if any.
    pub async fn remove(&self, source_id: SourceId) {
        self.inner.write().await.remove(&source_id);
    }

    /// Test-only: how many sources are currently registered. The
    /// shipping code uses `get(&ctx.source_id)` instead.
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
impl SourceConnector for JiraMux {
    fn kind(&self) -> SourceKind {
        SourceKind::Jira
    }

    async fn healthcheck(&self, ctx: &ConnCtx) -> Result<SourceHealth, DayseamError> {
        match self.inner.read().await.get(&ctx.source_id).cloned() {
            Some(c) => c.healthcheck(ctx).await,
            None => Err(DayseamError::InvalidConfig {
                code: error_codes::IPC_SOURCE_NOT_FOUND.to_string(),
                message: format!("no Jira source registered for id {}", ctx.source_id),
            }),
        }
    }

    async fn sync(&self, ctx: &ConnCtx, request: SyncRequest) -> Result<SyncResult, DayseamError> {
        match self.inner.read().await.get(&ctx.source_id).cloned() {
            Some(c) => c.sync(ctx, request).await,
            None => Err(DayseamError::InvalidConfig {
                code: error_codes::IPC_SOURCE_NOT_FOUND.to_string(),
                message: format!("no Jira source registered for id {}", ctx.source_id),
            }),
        }
    }
}
