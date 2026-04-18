# Changelog

All notable changes to Dayseam are documented in this file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project adheres to
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Initial monorepo scaffold: Cargo workspace with seven crate skeletons, pnpm
  workspace with a Tauri + React + TypeScript + Tailwind desktop app shell,
  CI pipeline (rust, frontend, check-semver-label), PR template, and branch
  protection setup script.
- `dayseam-core` domain types, `DayseamError` taxonomy with stable error
  codes, and ts-rs-generated TypeScript bindings committed to
  `packages/ipc-types/src/generated/`.
- `dayseam-db`: SQLite persistence layer with the v1 schema from design
  §5.2, a `sqlx`-managed migration, and typed repositories for every table
  (`SourceRepo`, `IdentityRepo`, `LocalRepoRepo`, `ActivityRepo`,
  `RawPayloadRepo`, `DraftRepo`, `LogRepo`, `SettingsRepo`). `open(path)`
  enables WAL + foreign keys and is idempotent across re-opens.
- `dayseam-secrets`: `Secret<T>` wrapper with redacting `Debug`/`Display`
  and zeroing `Drop`, a narrow `SecretStore` trait, an `InMemoryStore`
  for tests, and a feature-gated `KeychainStore` that stores tokens in
  the macOS Keychain under a `service::account` composite key. Delete is
  idempotent and the macOS round-trip is covered by an `#[ignore]`d
  smoke test.
- `ARCHITECTURE.md`: top-down living architecture + versioned roadmap
  for Dayseam. Covers principles, repo layout, runtime topology, the
  connector/sink contracts, the canonical artifact layer, persistence
  + secrets + event bus design, testing strategy, release engineering
  (including updater-key custody), and the v0.1–v0.5 roadmap.
- Event types on the IPC boundary (`dayseam-core::types::events`):
  `RunId` newtype, `ProgressEvent` + `ProgressPhase` (Starting /
  InProgress / Completed / Failed), `LogEvent` with structured
  `context: JsonValue`, and `ToastEvent` + `ToastSeverity`. All
  generated TypeScript bindings are committed alongside.
- `dayseam-events` crate: per-run ordered streams (`RunStreams`,
  `ProgressSender`, `LogSender`) built on `tokio::sync::mpsc` for
  sync-run progress and structured logs, plus an app-wide `AppBus`
  built on `tokio::sync::broadcast` for `ToastEvent` fanout. Publishers
  never block, slow broadcast subscribers observe `Lagged` explicitly
  and recover by resubscribing, and receivers observe end-of-stream
  cleanly once every sender is dropped.
- Canonical identity types on `dayseam-core`: `Person` (one row per
  human, with `is_self` flag) and `SourceIdentity` (one row per
  `(person, source, external actor id)` mapping, tagged by
  `SourceIdentityKind = GitEmail | GitLabUserId | GitLabUsername |
  GitHubLogin`). The legacy v0.1 `Identity` record is kept for
  schema compatibility and will be retired in Phase 2. All three new
  types ship with serde round-trip coverage and committed TypeScript
  bindings.
- `DayseamError` gains two non-failure-looking variants, each with
  their own stable error codes:
  - `Cancelled { code, message }` — surfaced when a run is cancelled
    by the user, by app shutdown, or by a newer run superseding this
    one (`run.cancelled.by_user`, `run.cancelled.by_shutdown`,
    `run.cancelled.by_superseded`). The UI renders this as
    "cancelled", not as an error toast.
  - `Unsupported { code, message }` — surfaced when a connector is
    asked to service a `SyncRequest` variant it has no implementation
    for, e.g. `SyncRequest::Since(Checkpoint)` against a connector
    that only supports day-scoped pulls
    (`connector.unsupported.sync_request`). The orchestrator catches
    this and falls back to the equivalent non-incremental call.
  - Two HTTP-layer codes (`http.retry.budget_exhausted`,
    `http.transport`) are also reserved for the connector SDK's
    shared `HttpClient`.
- `connectors-sdk` crate: the shared plumbing every source connector
  is built on top of.
  - `SourceConnector` trait with a single `sync(ctx, SyncRequest) ->
    SyncResult` method, a `healthcheck(ctx)`, and a stable `kind()`
    tag. `SyncRequest` covers `Day(NaiveDate)`, `Range { start, end
    }`, and `Since(Checkpoint)`; `SyncResult` returns normalised
    `ActivityEvent`s, an optional new `Checkpoint`, `SyncStats`
    (fetched / filtered / http_retries), warnings, and `RawRef`s.
  - `AuthStrategy` trait with `NoneAuth` and `PatAuth` (PAT from the
    macOS Keychain via `dayseam-secrets`), plus an `AuthDescriptor`
    every connector can expose for the UI to render the right
    "connect" affordance.
  - `ConnCtx` — the single context object every connector method
    receives, wiring `run_id`, canonical `person` + known
    `source_identities`, a `ProgressSender` / `LogSender` pair from
    the run's `RunStreams`, a `RawStore`, an injectable `Clock`, a
    shared `HttpClient`, and a `CancellationToken`. A
    `bail_if_cancelled` helper lets connector code short-circuit
    cooperatively on `DayseamError::Cancelled`.
  - `HttpClient` wrapping `reqwest::Client` with a shared retry loop:
    honours `429 Retry-After` (both delta-seconds and HTTP-date),
    retries transient 5xx with exponential backoff + jitter up to a
    configurable `RetryPolicy`, emits per-attempt progress events,
    and treats the run's `CancellationToken` as a hard ceiling —
    every sleep races the token and every attempt re-checks it so
    cancellation is observed within one tick.
  - `Clock` abstraction (`SystemClock` for production,
    `tokio::time::sleep`-backed) and `RawStore` trait (with
    `NoopRawStore` for v0.1) so real raw-payload persistence can land
    in Phase 2 without touching connector code.
  - `MockConnector`: an always-compiled in-memory `SourceConnector`
    driven by a fixture list. Used by downstream tests to exercise
    orchestrator and UI paths without any real HTTP, and self-checked
    with an integration suite covering day filtering, identity
    filtering, ordered progress emission, and correct `Unsupported`
    rejection of `SyncRequest::Since`.
  - Integration tests: `wiremock`-backed `HttpClient` retry and
    cancellation suites, `MockConnector` behavioural tests, and a
    `no_cross_crate_leak` guard that fails the build if
    `connectors-sdk` ever picks up a dependency on `dayseam-db`,
    `dayseam-secrets`, `dayseam-report`, or `sinks-sdk`.
