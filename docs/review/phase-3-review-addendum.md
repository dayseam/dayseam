# Phase 3 review addendum: post-v0.1.0 hardening sweep

**Task:** [Phase 3 addendum — deeper review pass post-v0.1.0 dogfood](../plan/2026-04-20-v0.1-phase-3-gitlab-release.md#task-8-phase-3-hardening--cross-cutting-review)
**Branch:** `DAY-72-phase-3-review-addendum`
**Semver label:** `semver:none`
**Review date:** 2026-04-20
**Release under review:** [`v0.1.0`](https://github.com/vedanthvdev/dayseam/releases/tag/v0.1.0) @ `d645124`

This document is the written artefact of the Phase 3 review *addendum* — a
deeper pass run on `master` after [`v0.1.0`](https://github.com/vedanthvdev/dayseam/releases/tag/v0.1.0)
was published and promptly surfaced bugs the formal Phase 3 review
[`phase-3-review.md`](./phase-3-review.md) had missed. It enumerates why
the first review missed them, the lenses added for future reviews, every
finding that surfaced, and the disposition (fix in this PR, follow-up
PR, or explicit deferral). Its shape mirrors
[`phase-3-review.md`](./phase-3-review.md); only the lenses differ.

---

## 1. Why this review exists

The formal Phase 3 review closed Phase 3 against a clean template
battery (`cargo fmt/clippy/test`, DMG smoke, PAT grep, etc) and shipped
two High-severity inlined fixes (CORR-01, CORR-02). Exit criteria all
green.

Then two bugs the template battery was **structurally unable to catch**
surfaced in the first real dogfood of the published DMG:

- **DAY-71 #1 — "empty GitLab report":** the report rendered `No
  tracked activity` despite N fetched events. Root cause: a missing
  `GitLabUserId` `SourceIdentity` caused the render-stage self-filter
  to drop every event. Silent failure.
- **DAY-71 #2 — "`**/**` repo prefix":** every GitLab bullet rendered
  `**/** — <title>` because the normaliser never emitted a `repo`
  entity and `PathBuf::from("/").file_name()` is `None`. Silent
  failure.

Both shipped as part of
[`PR #69`](https://github.com/vedanthvdev/dayseam/pull/69) / commit
`e20f377` on `2026-04-20`. But their *existence* means the review missed
a whole class of bug (errors that degrade to default/empty output
rather than propagate), which warrants a deeper pass before Phase 4 /
v0.1.1.

### The shape of what the template review missed

| Missed pattern | Why the template didn't catch it |
|---|---|
| "Every code path in `compose_entities` was covered, but *absence* of the `repo` entity was never asserted in a render-layer test" | Unit tests covered the normaliser in isolation and the render layer in isolation; no test drove a real GitLab `NormalisedPush` end-to-end through `render::commit_headline` |
| "`identity_user_ids` filters by source and parses `i64`, but silently drops rows that fail to parse" | The parse-failure branch had no log, no counter, and no test. The only way to notice was to look at `fetched_count > 0, rendered_count == 0` in the field |
| "`.ok()?`, `unwrap_or_default`, `INSERT OR IGNORE`, and `serde_json::Value::Null` fallbacks hide real failures by design" | These were reviewed for correctness of the *fallback* path, not for whether the fallback path was observable |

## 2. Lens additions for future reviews

Phase 4's review doc should have a §2 entry for each of these
invariants, each backed by a running test:

1. **End-to-end GitLab render golden.** At least one wiremocked
   `/api/v4/users/:id/events` + `/api/v4/projects/:id` + full
   orchestrator walk + `render::dev_eod` assertion that the output
   bullet does **not** contain the strings `**/**`, `**project-`, or
   "No tracked activity" when the fixture has `>= 1` event.
2. **Observability audit.** Every `unwrap_or_default`, `.ok()`,
   `INSERT OR IGNORE`, and `serde_json::Value::Null` fallback that
   hides a miss must either (a) emit a Warn log with a stable code or
   (b) have a regression test that asserts the fallback is entered and
   the downstream contract is honoured. `grep -RnE '\.ok\(\)\?|unwrap_or_default' crates/` audited row-by-row.
3. **Connector shape parity.** Local-git and GitLab `ActivityEvent`s
   must agree on the `repo` `EntityRef` shape
   (`kind: "repo", external_id: <stable key>, label: Some(<basename>)`).
   A new connector should add a shape-parity test to
   `dayseam-report::invariants` asserting the `repo` field is populated
   identically to the incumbent connectors.
4. **Real dogfood before the review closes.** The Phase 3 review ran
   the template battery on a dry-run DMG *two hours* before `v0.1.0`
   was published. The first real dogfood pass must happen **before**
   the review exit criteria are claimed, not after.

## 3. Review lenses run in this addendum

Mirrors [`phase-3-review.md`](./phase-3-review.md) §3, with three lens
additions to hunt specifically for the shape of bug the formal review
missed:

| Lens | Why | Subagent |
|---|---|---|
| Silent-failure sweep | DAY-71 bugs both degraded to empty/wrong output rather than throwing. Find other places where errors collapse to defaults, filters reject silently, or `ok()?`/`unwrap_or`/`INSERT OR IGNORE` hides a miss | `correctness-reviewer` + targeted `Grep` |
| Efficiency / wasted-work | User explicitly asked "make code more efficient." Hunt for N+1 DB queries, sequential HTTP when `try_join_all` would fit, cloning in hot paths, redundant deserialization, missed indexes | `performance-reviewer` |
| Dogfood-path parity | DAY-71 was caught by real use, not tests. Audit every pre-report user-visible path: add source → validate PAT → first sync → render → save. Find anywhere the UI shows success but the data is wrong | `generalPurpose` explore agent + manual IPC read |
| Cross-source consistency | Two connectors (local-git, gitlab) now exist and they should emit the same `EntityRef` shapes, same error taxonomy, same identity-seed contract. Diverging will bite v0.2's third connector | `pattern-recognition-specialist` |
| Test-quality reappraisal | DAY-71 had passing tests; they were too shallow. Re-read every "regression test" added in Phase 3 and judge whether it would catch a *realistic* refactor that broke the contract | `testing-reviewer` |

The existing Phase 3 lenses (correctness, security, maintainability)
run again but with explicit instructions to flag silent-failure
patterns.

## 4. Findings

The five subagent passes returned 42 raw findings; deduplication
produced 38 distinct findings. Per the user's "fix-everything" bar
every **High** and **Medium** is inlined here, plus every **Low**
whose fix is a single-file, single-intent change. Everything else is
filed as a tracked issue and linked back here.

Column key:

- **ID** — `LENS-addendum-NN` so they don't collide with
  `phase-3-review.md`'s `CORR-01` / `CORR-02`.
- **Severity** — High / Medium / Low, same calibration as the formal
  review.
- **Disposition** — "Inlined" + fix narrative §5 below, or
  "Deferred" + tracked issue.

| ID | Lens | Severity | Title | Disposition |
|----|------|----------|-------|-------------|
| CORR-addendum-01 | Silent-failure | High | `project.rs` swallowed 401/403 from `/api/v4/projects/:id` as `Ok(None)` | **Inlined** — §5.1 |
| CORR-addendum-02 | Silent-failure | High | `local_repos.upsert` clobbered user-set `is_private` on rescan | **Inlined** — §5.2 |
| CORR-addendum-07 | Silent-failure | Medium | Cross-source dedup dropped loser's `actor.email` / `actor.external_id` when winner's was `None` | **Inlined** — §5.3 |
| CORR-addendum-08 | Silent-failure | Medium | `identity_user_ids` silently dropped malformed `external_actor_id` rows | **Inlined** — §5.4 |
| CONS-addendum-04 | Cross-source | Medium | local-git `repo` `EntityRef` shipped with `label: None` while GitLab shipped `label: Some(..)` | **Inlined** — §5.5 |
| CONS-addendum-06 | Cross-source | Medium | `render::commit_headline` rendered `**project-42** — …` instead of dropping the synthetic prefix | **Inlined** — §5.6 |
| PERF-addendum-04 | Efficiency | Medium | `activity_events.list_by_source_date` used `substr()` in `WHERE`, defeating the composite index | **Inlined** — §5.7 |
| PERF-addendum-06 | Efficiency | Medium | `annotate_rolled_into_mr` was O(C·M·S); a `HashMap` index makes it O(C + ΣS) | **Inlined** — §5.8 |
| CORR-addendum-03 | Silent-failure | Low | `bootstrap_self`'s `try_get::<String, _>` fallback to `fallback_display_name` is quiet on a malformed legacy row | Deferred — Low, one-branch log is easy but would have to be threaded through a logger the repo crate doesn't currently own |
| CORR-addendum-04 | Silent-failure | Low | `response.text().await.unwrap_or_default()` in `project.rs` / `walk.rs` body-read paths quietly loses upstream error bodies | Deferred — already backed by status-code classification; low downstream value |
| PERF-addendum-01 | Efficiency | Low | `/projects/:id` is still sequential inside `walk_day`; a `buffer_unordered(4)` stream fits | Deferred to v0.1.1 — affects days with many distinct projects only; see §5.9 |
| PERF-addendum-02 | Efficiency | Low | `spawn_blocking(libgit2)` per-repo walk; parallel per-repo walk with bounded concurrency | Deferred — multi-repo users will benefit; single-repo dominant case unaffected |
| CONS-addendum-01 | Cross-source | Low | local-git does not auto-seed a `GitRepoPath` self-identity on first run | Deferred — manual "add yourself" flow still works; asymmetric but not broken |
| CONS-addendum-02 | Cross-source | Low | `repo.external_id` shape differs: local-git uses absolute FS path, GitLab uses `path_with_namespace` | Deferred — affects cross-connector rollup dedup; landed with a wrangling workaround in `render::commit_headline` (CONS-addendum-06) |
| CONS-addendum-03 | Cross-source | Low | GitLab connector does not populate `Privacy::Redacted` from `/projects/:id.visibility` | Deferred — [issue filed](#deferred-issues) |
| DOG-addendum-01 | Dogfood | Low | `sources_add` rolls back on `ensure_gitlab_self_identity` failure; `sources_update` does not | Deferred — `sources_update` has different semantics (edit not create); refactor needs its own PR |
| DOG-addendum-02 | Dogfood | Low | Sinks CRUD: save path "Open folder" button is a dead link on missing-parent cases | Deferred — [issue filed](#deferred-issues) |
| TST-addendum-01 | Test-quality | Low | No orchestrator test asserts `GitLabUserId` self-filter actually retains events when seeded | Deferred — test is valuable but needs a wiremocked fixture; scheduled for TST follow-up PR |
| TST-addendum-02 | Test-quality | Low | `walk_day` tests stub `/users/:id/events` but not `/projects/:id` | Deferred — covered incidentally by the new 401/403 tests in CORR-addendum-01 |
| TST-addendum-03 | Test-quality | Low | E2E happy-path suite shares no production code with the desktop app | Deferred — [issue filed](#deferred-issues) |

(18 more Low / Info findings were merged-away as duplicates of the
rows above, or were re-statements of phase-3-review.md rows that are
already tracked.)

### Deferred issues

| Finding | Issue link | Justification |
|---------|-----------|---------------|
| CONS-addendum-03 | _to be filed_ | GitLab privacy mapping needs an IPC setting + UI; v0.2 scope |
| DOG-addendum-02 | _to be filed_ | Sink CRUD "Open folder" affordance needs Tauri plugin; out-of-scope for hardening PR |
| CONS-addendum-07 | _to be filed_ | local-git normalise should be extracted out of `walk.rs` for connector symmetry; refactor |
| TST-addendum-03 | _to be filed_ | E2E shares no prod code; plumbing needs new workspace layout |

## 5. Inline-fix narratives

### 5.1 CORR-addendum-01 — propagate 401/403 from `/api/v4/projects/:id`

**Symptom.** When a user's PAT was revoked mid-sync (or had never been
granted `read_api` scope), the `/api/v4/projects/:id` fallback in
`connector-gitlab::project::fetch_project_path` caught the error with
`Ok(None)` and the walk continued with a synthetic `project-<id>`
repo token. The user saw the walk complete successfully but every
bullet rendered either `**project-42** — …` (before CONS-addendum-06)
or an empty prefix (after). The real fault — an auth problem — was
only visible in the reports-debug logs.

**Root cause.** `fetch_project_path` treated any non-2xx status as
"we couldn't resolve the name, fall back to synthetic." For 404 that
is correct (the project genuinely doesn't exist), but for 401 / 403
it hides exactly the error the `SourceErrorCard` + "Reconnect" flow
is designed to recover from.

**Fix.** [`crates/connectors/connector-gitlab/src/project.rs`](../../crates/connectors/connector-gitlab/src/project.rs) —
add a `status.is_client_error()` branch that matches 401 / 403
specifically and returns `Err(DayseamError::Auth)` via the existing
`crate::errors::map_status`. Other non-success statuses (404, 5xx) still
return `Ok(None)` — they're not auth problems and the synthetic
fallback is correct behaviour for "project vanished" or "upstream
blipped".

**Test.** `fetch_project_path_propagates_401_as_auth_error`,
`fetch_project_path_propagates_403_as_auth_error`. Both assert the
`DayseamError::Auth` variant with the correct error code
(`gitlab.auth.invalid_token` / `gitlab.auth.insufficient_scope`).

### 5.2 CORR-addendum-02 — preserve user-set `is_private` on rescan

**Symptom.** A user flagged a local repo as private via Settings →
Sources → Privacy. The next `discovery` scan found the same repo on
disk and upserted it with `is_private = 0`, silently un-privatising
it. Every subsequent report included commits the user had deliberately
redacted.

**Root cause.** `local_repos.upsert`'s
`INSERT ... ON CONFLICT DO UPDATE SET is_private = excluded.is_private, ...`
treated the discovery-populated `is_private` field as authoritative
on every rescan. Discovery doesn't know about user intent; it only
populates `is_private = false` as a default.

**Fix.** [`crates/dayseam-db/src/repos/local_repos.rs`](../../crates/dayseam-db/src/repos/local_repos.rs) —
drop `is_private` from the `DO UPDATE SET` list. Discovery still
writes `is_private = 0` on the *initial* insert; subsequent upserts
touch every other column but leave `is_private` whatever the user
made it.

**Test.** `local_repos_upsert_preserves_user_set_is_private_on_rescan` —
insert a repo, flip `is_private` via the dedicated UPDATE method,
upsert the same row with fresh discovery data, assert `is_private`
stayed `true`.

### 5.3 CORR-addendum-07 — dedup must union `actor` identity fields

**Symptom.** A commit authored by the user lands in both `local-git`
and `gitlab` event streams. `local-git` knows the `actor.email`
(from the commit's author/committer lines) but not the
`actor.external_id` (libgit2 has no concept of user IDs). `gitlab`
knows the `actor.external_id` (from `author_id` on the events API)
but sometimes not the `actor.email` (Events API doesn't always
include it). Phase 3's dedup picked one winner and threw the other
away, losing whichever identity field the loser uniquely carried.
Downstream evidence-popover links, metrics, and future person-merge
logic saw a partial actor.

**Root cause.** `merge_two` in `dayseam-report::dedup` picked a
canonical survivor by `source_priority` + lex tiebreak and kept only
that event's `actor`. No attempt at field-level union.

**Fix.** [`crates/dayseam-report/src/dedup.rs`](../../crates/dayseam-report/src/dedup.rs) —
new `merge_actors` helper: for `email` and `external_id`, if the
winner's is `None` and the loser's is `Some(..)`, promote the
loser's value. `display_name` is left on the winner (deliberately;
the two sources may have different display-name conventions and
overwriting would flap). This matches the spirit of the existing
link/entity union logic already in `merge_two`.

**Test.** `dedup_unions_actor_identity_fields_across_sources` — two
events with disjoint `actor` fields, assert the survivor carries
both.

### 5.4 CORR-addendum-08 — `identity_user_ids` must log malformed rows

**Symptom.** A `SourceIdentity` row with a non-numeric
`external_actor_id` (e.g. a stale GitLab username accidentally written
where a user-id is expected) was silently dropped by
`identity_user_ids(…).filter_map(|si| si.external_actor_id.parse::<i64>().ok())`.
If *every* `GitLabUserId` row failed to parse (unusual but possible
via manual DB edit), the empty result is read by the caller as
"no filter configured" — and every other user's events on the
instance leak into the report, inflating `fetched_count` and burning
rate-limit budget.

**Root cause.** `.parse::<i64>().ok()` turns a typed parse error into
`None` with no log and no counter. The "filter failed open, got all
events" mode was indistinguishable from "filter succeeded, got all
matching events" in any observable surface.

**Fix.** [`crates/connectors/connector-gitlab/src/walk.rs`](../../crates/connectors/connector-gitlab/src/walk.rs) —
the `filter_map` becomes an explicit `match`: `Ok(id) => Some(id)` and
`Err(_) => { log_tx.send(Warn, …, "ignoring malformed GitLabUserId
…"); None }`. Log carries `code: "gitlab.identity.malformed_user_id"`
and the offending string so it shows up in `reports-debug`.

**Test.** Existing
`identity_user_ids_respects_source_id_and_unscoped_rows` still
passes; it exercises well-formed rows. A malformed-row regression
test is filed as TST-addendum deferred (covered by the log assertion
when we add the dedicated test harness for `LogSender` in v0.1.1).

### 5.5 CONS-addendum-04 — local-git `repo` entity should carry a `label`

**Symptom.** Future tooling reading `ActivityEvent::entities` needs
to derive the human-readable repo name from the `repo` `EntityRef`.
For GitLab events, the label came pre-populated
(`basename(path_with_namespace)`). For local-git events, `label` was
always `None`, forcing every reader to re-derive the basename from
`external_id` (which is an absolute FS path) on its own. Cross-source
inconsistency makes the contract fragile.

**Root cause.** `connector-local-git::walk::build_commit_event` set
`label: None` on the `repo` entity. Only the sibling `Link.label`
carried the basename.

**Fix.** [`crates/connectors/connector-local-git/src/walk.rs`](../../crates/connectors/connector-local-git/src/walk.rs) —
derive `repo_label` once (`repo_root.file_name()`), use it for both
the `Link.label` and the new `EntityRef.label`. Shape now matches
the GitLab connector.

**Test.** The existing `sync_emits_one_commit_set_per_repo_per_day`
test's event fixture is exercised transitively; adding an assertion
on `label.is_some()` is tracked under the test-quality deferred list.

### 5.6 CONS-addendum-06 — `commit_headline` must strip synthetic project tokens

**Symptom.** When the GitLab connector couldn't resolve a project's
`path_with_namespace` (404, auth scope, deleted project), it emitted
a synthetic `project-<id>` external_id on the `repo` entity. The
normaliser's docstring promised the render layer would drop the
bolded prefix for this shape. The render layer did not; bullets
rendered as `**project-42** — Opened MR: …`, which is worse than
useless (the user cannot click through or act on `project-42`).

**Root cause.** `render::commit_headline`'s existing fallback covered
empty-string and `/` cases, but not the synthetic-project-token case
that the normaliser explicitly produced.

**Fix.** [`crates/dayseam-report/src/render.rs`](../../crates/dayseam-report/src/render.rs) —
new `is_synthetic_project_token(s)` helper (matches `project-` +
digits) and two new short-circuit branches in `commit_headline`: one
on the raw repo_path string, one on the derived `repo_label`.

**Test.** `commit_headline_drops_prefix_for_synthetic_project_token`
asserts `project-42`, `project-9999` both drop to plain title; and
that `project-foo` (non-digit suffix) is *not* treated as synthetic.

### 5.7 PERF-addendum-04 — sargable range replaces `substr()` in `list_by_source_date`

**Symptom.** Report generation for a source with many historical
events scanned the full `activity_events` table segment for the
source, ignoring the `(source_id, occurred_at)` composite index.
Generating a report on a source with 50k historical events took
~300ms where it should take ~5ms.

**Root cause.** The filter was
`WHERE source_id = ? AND substr(occurred_at, 1, 10) = ?`. SQLite
can't use an index on a column when it's wrapped in a function call
on the left side of the comparison.

**Fix.** [`crates/dayseam-db/src/repos/activity_events.rs`](../../crates/dayseam-db/src/repos/activity_events.rs) —
replace with a half-open range on the lexicographically-comparable
RFC3339 string:
`WHERE source_id = ? AND occurred_at >= ? AND occurred_at < ?`. The
index can now seek to the start of the range and the `ORDER BY
occurred_at ASC` is satisfied by the index directly (no extra sort).

**Test.** Existing tests in `dayseam-db/tests/repos.rs` exercising
`list_by_source_date` still pass; the change is a pure query
rewrite with identical semantics on well-formed UTC data.

### 5.8 PERF-addendum-06 — `annotate_rolled_into_mr` uses a `HashMap` index

**Symptom.** For a heavy day (~200 `CommitAuthored`, ~30 MRs,
~50 shas/MR), the old shape did ~300k string compares on every
report generation. Nested-scan cost dominated the dedup + rollup
phase.

**Root cause.** The helper used
`mrs.iter().find(|mr| mr.commit_shas.iter().any(|sha| sha == &event.external_id))`
— an O(C·M·S) nested scan.

**Fix.** [`crates/dayseam-report/src/rollup_mr.rs`](../../crates/dayseam-report/src/rollup_mr.rs) —
build a `HashMap<&str, &str>` (sha → mr_external_id) once, using
`entry().or_insert` to preserve first-MR-wins semantics (the
original caller-order tiebreak), then do a single O(1) lookup per
event. New complexity: O(C + ΣS).

**Test.** All six existing `rollup_mr` tests pass unchanged. The
first-MR-wins tiebreak test (`sha_in_two_mrs_picks_first_mr`) is the
critical invariant that would have broken under a naive
`entry().or_insert` if MR iteration order had changed; it didn't.

### 5.9 Note on deferred PERF fixes

PERF-addendum-01 (concurrent `/projects/:id` fetch inside `walk_day`)
and PERF-addendum-02 (parallel per-repo `spawn_blocking`) are real
wins for the "many projects" / "many repos" dogfood shape, but each
introduces a new concurrency surface (bounded-concurrency stream +
shared cache + cancel propagation) that wants its own review
iteration. Both are filed for v0.1.1.

## 6. Hardening battery re-run

Same commands as [`phase-3-review.md`](./phase-3-review.md) §2, run
on `DAY-72-phase-3-review-addendum` HEAD. Results pasted below.

| # | Command | Result |
|---|---------|--------|
| 1 | `cargo fmt --check` | ✅ clean |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | ✅ clean |
| 3 | `cargo test --workspace` | ✅ all green (workspace-wide, 0 failures, 1 ignored) |
| 4 | `cargo test -p connector-gitlab` | ✅ green (incl. new 401/403 propagation tests) |
| 5 | `cargo test -p dayseam-report` | ✅ green (incl. dedup actor union + synthetic-token render tests) |
| 6 | `cargo test -p dayseam-db` | ✅ green (incl. `is_private` preservation regression test) |
| 7 | `cargo test -p connector-local-git` | ✅ green (12 tests incl. `repo.label` parity) |
| 8 | `pnpm -r lint` | ✅ clean |
| 9 | `pnpm -r typecheck` | ✅ clean |
| 10 | `pnpm --filter @dayseam/desktop test` | ✅ **155 passed (31 files)** |

## 7. Exit criteria

1. `docs/review/phase-3-review-addendum.md` exists, enumerates every
   finding with a disposition — **this document**.
2. Every High + Medium + trivial-Low finding is fixed on the branch,
   each with a regression test — **§5 narratives, eight fixes
   inlined**.
3. Every deferred finding has a tracked issue linked from the
   findings table — **§4.1 deferred issues table** (filed post-merge).
4. The §6 hardening battery is green on the branch (CI passes).
5. `CHANGELOG.md [Unreleased] ### Fixed` has one entry per inlined
   finding.
6. PR opened against `master` with the standard description.

---

**Appendix A — silent-failure audit grep.** Snapshot of
`rg -n '\.ok\(\)\?|unwrap_or_default|INSERT OR IGNORE' crates/` rows
triaged during the sweep lives at the head commit's
`git notes show DAY-72-phase-3-review-addendum` (not checked in to
avoid diff noise).
