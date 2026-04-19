# Phase 2 — 3-day dogfood notes

**Plan item:** [Phase 2, Task 7.5 — Dogfood sweep](../plan/2026-04-18-v0.1-phase-2-local-git.md#task-7-first-run-empty-state--setup-sidebar--dogfood-polish)
**Branch (scaffold):** `DAY-49-phase-2-dogfood-notes`
**Semver label:** `semver:none`
**Owner:** `_<your name>_`

This is the written artifact of the Phase 2 dogfood sweep. The plan asks the
author to use Dayseam for their own EOD for three consecutive days, keep a
running notebook of tiny friction points, fix the easy ones on the spot, and
file issues for the rest so Phase 3 can pick them up.

This file exists as a **template** — it is committed empty so that (a) the
sweep has a known home the next PR can link to, and (b) the follow-up PR that
closes 7.5 is a pure content update, not a structural invention. Every field
below is an intentional placeholder; filling them in is the act that closes
the plan item.

---

## 1. Setup snapshot

Filled in once, at the start of Day 1.

| Field                  | Value                                  |
|------------------------|----------------------------------------|
| Build SHA              | `_<commit sha on master>_`             |
| Tauri dev or packaged  | `_<pnpm tauri:dev / packaged build>_`  |
| macOS version          | `_<e.g. 15.4>_`                        |
| Fresh or upgraded DB   | `_<fresh state.db / upgraded from v1>_`|
| Scan roots configured  | `_<absolute paths>_`                   |
| Identities configured  | `_<git emails / gitlab handles>_`      |
| Sinks configured       | `_<markdown-file path(s)>_`            |
| Self-person name       | `_<display name or "Me" sentinel>_`    |

Anything surprising about the first-run path (empty state, setup sidebar,
identity dialog) goes in Day 1 §3 rather than here.

---

## 2. Day-by-day log

Each day uses the same shape so the rollup table in §3 can be built by
concatenation. Keep entries small and concrete; this is a friction log, not a
design document.

### Day 1 — `_<YYYY-MM-DD>_`

**What I actually did today (so the generated report has something to chew
on):** `_<one or two sentences>_`

**Generate → Save loop:**

| Step                       | Observation                     |
|----------------------------|---------------------------------|
| Click Generate             | `_<result, duration, notes>_`   |
| Streaming preview          | `_<paints incrementally? lag?>_`|
| Evidence popover           | `_<links open? commit titles?>_`|
| Save → markdown file       | `_<marker block intact?>_`      |
| Re-open file in Obsidian   | `_<frontmatter valid?>_`        |

**Friction observations (Day 1):**

| ID    | Area                | Observation                         | Severity (L/M/H) | Next action            |
|-------|---------------------|-------------------------------------|------------------|------------------------|
| D1-01 | `_<UI / IPC / …>_`  | `_<what snagged me>_`               | `_<L/M/H>_`      | `_<Fix / Follow-up / Defer>_` |

### Day 2 — `_<YYYY-MM-DD>_`

**What I actually did today:** `_<one or two sentences>_`

**Generate → Save loop:**

| Step                       | Observation                     |
|----------------------------|---------------------------------|
| Click Generate             | `_<result, duration, notes>_`   |
| Streaming preview          | `_<paints incrementally? lag?>_`|
| Evidence popover           | `_<links open? commit titles?>_`|
| Save → markdown file       | `_<marker block intact?>_`      |
| Re-open file in Obsidian   | `_<frontmatter valid?>_`        |

**Friction observations (Day 2):**

| ID    | Area                | Observation                         | Severity (L/M/H) | Next action            |
|-------|---------------------|-------------------------------------|------------------|------------------------|
| D2-01 | `_<UI / IPC / …>_`  | `_<what snagged me>_`               | `_<L/M/H>_`      | `_<Fix / Follow-up / Defer>_` |

### Day 3 — `_<YYYY-MM-DD>_`

**What I actually did today:** `_<one or two sentences>_`

**Generate → Save loop:**

| Step                       | Observation                     |
|----------------------------|---------------------------------|
| Click Generate             | `_<result, duration, notes>_`   |
| Streaming preview          | `_<paints incrementally? lag?>_`|
| Evidence popover           | `_<links open? commit titles?>_`|
| Save → markdown file       | `_<marker block intact?>_`      |
| Re-open file in Obsidian   | `_<frontmatter valid?>_`        |

**Friction observations (Day 3):**

| ID    | Area                | Observation                         | Severity (L/M/H) | Next action            |
|-------|---------------------|-------------------------------------|------------------|------------------------|
| D3-01 | `_<UI / IPC / …>_`  | `_<what snagged me>_`               | `_<L/M/H>_`      | `_<Fix / Follow-up / Defer>_` |

---

## 3. Rollup

Filled in at the end of Day 3, before opening the 7.5 PR.

### 3.1 What worked

`_<bullet list of things that clicked — e.g. "evidence popover links opened in
VSCode on every commit", "marker block survived a manual Obsidian edit">_`

### 3.2 What didn't

`_<bullet list of things that consistently snagged across multiple days — e.g.
"date picker defaults to last-used rather than today", "toast stacking crosses
three lines on long paths">_`

### 3.3 Numbers worth recording

| Metric                                | Value                   |
|---------------------------------------|-------------------------|
| Median Generate duration (3 runs/day) | `_<ms>_`                |
| Worst Generate duration observed      | `_<ms>_`                |
| `log_entries` rows per day            | `_<count>_`             |
| Rust panics over 3 days               | `_<count, expected 0>_` |
| Unhandled JS promise rejections       | `_<count, expected 0>_` |

---

## 4. Findings and disposition

Every friction observation from §2 lands in exactly one row here. Shape
mirrors [`phase-1-review.md` §4](../review/phase-1-review.md#4-findings--resolutions)
so the Task 8 cross-cutting review can fold rows in without re-formatting.

| ID    | Area                | Finding                     | Disposition                  | Resolution     |
|-------|---------------------|-----------------------------|------------------------------|----------------|
| D1-01 | `_<area>_`          | `_<short recap>_`           | `_<Fix / Follow-up / Defer>_`| `_<PR #nn or issue #nn or "this PR">_` |

**Disposition rules (same as Phase 1 review):**

- `Fix` — landed in the 7.5 PR itself. Must link the commit SHA at merge.
- `Follow-up` — tracked as a merged follow-up PR before Task 8 opens. Must
  link the PR URL.
- `Defer` — intentionally pushed to Phase 3. Must link a filed issue and a
  one-sentence justification.

No row leaves this table without a resolution. Anything that feels like "I'll
deal with it later" becomes an explicit `Defer` with an issue, per the Phase 1
placeholder-scan discipline.

---

## 5. Exit checklist for Task 7.5

- [ ] Day 1 / Day 2 / Day 3 sections in §2 are filled in on three distinct
  calendar dates (weekend skip is allowed; record it).
- [ ] §3.3 numbers recorded from the actual runs, not estimated.
- [ ] Every observation in §2 has a matching row in §4 with a disposition.
- [ ] Every `Fix` row has a commit SHA; every `Follow-up` row has a merged PR
  URL; every `Defer` row has an issue URL.
- [ ] PR body for the 7.5 closure links this document and the
  [Phase 2 plan item](../plan/2026-04-18-v0.1-phase-2-local-git.md#task-7-first-run-empty-state--setup-sidebar--dogfood-polish).
- [ ] `CHANGELOG.md` has an entry under Unreleased that names this file.

Until every box is checked, Task 7.5 stays open. The Task 8 cross-cutting
review is allowed to start in parallel, but cannot close until 7.5 closes —
the Phase 2 exit criteria require it.
