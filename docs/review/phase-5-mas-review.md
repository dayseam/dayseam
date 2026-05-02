# Phase 5 (MAS) capstone review

**Task:** **MAS-9a** — full review + written artefact ([plan — Block MAS-9](../plan/2026-phase-5-mas-app-store.md#mas-block-9-capstone))  
**Tracking issue:** [#210](https://github.com/dayseam/dayseam/issues/210) (Phase 5 umbrella)  
**Branch:** `DAY-210-mas-9a-phase5-review` · **PR:** [#242](https://github.com/dayseam/dayseam/pull/242) *(scaffold; update when superseded by the completion PR)*  
**Semver label:** *(typically `semver:patch` when closing **MAS-9a** with substantive findings; `semver:none` is OK for doc-only scaffolding PRs)*  
**Review date:** *(YYYY-MM-DD when sign-off is recorded)*  
**Release / commit under review:** *(tag or merge-base range for the MAS track being closed)*

This document is the written artefact of the **MAS-9a** capstone review. It
enumerates what was reviewed, how it was reviewed, findings, and resolution
(fix in-PR, follow-up issue, or explicit deferral). Its shape follows earlier
phase reviews (for example [`phase-3-review.md`](./phase-3-review.md)); the
lenses below are **MAS-specific** per the Phase 5 plan.

**Downstream tasks:** **MAS-9b** (P0/P1 bug sweep for **bar A**) and **MAS-9c**
(dogfood evidence) consume this doc; keep cross-links updated.

---

## 0. Exit bars (normative — from plan)

Stakeholders must not conflate **“submission-ready”** with **“release
automation complete.”** State explicitly how this review closes each bar.

| Bar | Definition | Status in this review |
|-----|------------|------------------------|
| **A. Engineering complete** | Sandboxed MAS build is **store-compatible**: real shell, bookmarks + Keychain + OAuth + connectors + sinks under sandbox; **manual** upload to App Store Connect succeeds if that is the path in use; **MAS-9a** (this doc) + **MAS-9c** dogfood done; **MAS-9b** clears P0/P1 for bar A. | *TBD — yes / partial / no + notes* |
| **B. Release automation complete** | Automated Connect upload on the same cadence as GitHub Releases (or successor); export compliance docs align with upload metadata. | *TBD — **MAS-8d** merged: [`mas-connect-upload.yml`](../../.github/workflows/mas-connect-upload.yml) + [`MAS-CONNECT-UPLOAD.md`](../release/MAS-CONNECT-UPLOAD.md); store-signed **`.pkg` in CI** may remain **MAS-8d-followup** — state which path was used for evidence.* |

**MAS-9a** must state whether production uploads were **manual** or **automated**
(per plan executive summary).

---

## 1. Canonical MAS smoke (dogfood evidence — copy results into §5 or appendix)

Reuse on every manual / dogfood pass ([plan source](../plan/2026-phase-5-mas-app-store.md#canonical-mas-smoke-checklist)):

1. Cold **launch** (MAS build).
2. **Open folder** picker; grant path; **quit and relaunch** — access still works for scan/sink.
3. **Local Git** scan on nested repo layout (per **MAS-0b** symlink policy).
4. **Save** report to Markdown sink in permitted folder.
5. **Reconnect** a secret-backed source (rotate or re-validate).
6. **OAuth** complete flow (e.g. Outlook).
7. **No** updater UI or updater network calls.
8. **Upgrade** from previous MAS build (if applicable).

---

## 2. Inventory (fill before deep lenses)

### 2.1 Baseline and head

| | Commit | Label |
|---|--------|-------|
| Baseline | *TBD* | *e.g. last commit before Phase 5 MAS track (or MAS-1a merge)* |
| Head | *TBD* | *tip reviewed for capstone sign-off* |

### 2.2 PRs / merges in scope (first-parent narrative)

| # | PR | Branch | Summary |
|---|----|--------|---------|
| *TBD* |  |  |  |

### 2.3 Surface under review

*Shortstat, rough directory distribution, or “full MAS-touched tree” — TBD.*

---

## 3. Review lenses (MAS-9a checklist)

Record **pass / gap / N/A** and evidence (paths, commands, PR links) per row.

### 3.1 IPC

*TBD*

### 3.2 Errors (taxonomy, sandbox-specific surfaces)

*TBD*

### 3.3 Keychain (SKU prefix, coexistence with direct build)

*TBD*

### 3.4 Filesystem (security-scoped bookmarks, stale/rename, symlinks per **MAS-0b**)

*TBD*

### 3.5 OAuth (loopback, parity with direct)

*TBD*

### 3.6 Subprocesses / helper binaries (enumeration + sandbox legality — **MAS-0b** §8 baseline)

*TBD*

### 3.7 Capability deny-list vs **MAS-0b** matrix

*TBD*

### 3.8 CSP / WebView exposure (if in scope for this release)

*TBD*

### 3.9 **`cfg` / `feature = "mas"` inventory** (*Single codebase* exit rule)

Per plan: any user-visible behavioural `#[cfg(feature = "mas")]` / `if mas`
beyond **packaging / entitlements / capability allow-lists / store metadata /
documented UX deltas** needs a **linked removal issue + target** or **blocker
sign-off** here.

| Location | Pattern | Justified? | Removal issue (if any) |
|----------|---------|------------|-------------------------|
| *TBD* |  |  |  |

---

## 4. Findings

| ID | Severity | Lens | Finding | Resolution |
|----|----------|------|---------|------------|
| *—* |  |  | *(none yet)* |  |

---

## 5. MAS-9b / MAS-9c handoff

- **MAS-9b:** Link P0/P1 issues discovered here; confirm none remain for **bar A**.
- **MAS-9c:** Attach dogfood notes, build identifiers, and **Canonical MAS smoke** results (or pointer to appendix).

---

## Appendix A — Optional: command log / screenshots

*Reserve for paste-heavy evidence.*
