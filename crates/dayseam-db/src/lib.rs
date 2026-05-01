//! `dayseam-db` — the single persistence layer used by every other Dayseam
//! crate. Nothing else in the workspace touches SQLite directly; everything
//! flows through the typed repositories exposed here so we never sprinkle
//! ad-hoc SQL across connectors or the report engine.
//!
//! The schema is versioned via `sqlx::migrate!("./migrations")`, opening
//! a pool runs any pending migrations, and every repository round-trip is
//! covered by integration tests in `tests/repos.rs`. Crate `build.rs` prints
//! `cargo:rerun-if-changed` on the whole **`migrations/`** directory so any new
//! or edited `.sql` file rebuilds this crate (**MAS-4a**) — Cargo does not track
//! those paths for `sqlx::migrate!`'s compile-time embed on its own.

pub mod error;
pub mod pool;
pub mod repairs;
pub mod repos;

pub use error::{DbError, DbResult};
pub use pool::open;
pub use repairs::{registered_repairs, SerdeDefaultRepair};

pub use repos::{
    activity_events::ActivityRepo,
    artifacts::ArtifactRepo,
    drafts::DraftRepo,
    identities::IdentityRepo,
    local_repos::LocalRepoRepo,
    logs::{LogRepo, LogRow},
    persons::PersonRepo,
    raw_payloads::{RawPayload, RawPayloadRepo},
    settings::SettingsRepo,
    sinks::SinkRepo,
    source_identities::SourceIdentityRepo,
    sources::SourceRepo,
    sync_runs::SyncRunRepo,
};
