//! `connector-jira` — Dayseam's third
//! [`connectors_sdk::SourceConnector`] implementation.
//!
//! This crate owns Jira Cloud credential validation, `SourceKind::Jira`
//! registration with the orchestrator, the per-source `JiraConnector`
//! handle, and the per-day JQL walker that turns one Jira workspace
//! + one day into a vec of [`dayseam_core::ActivityEvent`].
//!
//! Heavy lifting shared with Confluence (ADF parsing, cursor
//! pagination, cloud-identity discovery, the nine-code error
//! taxonomy) lives one layer down in [`connector_atlassian_common`].
//!
//! ## Modules
//!
//! * [`auth`] — `validate_auth` + `list_identities`, both thin
//!   wrappers around [`connector_atlassian_common::discover_cloud`] /
//!   [`connector_atlassian_common::seed_atlassian_identity`]. The two
//!   entry points the IPC layer (DAY-82) calls when a Jira source is
//!   added.
//! * [`config`] — [`JiraConfig`]. Per-source configuration carried on
//!   the [`dayseam_core::SourceConfig::Jira`] row: just the workspace
//!   URL and the account email. The API token lives in the keychain
//!   via the source's `secret_ref`.
//! * [`connector`] — the [`SourceConnector`] implementation, the
//!   `JiraConnector` per-source handle, and the `JiraMux` that
//!   dispatches by `ctx.source_id` the way [`connector_gitlab::GitlabMux`]
//!   does for GitLab. `sync` on `SyncRequest::Day` drives
//!   [`walk::walk_day`]; `Range` / `Since` stay `Unsupported` until
//!   v0.3's incremental scheduler.
//! * [`walk`] — the JQL walker (DAY-77). One `POST
//!   /rest/api/3/search/jql` per page, cursor-paginated, day-window
//!   filtered, identity-filtered. Returns a [`walk::WalkOutcome`]
//!   the connector layer wraps into a
//!   [`connectors_sdk::SyncResult`].
//! * [`normalise`] — per-issue → zero-or-more
//!   [`dayseam_core::ActivityEvent`] mapping. One arm per v0.2
//!   `ActivityKind::Jira*` variant.
//! * [`rollup`] — the rapid-transition collapse. Consecutive
//!   same-author status changes within
//!   [`rollup::RAPID_TRANSITION_WINDOW_SECONDS`] fuse into one
//!   [`dayseam_core::ActivityKind::JiraIssueTransitioned`] event
//!   with a `transition_count` on the metadata, motivated by the
//!   spike's `CAR-5117` six-step cascade.
//!
//! [`SourceConnector`]: connectors_sdk::SourceConnector

pub mod auth;
pub mod config;
pub mod connector;
pub mod normalise;
pub mod rollup;
pub mod walk;

pub use auth::{list_identities, validate_auth};
pub use config::JiraConfig;
pub use connector::{JiraConnector, JiraMux, JiraSourceCfg};
pub use rollup::{CollapsedTransition, StatusTransition, RAPID_TRANSITION_WINDOW_SECONDS};
pub use walk::{walk_day, WalkOutcome};
