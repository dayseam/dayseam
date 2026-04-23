# Cross-source enrichment parity audit — GitLab MR vs GitHub PR Jira-key extraction

**Filed:** 2026-04-20 (DAY-112)  
**Phase:** v0.5 test-quality hardening  
**Status:** Drift found → resolved by removing the divergent connector-level path; pinned with parity tests.

## Why this audit exists

v0.4's review §3 flagged that the cross-source enrichment pipeline is
the one place the GitLab, GitHub, and Jira connectors all touch the
same code. If GitLab MR Jira-key extraction uses one rule (e.g. titles
only) and GitHub PR extraction uses a different rule (titles + bodies,
or a looser/tighter regex, or a different per-event bail), reports
generated on mixed-forge days silently under- or over-report the
`jira_issue` ↔ MR/PR linking that the `(triggered by …)` annotation
relies on. DAY-112's first deliverable is this audit; the remediation
(code change + parity tests) lands in the same PR.

## The audit: what the code actually does today (v0.5.2 / master @ `e94c688`)

Three locations mention Jira-style ticket keys. Two of them extract;
one only consumes.

### Location 1: `crates/dayseam-report/src/enrich.rs::extract_ticket_keys` (centralized)

- **When it runs:** once, over the whole day's event stream, as the
  first step of the report pipeline at
  [`crates/dayseam-report/src/pipeline.rs:66`](../../crates/dayseam-report/src/pipeline.rs).
  Every event — commit, MR, PR, issue, Jira transition, Confluence
  page — passes through it regardless of source kind.
- **Scan surface:** event `title` + event `body` (see
  [`enrich.rs:89-93`](../../crates/dayseam-report/src/enrich.rs)).
- **Regex shape:** hand-rolled ASCII scanner at
  [`enrich.rs::scan_ticket_keys`](../../crates/dayseam-report/src/enrich.rs),
  accepting `/\b[A-Z]{2,10}-\d+\b/`. Prefix is **2–10 uppercase
  letters only** (no digits allowed in the prefix), then a single
  hyphen, then 1+ digits, bounded on both sides by non-alphanumeric.
- **Dedup rule:** per-event `sort` + `dedup` (first occurrence wins
  positionally but output is sorted).
- **Max-keys bail:** `MAX_TICKET_KEYS_PER_EVENT = 3` — if an event
  surfaces **more than 3** candidates between its title and body, the
  function attaches **zero** entities. The rationale (per the module
  docs and `extract_ticket_keys_bails_on_noisy_titles` at
  [`enrich.rs:653`](../../crates/dayseam-report/src/enrich.rs)): a
  commit title like `"Fix GH-123 by bumping LOG4J-2 from 2.17.0 to
  2.17.2"` contains tokens that syntactically match but aren't real
  tickets, and we'd rather attach nothing than attach the wrong three.
- **Label:** `EntityRef::label` is `None` (see
  [`enrich.rs:108`](../../crates/dayseam-report/src/enrich.rs)).
- **Idempotency:** checks `event.entities.iter().any(|e| e.kind ==
  EntityKind::JiraIssue && e.external_id == key)` before attaching,
  so a second pass over already-enriched events is a no-op.

### Location 2: `crates/connectors/connector-github/src/normalise.rs::extend_with_ticket_keys` (GitHub connector-local)

- **When it runs:** at normalise time, before the event is persisted.
  Called from `normalise_pull_request` (line 203), `normalise_issues_event`
  (line 260), and the `search/issues` rescue path in
  [`walk.rs:555`](../../crates/connectors/connector-github/src/walk.rs).
  **GitHub-only.** GitLab has no analogous call — see Location 3.
- **Scan surface:** **title only** (never body). Both normalise call
  sites pass `&pr.title` / `&issue.title`; the search-rescue path
  passes `&hit.title` (and explicitly sets `body: None` on the
  resulting event, so body isn't even available here).
- **Regex shape:** hand-rolled ASCII scanner at
  [`normalise.rs::ticket_keys`](../../crates/connectors/connector-github/src/normalise.rs),
  accepting `/\b[A-Z][A-Z0-9]+-\d+\b/`. Prefix is **1 uppercase
  letter + 1+ uppercase-or-digit chars** (digits allowed in the
  prefix after the first letter, unbounded length).
- **Dedup rule:** per-call `BTreeSet` guards duplicate emission.
- **Max-keys bail:** `MAX_TICKET_KEYS_PER_EVENT = 8`. No bail — just
  a hard cap. A PR with 30 references attaches the first 8.
- **Label:** `EntityRef::label` is `Some(key)` — the key text repeats
  as the label.
- **Idempotency:** not relevant; it runs exactly once at normalise.

### Location 3: `crates/connectors/connector-gitlab/` (GitLab connector — no connector-level extraction)

- `rg 'ticket|EntityKind::JiraIssue|jira_key' crates/connectors/connector-gitlab/ --type rust`
  returns **zero matches**. GitLab MRs do **not** carry a
  `EntityKind::JiraIssue` entity at normalise time; they acquire one
  only later via Location 1 in the report pipeline.

## The 4-dimension comparison

| Dimension | GitHub PR path (connector + report) | GitLab MR path (report only) |
|-----------|--------------------------------------|-------------------------------|
| Regex | Looser: `[A-Z][A-Z0-9]+-\d+` (digits allowed in prefix, no length cap) **plus** stricter `[A-Z]{2,10}-\d+` in the later enrich pass | Strict only: `[A-Z]{2,10}-\d+` |
| Scan surface | Title only (connector) **plus** title+body (enrich) | Title + body (enrich only) |
| Max-keys bail | Connector: hard cap of 8 (attach first 8); enrich: bail at >3 (attach zero) — but by the time enrich runs, connector may have already attached ≤8, and enrich's idempotency check only *prevents duplicates*, never *removes* pre-existing entities | Enrich: bail at >3 (attach zero) |
| Label | Connector: `Some(key)`; enrich: `None` — whichever path wins the first attachment sets the label for the rest of the pipeline's life | Always `None` |

## Concrete drift cases

Identical title + body content, identical day, one GitLab MR vs one
GitHub PR. After the full report pipeline runs:

| Title / body | GitLab MR entities | GitHub PR entities | Verdict |
|--------------|--------------------|--------------------|---------|
| `"CAR-5117: fix regression"` | 1 × `JiraIssue("CAR-5117", label=None)` | 1 × `JiraIssue("CAR-5117", label=Some("CAR-5117"))` | Cosmetic drift (label) — no render consumer, but the persisted row shape differs on disk. |
| Title only, body has `ACME-42` | 1 × `JiraIssue("ACME-42")` (enrich scans body) | 0 — connector scans title only; enrich adds `ACME-42`. **Same final count, same key.** Actually matches — body-only keys work for both forges because enrich handles them uniformly. | **No drift** for this case. |
| `"Bumping LOG4J-2 from 2.17.0 to 2.17.2"` | 0 — enrich's `[A-Z]{2,10}` rejects `LOG4J` (ends in digit). | 1 × `JiraIssue("LOG4J-2")` — connector's `[A-Z][A-Z0-9]+` accepts `LOG4J` (letter+letters+digit). Enrich's idempotency check sees the entity already attached and **does not remove it**. | **Drift.** GitHub PRs cross-link to phantom Jira keys that GitLab MRs with identical titles correctly ignore. |
| `"VERYLONGPROJECT-42"` (11-char prefix) | 0 — enrich's `{2,10}` rejects. | 1 × `JiraIssue("VERYLONGPROJECT-42")` — connector's unbounded prefix accepts. | **Drift.** |
| `"Fix CAR-1 CAR-2 CAR-3 CAR-4 - regression"` (4 real keys in title) | 0 — enrich's `>3` bail fires on the full title+body scan, attaches nothing. | 4 × `JiraIssue(...)` — connector attaches all 4 (under its 8-cap). Enrich's bail doesn't remove them. | **Drift.** GitHub over-reports against GitLab for the same noise pattern. |

Cases 3, 4, 5 are the failure classes. They are *not theoretical* —
PR / MR titles referencing CVEs (`CVE-2021-1234`), log library
versions (`LOG4J-2`), 3D tooling (`3D-2`), or long-prefix internal
project codes are all common enough that any reasonably-active
engineer will hit one within a week.

**The cosmetic label difference in case 1 has no downstream
consumer.** A workspace-wide grep for `label.*JiraIssue` /
`JiraIssue.*label` outside test code returns only the centralized
writes (`label: None`) and the read sites in
`dayseam-report::rollup.rs` (which reads `external_id`, never
`label`) and `dayseam-report::render.rs` (which formats the Jira
project label, not the per-issue label). It's a harmless drift —
recording it for completeness.

## Dimension: link-priority tie-breaking (cross-forge `triggered by …`)

The v0.5 plan specifically asked this dimension be audited. It lives
in
[`enrich.rs::annotate_transition_with_mr`](../../crates/dayseam-report/src/enrich.rs)
— the function that stamps `JiraIssueTransitioned` events with the
MR or PR that "triggered" them.

Reading the function:

- The candidate pool is built from MR-like activity kinds across
  **both forges** (`MrOpened`, `MrMerged`, `GitHubPullRequestOpened`,
  `GitHubPullRequestMerged`, `GitHubPullRequestClosed`) in one pass
  (see the `build_issue_to_mr_candidates` helper).
- The selection rule is *earliest `occurred_at`, tie-broken by
  `ActivityEvent::id`* — pure temporal order, completely
  forge-agnostic (`enrich.rs:136-138` in the module docs).
- The 24-hour lookback window applies uniformly.

**Verdict for link-priority: no drift.** The tie-breaking is
parity-by-construction today. Pinning test (parity test 4 below)
exists to keep it that way — a future refactor that, say, prefers
GitHub PRs when tied on `occurred_at` would fail it.

## Decision: what to change

The plan (DAY-112 step 5 under "if drift found") prescribed:

> Extract a shared `extract_jira_keys(title: &str, body: Option<&str>) -> Vec<JiraKey>` helper into `crates/connectors-sdk/src/jira_key_extraction.rs` (new module). [...] Refactor both GitLab and GitHub extraction paths to call the shared helper.

That prescription assumed symmetric connector-level paths. Reality is
asymmetric: **GitHub has a connector-level extractor; GitLab has
none.** Both forges ultimately flow through the same centralized
`extract_ticket_keys` in the report pipeline, which already *is* the
shared helper — it's just running after a divergent GitHub-only
pre-pass.

The right fix is therefore to **remove the divergent pre-pass**, not
to add a symmetric one on the GitLab side. Specifically:

- Delete `extend_with_ticket_keys` + `ticket_keys` + the
  `MAX_TICKET_KEYS_PER_EVENT` const from
  `crates/connectors/connector-github/src/normalise.rs`.
- Delete the `for key in crate::normalise::ticket_keys(&hit.title)`
  loop from `walk.rs:555-564`.
- Update the doc-comment at `normalise.rs:19-23` to say the Jira-key
  enrichment lives in the report layer for all connectors.
- Update / delete the three now-outdated GitHub connector tests
  (`pr_title_with_jira_ticket_key_enriches_entity_list`,
  `ticket_keys_extracts_unique_tokens_in_order`,
  `ticket_keys_ignores_lowercase_or_kebab_branches`) — the first is
  rewritten to pin the new contract (connector does *not* attach
  `JiraIssue`; the report pipeline does), the last two are deleted
  along with the function they tested.

The behaviour narrows on GitHub:

- `LOG4J-2`-class phantom keys stop attaching to GitHub PRs — now
  consistent with GitLab MRs.
- Long-prefix keys (`VERYLONGPROJECT-42`) stop attaching — now
  consistent.
- Titles with >3 noise keys now attach zero entities, same as
  GitLab — the v0.2-era design choice that we'd rather attach
  nothing than guess the wrong 3 now governs both forges.

The behaviour on GitLab is unchanged.

## Parity tests (pinning the new invariant)

Four integration tests live at
`crates/dayseam-report/tests/enrichment_parity.rs`. Each test builds
one GitLab MR event and one GitHub PR event with identical
`title` + `body` (and, for the cross-forge link-priority test, the
appropriate occurred_at offsets), runs both through the same report
pipeline, and asserts the emitted `JiraIssue` entity sets are
*exactly* equal.

1. **`gitlab_mr_and_github_pr_extract_same_jira_keys_from_title_plus_body`**
   — title carries `CAR-5117`, body carries `PROJ-42`. Both forges
   must emit `{"CAR-5117", "PROJ-42"}`.
2. **`gitlab_mr_and_github_pr_extract_same_jira_keys_from_body_only`**
   — title is plain, body carries `ACME-12`. Both forges must emit
   `{"ACME-12"}`.
3. **`gitlab_mr_and_github_pr_deduplicate_keys_identically`**
   — title and body both carry `CAR-5117`. Both forges must emit
   exactly one `JiraIssue("CAR-5117")` entity.
4. **`earliest_opened_wins_triggered_by_slot_across_forges`**
   — GitLab MR opens at `T1`, GitHub PR opens at `T2 > T1`, both
   reference `CAR-5117`, a Jira transition on `CAR-5117` at `T2 +
   10min`. The transition's `parent_external_id` must be the
   **GitLab MR's** external_id (earliest-opened wins), regardless of
   event-vec insertion order.

Each test asserts exact parity (`assert_eq!`), so any future
regression on either side — tightening GitLab's regex, loosening
GitHub's bail, changing the tie-breaker — fails the test.

## Would-have-caught (revert-probe verification)

Before shipping, I reverted the DAY-112 fix by restoring
`connector-github/src/normalise.rs::extend_with_ticket_keys` +
`ticket_keys` + `MAX_TICKET_KEYS_PER_EVENT` + their two call sites
(`walk.rs` + `normalise.rs`), then ran:

```
cargo test -p connector-github --lib normalise
cargo test -p dayseam-report --test enrichment_parity
```

Observed outcomes:

- **`pr_title_with_jira_ticket_key_does_not_attach_entity_at_normalise_time`
  failed red** — the drifted GitHub normaliser re-attaches
  `EntityKind::JiraIssue { external_id: "CAR-5117", label:
  Some("CAR-5117") }` at normalise time, violating the asserted
  invariant. This is the sentinel the fix is pinned on. If a future
  contributor re-adds any connector-local scanner on the GitHub
  side, this test turns red before the change ships.
- The 4 `enrichment_parity` tests stayed green under the revert,
  which is the expected layering: they build `ActivityEvent`s
  directly (no connector round-trip) and pin the centralized
  `dayseam_report::extract_ticket_keys` cross-forge invariant. The
  drift never lived in the central extractor, so the parity tests
  correctly refuse to claim a bug they weren't probing for.

Net: the drift-catching layer is the rewritten connector-github
normalise test (sentinel for "no connector pre-pass allowed"), and
the parity-locking layer is the 4 `enrichment_parity` tests
(sentinel for "central extractor behaves identically for both
forges"). Together they close the drift in two places so a
regression on either axis — bringing the pre-pass back, or
asymmetrizing the central extractor — fails fast.

Restoring the fix returned every test to green.

## Follow-ups (deferred)

None. The drift is closed, the parity is pinned, and the audit
closes TST-v0.4's cross-source-enrichment-parity gap.
