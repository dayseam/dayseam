# Phase 5 (MAS) capstone review

**Task:** **MAS-9a** — full review + written artefact ([plan — Block MAS-9](../plan/2026-phase-5-mas-app-store.md#mas-block-9-capstone))  
**Tracking issue:** [#210](https://github.com/dayseam/dayseam/issues/210) (Phase 5 umbrella)  
**Branch:** `DAY-210-mas-9a-lenses-fs` · **PR:** [#245](https://github.com/dayseam/dayseam/pull/245)  
**Semver label:** *(typically `semver:patch` when closing **MAS-9a** with substantive findings; `semver:none` is OK for doc-only scaffolding PRs)*  
**Review date:** *(YYYY-MM-DD when sign-off is recorded)*  
**Release / commit under review:** first-parent **`c9eb8d7`..`7b88204`** (**MAS-1a** [#216](https://github.com/dayseam/dayseam/pull/216) through **MAS-9a** IPC lens [#244](https://github.com/dayseam/dayseam/pull/244); captured 2026-05-02). **[#245](https://github.com/dayseam/dayseam/pull/245)** extends **§3.4** only (desk review prose) — refresh **Head** again after **#245** merges if **`master`** moves.

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

## 1. Canonical MAS smoke (dogfood evidence — copy results into §5, Appendix A, or a dedicated dogfood subsection)

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

**GitHub compare (full diff):** [`8aaab40...7b88204`](https://github.com/dayseam/dayseam/compare/8aaab40...7b88204) — includes **MAS-0b** merge **#214** for context; capstone narrative below starts at **MAS-1a**.

### 2.1 Baseline and head

| | Commit | Label |
|---|--------|-------|
| Baseline (context) | `8aaab40` | [#214](https://github.com/dayseam/dayseam/pull/214) — **MAS-0b** architecture addendum; last first-parent merge before **MAS-1a** |
| In-scope start | `c9eb8d7` | [#216](https://github.com/dayseam/dayseam/pull/216) — **MAS-1a** (first shipped MAS app-code on **`0.13.x`**) |
| Head (capture) | `7b88204` | [#244](https://github.com/dayseam/dayseam/pull/244) — **MAS-9a** §2 refresh + **§3.1 IPC** lens; tip of **`master`** at that merge (**§3.4** prose in [#245](https://github.com/dayseam/dayseam/pull/245)) |

### 2.2 PRs / merges in scope (first-parent, `c9eb8d7^..7b88204`, excluding `chore(release)`)

| # | PR | Merge title |
|---|----|---------------|
| 216 | [#216](https://github.com/dayseam/dayseam/pull/216) | MAS-1a add mas Cargo feature and Tauri merge profile |
| 217 | [#217](https://github.com/dayseam/dayseam/pull/217) | MAS stub entitlements + CI bundle verification |
| 219 | [#219](https://github.com/dayseam/dayseam/pull/219) | **MAS-2a** — App Sandbox + network.client in MAS plist |
| 220 | [#220](https://github.com/dayseam/dayseam/pull/220) | MAS-2b sandbox launch smoke + privacy inventory |
| 221 | [#221](https://github.com/dayseam/dayseam/pull/221) | MAS-2c JIT entitlement compliance doc |
| 222 | [#222](https://github.com/dayseam/dayseam/pull/222) | MAS-3 gate updater off Mac App Store SKU |
| 223 | [#223](https://github.com/dayseam/dayseam/pull/223) | Add security_scoped_bookmarks table |
| 224 | [#224](https://github.com/dayseam/dayseam/pull/224) | Bookmark helpers + RAII guard |
| 225 | [#225](https://github.com/dayseam/dayseam/pull/225) | Local Git bookmark sync + MAS discovery |
| 226 | [#226](https://github.com/dayseam/dayseam/pull/226) | MAS-4d Markdown sink bookmark rows |
| 227 | [#227](https://github.com/dayseam/dayseam/pull/227) | Materialize security-scoped bookmark blobs |
| 228 | [#228](https://github.com/dayseam/dayseam/pull/228) | Stale Local Git bookmark UX (MAS-4f) |
| 229 | [#229](https://github.com/dayseam/dayseam/pull/229) | MAS-5a Keychain sandbox audit (docs) |
| 230 | [#230](https://github.com/dayseam/dayseam/pull/230) | Split MAS-5b into 5b1 and 5b2 (plan + addendum) |
| 231 | [#231](https://github.com/dayseam/dayseam/pull/231) | MAS-5b1 — distinct Application Support for MAS profile |
| 232 | [#232](https://github.com/dayseam/dayseam/pull/232) | MAS-5b2 — Keychain service names for Mac App Store SKU |
| 233 | [#233](https://github.com/dayseam/dayseam/pull/233) | Document MAS outbound HTTPS entitlements |
| 234 | [#234](https://github.com/dayseam/dayseam/pull/234) | Add network.server for OAuth loopback |
| 235 | [#235](https://github.com/dayseam/dayseam/pull/235) | Ship PrivacyInfo.xcprivacy |
| 236 | [#236](https://github.com/dayseam/dayseam/pull/236) | Add MAS export compliance doc |
| 237 | [#237](https://github.com/dayseam/dayseam/pull/237) | Add MAS App Review paste pack |
| 238 | [#238](https://github.com/dayseam/dayseam/pull/238) | MAS bundle CI on tags and schedule |
| 239 | [#239](https://github.com/dayseam/dayseam/pull/239) | MAS-8b local MAS build helper |
| 240 | [#240](https://github.com/dayseam/dayseam/pull/240) | DAY-195 preflight on MAS packaging CI |
| 241 | [#241](https://github.com/dayseam/dayseam/pull/241) | MAS-8d TestFlight upload workflow |
| 242 | [#242](https://github.com/dayseam/dayseam/pull/242) | MAS-9a capstone review scaffold |
| 243 | [#243](https://github.com/dayseam/dayseam/pull/243) | MAS-9a review §2 + cfg inventory |
| 244 | [#244](https://github.com/dayseam/dayseam/pull/244) | MAS-9a §2 refresh + §3.1 IPC lens |

### 2.3 Surface under review

```text
$ git diff --shortstat 8aaab40..7b88204
 64 files changed, 3952 insertions(+), 403 deletions(-)
```

Rough centres: `apps/desktop/src-tauri/` (sandbox, bookmarks, Keychain, IPC, `distribution_profile`), `apps/desktop/src/distribution/` + updater hooks, `docs/compliance/`, `docs/design/2026-phase-5-mas-architecture.md`, `.github/workflows/mas-*.yml`, `scripts/release/mas/`, [`scripts/ci/mas-sandbox-launch-smoke.sh`](../../scripts/ci/mas-sandbox-launch-smoke.sh).

---

## 3. Review lenses (MAS-9a checklist)

Record **pass / gap / N/A** and evidence (paths, commands, PR links) per row.

### 3.1 IPC

- **Status:** **Partial** — desk review of SKU-specific surfaces; full command ↔ capability matrix is **§3.7**.
- **Evidence:** [`apps/desktop/src-tauri/src/ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs)

**Compile-time channel (`MAS-3` / `MAS-3b`):** [`distribution_profile`](../../apps/desktop/src-tauri/src/ipc/commands.rs) is a `#[tauri::command]` (see **`MAS-3b`** comment in-tree) returning `crate::DISTRIBUTION_PROFILE`, set in [`lib.rs`](../../apps/desktop/src-tauri/src/lib.rs) to **`"mas"`** vs **`"direct"`** under `#[cfg(feature = "mas")]` / `#[cfg(not(feature = "mas"))]`. It is listed in the static `COMMANDS` allow-list next to other vetted handlers so the **same** webview asset build can resolve the SKU at runtime via IPC (per **MAS-1a** / **MAS-3** — no separate front-end bundle per channel).

**Security-scoped bookmark IPC (`MAS-4a–f`):** Local Git and Markdown sink flows call `sync_local_git_security_scoped_rows`, `materialize_local_git_bookmarks_from_paths`, `sync_markdown_sink_security_scoped_rows`, and `materialize_markdown_sink_bookmarks_from_paths` under `#[cfg(feature = "mas")]` / `#[cfg(all(feature = "mas", target_os = "macos"))]` inside `sources_add` / `sources_update` / `sinks_add` (same file). Errors map through `DayseamError` with stable internal contexts such as **`security_scoped_bookmarks.*`** (see `map_bookmark_materialize_db_error` and call sites). **MAS-4f** surfaces stale roots via `publish_stale_local_git_bookmark_toast`.

**Gap / follow-up:** No issue opened — confirm on a future pass that every **MAS-only** code path behind `cfg` is either unreachable from the direct build (true today) or mirrored in Vitest/IPC parity tests where behaviour differs only by profile.

### 3.2 Errors (taxonomy, sandbox-specific surfaces)

*TBD*

### 3.3 Keychain (SKU prefix, coexistence with direct build)

*TBD*

### 3.4 Filesystem (security-scoped bookmarks, stale/rename, symlinks per **MAS-0b**)

- **Status:** **Partial** — desk review of bookmark persistence + runtime helpers; symlink “escape hatch” behaviour is **not** audited line-by-line in this lens (policy in **MAS-0b** §9.4; exercise via **Canonical MAS smoke** **§1**, item 3 — Local Git scan on nested repo layout).
- **Evidence:** [`security_scoped/mod.rs`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) (**MAS-4b** — `SecurityScopedGuard` / `from_bookmark`, `ResolvedBookmark::is_stale`, `create_directory_bookmark`, non-macOS stubs), [`security_scoped_bookmarks.rs`](../../crates/dayseam-db/src/repos/security_scoped_bookmarks.rs) + [`0007_security_scoped_bookmarks.sql`](../../crates/dayseam-db/migrations/0007_security_scoped_bookmarks.sql) (**MAS-4a**), [`ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) sync/materialize + **MAS-4f** stale-root toasts (cross-ref **§3.1**), [`local_git_scan.rs`](../../apps/desktop/src-tauri/src/local_git_scan.rs) (**MAS-4c** discovery).

**Symlink / rename policy:** [**MAS-0b**](../design/2026-phase-5-mas-architecture.md#94-symlinks) (architecture **§9.4 Symlinks**) documents canonicalization on persist, scan-root containment, and `meta_json` for bookmark rows. This lens assumes implementation tracks that doc; **Gap / follow-up:** file an issue if dogfood (**MAS-9c**) finds a divergence.

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

Mechanical sweeps (repo root; requires [ripgrep](https://github.com/BurntSushi/ripgrep)):

```bash
rg 'feature\s*=\s*"mas"|cfg\(.*feature\s*=\s*"mas"|cfg\(not\(feature\s*=\s*"mas"\)\)|cfg\(all\(feature\s*=\s*"mas"' apps/desktop/src-tauri -g'*.rs'
rg 'distribution_profile|"mas"' apps/desktop/src/distribution apps/desktop/src/features/updater -g'*.{ts,tsx}'
```

The Rust pattern is intentionally prefix-oriented (it matches `#[cfg(all(feature = "mas", …))]` without enumerating every `cfg` combinator). Re-run after meaningful MAS edits.

| Location | What diverges | Justified per plan bucket? | Removal issue (if any) |
|----------|----------------|-----------------------------|-------------------------|
| [`apps/desktop/src-tauri/Cargo.toml`](../../apps/desktop/src-tauri/Cargo.toml) | `[features] mas = []` gate + docs | Yes — **feature / packaging** | — |
| [`apps/desktop/src-tauri/tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) | Bundle id + `entitlements.mas.plist` | Yes — **packaging / entitlements** | — |
| [`apps/desktop/package.json`](../../apps/desktop/package.json) | `tauri:build:mas` script | Yes — **packaging** | — |
| [`apps/desktop/src-tauri/src/startup.rs`](../../apps/desktop/src-tauri/src/startup.rs) | `DATA_SUBDIR` / path roots under `#[cfg(feature = "mas")]` | Yes — **MAS-5b1** coexistence / **MAS-0b** §10 | — |
| [`apps/desktop/src-tauri/src/lib.rs`](../../apps/desktop/src-tauri/src/lib.rs) | Tauri builder + capability split | Yes — **packaging / capabilities** | — |
| [`apps/desktop/src-tauri/src/keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) | Keychain service / account strings | Yes — **MAS-5b2** | — |
| [`apps/desktop/src-tauri/src/main.rs`](../../apps/desktop/src-tauri/src/main.rs) | Updater / menu / single-instance registration | Yes — **MAS-3** updater removal | — |
| [`apps/desktop/src-tauri/src/local_git_scan.rs`](../../apps/desktop/src-tauri/src/local_git_scan.rs) | Default scan roots vs security-scoped MAS discovery | Yes — **MAS-4c** filesystem contract | — |
| [`apps/desktop/src-tauri/src/ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) | Bookmarks, `distribution_profile`, folder pickers, `#[cfg(all(feature = "mas", target_os = "macos"))]` branches | Yes — **IPC + FS** tasks **MAS-4a–f**; **§3.1 / §3.4** must still sign off pass vs gap | — |
| [`apps/desktop/src/distribution/DistributionProfileProvider.tsx`](../../apps/desktop/src/distribution/DistributionProfileProvider.tsx) | `invoke("distribution_profile")` → `"mas"` \| `"direct"` | Yes — **MAS-3** documented UX delta (feeds `useUpdater` gate) | — |
| [`apps/desktop/src/distribution/distributionProfileContext.ts`](../../apps/desktop/src/distribution/distributionProfileContext.ts) | `DistributionProfileLoaded` union | Yes — typed **store metadata** surface | — |

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
