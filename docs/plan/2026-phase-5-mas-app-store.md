# Phase 5 (MAS): Mac App Store distribution — Implementation plan

> **Status:** **MAS-0** complete — plan index (**MAS-0a**) + normative addendum (**MAS-0b**: [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md)). **Target:** complete Phase 5 engineering on the **`0.13.x`** line; if Apple or CI blockers force overflow, document remaining work under **`MAS-*-followup`** and the next patch line (**`0.14.x`**) only by explicit decision — do not silently drift.  
> **Tracking issue:** [#210](https://github.com/dayseam/dayseam/issues/210).  
> **Canonical architecture:** this phase extends [`ARCHITECTURE.md`](../../ARCHITECTURE.md); sandbox-specific decisions live in the Phase 5 addendum (**MAS-0b** file above).  
> **Agent execution:** follow [`AGENTS.md`](../../AGENTS.md) — issue → branch `DAY-NNN` from `master`, one commit per branch, no agent-merge of PRs.

## Executive summary

Phase 5 adds a **Mac App Store–eligible build flavor** of Dayseam alongside the existing **direct-download** (Developer ID / notarized DMG) distribution. The current production shell **explicitly avoids App Sandbox** ([`apps/desktop/src-tauri/entitlements.md`](../../apps/desktop/src-tauri/entitlements.md)); MAS requires a **sandboxed** entitlement profile and a **different update story** (no in-app Tauri updater). This phase implements that second SKU without regressing the existing SKU.

**Non-goals for Phase 5 (unless explicitly pulled into scope later):**

- App Store Connect listing assets, screenshots, pricing, or **first-time** provisioning profile / identifier setup **outside automation** (operators still create App IDs and store secrets in CI; this plan does not document Apple’s web UI click-by-click).
- Windows Microsoft Store (future phase).
- Replacing direct-download distribution.

### Phase 5 exit criteria (two explicit bars)

Stakeholders should not conflate **“submission-ready”** with **“release automation complete.”**

| Bar | Definition | **MAS-8d required?** |
|-----|------------|----------------------|
| **A. Engineering complete** | Sandboxed MAS build is **store-compatible**: real shell launches, bookmarks + Keychain + OAuth + connectors + sinks validated under sandbox; **manual** upload to App Store Connect succeeds; review doc (**MAS-9a**) and dogfood (**MAS-9c**) done; P0/P1 bugs fixed (**MAS-9b**). | **No.** Manual upload is acceptable. **MAS-9a** must state whether upload was manual or automated. |
| **B. Release automation complete** | **MAS-8d** (or successor) wires **automated** Connect upload to the same cadence as GitHub Releases; export compliance docs (**MAS-7b**) align with what the upload job asserts. | **Yes.** |

**Phase 5 is officially “closed” in planning terms when bar A is met** and any gap to bar B is either **MAS-8d merged** or **tracked as `MAS-8d-followup`** with an owner — not silently dropped.

### Long-term product: one codebase, two distribution channels

**Initial approach (early `0.13.x` tasks):** Cargo / Tauri **feature flags** (e.g. `mas`) and separate entitlements plists are acceptable **scaffolding** to land sandbox + CI without blocking `master`.

**End state (explicit phase goal):** the team should **not** permanently maintain two behavioural code paths behind `#[cfg(feature = "mas")]` / duplicated UI logic. Aim to converge on **one runtime codebase** that works for **both** Mac App Store and direct (GitHub) download.

**Separate concerns** (do not let “Apple required it” hide runtime branching):

| Layer | Examples (allowed long-term divergence) |
|-------|----------------------------------------|
| **Packaging** | Bundle ID, signing identity, DMG vs `.pkg` / store export, `createUpdaterArtifacts` on/off. |
| **Entitlements** | `entitlements.plist` vs `entitlements.mas.plist` keys and values. |
| **Capability allow-lists** | MAS capability JSON omits updater + any deny-listed permissions (full matrix in **MAS-0b**). |
| **Store metadata** | `PrivacyInfo.xcprivacy`, App Review notes, export compliance prose. |
| **UX deltas** | “Check for updates” absent on MAS only if **compile-time** or single `distribution_profile` enum — not scattered `if (mas)` in business logic. |

**Any user-visible behavioural `#[cfg(feature = "mas")]` / `if mas` beyond the table above** must either (1) land with a **linked removal issue** and target date, or (2) **block MAS-9a completion** until justified in **`docs/review/phase-5-mas-review.md`** with reviewer sign-off.

Runtime behaviour (bookmarks, scoped file access, Keychain, connectors) should be **unified** where feasible: the direct build may adopt the same security-scoped access patterns on macOS to reduce drift. Document the convergence plan in **MAS-0b**; **MAS-9a** includes a **flag / cfg inventory** with the rule above.

---

## Versioning contract (`0.13.x`) — **locked rules**

These rules are **normative in this document** (not “pick one in the PR body”):

| Rule | Detail |
|------|--------|
| **Docs kickoff** | **MAS-0a / MAS-0b** (plan + architecture stub only): PR label **`semver:none`**. **No** `v0.13.0` tag from docs-only merges. The version ladder below starts with **the first application-code merge** on the MAS track. |
| **Entering `0.13.x`** | The **first merged PR** that changes **shipped application code** for Phase 5 (typically **MAS-1a**) MUST use **`semver:minor`** so the release workflow cuts **`v0.13.0`** from the current **`0.12.x`** line. (Patch-only bumps cannot move `0.12.z` → `0.13.0`.) |
| **Subsequent tasks** | **MAS-1b** onward through the capstone: each merge that ships user-visible or packager-visible MAS work uses **`semver:patch`** → **`v0.13.1`**, **`v0.13.2`**, … in task order unless tasks are **batched** into one PR (preferred when several steps are tiny). |
| **Inserted tasks** | Renumber **target versions** from the insertion point; do not pretend older table rows stay valid after the catalogue changes. |
| **Overflow** | **Target** completion in **`0.13.x`**. If external blockers (Apple review, secrets, runner capacity) prevent closure, record **`MAS-*-followup`** and either continue **`0.13.(N+1)`** or escalate minor line by **team decision** — not by accidental drift. |
| **Capstone** | **MAS-9a–c** land at **`v0.13.25`–`v0.13.27`** in the table below (recalculated after splitting **MAS-5b** into **MAS-5b1** / **MAS-5b2**). |

**Branch / ticket naming**

- **Task IDs:** `MAS-0a`, `MAS-0b`, `MAS-1a`, … Split delivery steps may use suffixes (e.g. **`MAS-5b1`**, **`MAS-5b2`**) — GitHub issue title: `MAS-5b1: <short title>` (same **`MAS-*`** prefix for traceability).
- **Branches:** **`DAY-NNN-*`** per [`AGENTS.md`](../../AGENTS.md); keep **MAS-*** in the issue title for traceability.

---

## Direct vs MAS: coexistence, migration, and data (outputs of **MAS-0b**)

The plan **requires** **`docs/design/2026-phase-5-mas-architecture.md`** (after **MAS-0b**) to answer — not “if applicable”:

| Topic | Must document |
|-------|----------------|
| **Co-installation** | May both apps be installed at once? If yes: distinct bundle IDs, distinct **Application Support** / container paths, and **Keychain service naming** must not trample. If no: installer / docs must say so. |
| **SQLite / state DB** | Same file, two copies, or migration tool? Lock semantics if both run. |
| **Keychain** | Same or distinct access groups / service identifiers per SKU. |
| **OAuth / URL schemes** | Collisions between direct and MAS builds on one machine. |
| **Migration direct → MAS** | Which config rows survive; which absolute paths break until re-authorized; whether updater prefs are ignored; re-consent flows. |
| **Access lifetime** | **Start/stop** security-scoped access: **RAII guard** (or `defer`-style) wrapping each filesystem operation batch; **no** session-wide blanket start unless justified in the addendum. Long-running sync jobs = one guard spanning the **job lifecycle** only. |

---

## Security-scoped bookmarks: design contract (**MAS-0b** + **MAS-4** block)

**MAS-0b** must specify (implementation follows in **MAS-4a** through **MAS-4f**):

| Topic | Requirement |
|-------|-------------|
| **Granularity** | Directory vs file bookmark for **scan roots** vs **sink folders**; whether saving a **new file** in an existing folder reuses the parent bookmark. |
| **Descendants** | After cold start, do nested repos under a bookmarked root always resolve? Under what macOS constraints? |
| **Rename / move** | Stale bookmark detection; **error taxonomy** (`DayseamError` / IPC codes); **re-prompt UX** (“Reselect folder in Settings”). |
| **Symlinks** | Allowed, canonicalized to realpath before persist, or **explicitly rejected** with user-facing copy. |
| **MAS-4f** | Implements stale-bookmark detection + recovery UX + tests (see task table). |

---

## Dual-channel release goal (GitHub → App Store updates)

**Reality:** Apple does **not** pull new binaries from GitHub Releases. App Store updates flow through **App Store Connect** after upload → processing → review/release.

**Target:** the same **semver** on `master` drives **both** direct (GitHub Release + updater) and MAS (upload to Connect). **Bar B** (above) makes automation explicit.

**Failure isolation:** Prefer **`continue-on-error`** (or a non-blocking child workflow) for **MAS-8d** so a failed Connect upload **does not** fail the **direct** `release.yml` job unless the team explicitly chooses strict coupling.

**Version skew:** App Store review lag means **direct** users may run **`v0.13.N`** while **MAS** users remain on **`v0.13.(N−k)`**. Document in **MAS-0b**: persisted data **backward-compatible** across at least **K** patch levels; migrations must not strand older MAS builds; support/docs may list **channel + version**.

**Rollback / bad MAS build:** **MAS-9** / **MAS-8** docs must cover phased release on Connect, **manual hold** before “Release,” and emergency hotfix path (direct channel may ship a patch ahead of MAS during an incident — call out support implications).

**Export compliance (MAS-7b)** must state assumptions that **MAS-8d** upload metadata will reuse (aligned answers in App Store Connect).

Implementation: **MAS-8d** in [Block MAS-8](#mas-block-8-ci-release).

---

<a id="canonical-mas-smoke-checklist"></a>

## Canonical MAS smoke checklist (reuse every manual / dogfood pass)

Copy into PRs or **`docs/review/phase-5-mas-review.md`** as evidence:

1. Cold **launch** (MAS build).  
2. **Open folder** picker; grant path; **quit and relaunch** — access still works for scan/sink.  
3. **Local Git** scan on nested repo layout (per **MAS-0b** symlink policy).  
4. **Save** report to Markdown sink in permitted folder.  
5. **Reconnect** a secret-backed source (rotate or re-validate).  
6. **OAuth** complete flow (e.g. Outlook).  
7. **No** updater UI or updater network calls.  
8. **Upgrade** from previous MAS build (if applicable).  

---

## Testing discipline (applies to every task)

1. **Existing tests:** Do not change assertions or snapshots **unless** the task intentionally changes behaviour (document why in the PR). Prefer **adding** focused tests over rewriting historical ones.
2. **Required verification** (adapt per task — minimal subset for docs-only tasks):

   ```text
   cargo fmt --all -- --check
   cargo clippy --workspace --all-targets -- -D warnings
   cargo test --workspace
   pnpm -r lint && pnpm -r typecheck && pnpm -r test   # as applicable
   ```

   For large workspaces, use the **relevant subset** in [`AGENTS.md`](../../AGENTS.md).

   Desktop: `pnpm exec vitest run` in `apps/desktop`; keep **the full desktop Vitest suite** green.

3. **New tests:** Each **code** task adds coverage for new paths. No task merges with “TODO tests.”
4. **Unit and integration tests first; E2E sparingly:** Prefer **`cargo test`**, integration tests, **Vitest** + mocks, **wiremock**, fixture DBs. **Playwright** only for **thin** smoke where unavoidable.

---

## Codesign & entitlement verification (CI + self-review)

**MAS-1b** and later MAS packaging tasks must go beyond **“plist parses”**:

| Check | Minimum |
|-------|---------|
| **Artifact** | **`cargo tauri build`** (or scripted equivalent) produces a **`.app`** for **both** default and **`mas`** profiles where applicable. |
| **Entitlements** | `codesign -d --entitlements :- --xml <App>` shows **expected** keys; MAS build includes **`com.apple.security.app-sandbox`** when sandbox is enabled. |
| **Forbidden on MAS** | Grep / `nm` (or documented equivalent) confirms **no** updater symbols / endpoint baked into MAS flavour where intended absent. |
| **Direct build** | Still meets **hardened runtime** expectations documented in [`docs/release/CODESIGN.md`](../release/CODESIGN.md). |

---

## Self-review checklist (end of every task, before PR)

- [ ] **Repo-wide regression:** verification commands green.  
- [ ] **SKU parity:** default direct build unchanged unless task is MAS-only (then manual smoke + **Canonical MAS smoke** subset).  
- [ ] **`CHANGELOG.md`:** per semver policy.  
- [ ] **`ARCHITECTURE.md`** or Phase 5 addendum updated if boundaries moved.  
- [ ] **Capabilities:** MAS bundle matches **MAS-0b** matrix; no silent broadening.  
- [ ] **Security:** no secrets in repo.  
- [ ] **Codesign row:** for packaging tasks, **Codesign & entitlement verification** section satisfied.  

---

## Task catalogue

**Target versions** below assume **MAS-1a** = **`v0.13.0`** (`semver:minor`), then **`semver:patch`** per row. **MAS-0a/b** = **`semver:none`**.

<a id="mas-block-0"></a>

### Block MAS-0 — Planning & architecture (`semver:none`)

| ID | Task | Target version | Deliverables |
|----|------|----------------|--------------|
| **MAS-0a** | Plan index + **locked** semver rules (this doc) | **Kickoff** | [`docs/plan/README.md`](./README.md) Phase 5 row; rules in **Versioning contract** above. |
| **MAS-0b** | Architecture addendum | **Kickoff (follows 0a)** | **Expand** [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md): dual-SKU diagram; **entitlement + capability matrices** (default vs MAS, deny-list); bookmark contract (**granularity**, **stale/rename**, **symlinks**, **start/stop lifetime**); Keychain + OAuth + **coexistence/migration**; **JIT** evidence (exact keys, Tauri/WebKit citations, **fallback** if App Review rejects); **privacy/SDK inventory** output for **MAS-7a**; **version skew + rollback**; **subprocess / helper binary** enumeration baseline for **MAS-9a**. |

**Self-review (MAS-0):** links valid; no contradiction with [`entitlements.md`](../../apps/desktop/src-tauri/entitlements.md).

---

### Block MAS-1 — Build matrix + **macOS CI prerequisite**

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-1a** | **`mas` feature** + Tauri profile flag; **default artefact unchanged** | **`v0.13.0`** (`semver:minor`) | `Cargo.toml` / `build.rs` / config; **MAS-0b** feature matrix referenced. |
| **MAS-1b** | **`entitlements.mas.plist` stub** + **macOS GHA** job | **`v0.13.1`** | Plist wired by feature; **macOS runner** runs **`cargo tauri build`** (or `build --features mas`) so both profiles **package**; **Codesign & entitlement verification** minimums; not merely `plutil -lint`. |
| **MAS-1c** | *(Optional merge into 1b if tiny)* Reserve **self-hosted macOS** or longer timeout if GitHub-hosted is insufficient | **`v0.13.1`** | Document in **MAS-0b** / CI comments if skipped. |

**Self-review:** checksum / behaviour vs pre-change baseline for **default** build.

---

### Block MAS-2 — Sandbox foundation + **real shell** + **early privacy inventory**

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-2a** | **`com.apple.security.app-sandbox`** in **MAS plist only** + `network.client` as needed | **`v0.13.2`** | MAS plist only. |
| **MAS-2b** | **Sandboxed smoke: real Dayseam shell** | **`v0.13.3`** | **Forbidden:** noop / fake stub app. **Required:** production **`main`** window (or feature-flagged minimal shell **using same binary**); WebView loads; **no** full connector QA yet. **Deliverable:** **Privacy + embedded SDK inventory** (markdown table: SDK → existing manifest? → gap for **MAS-7a**). |
| **MAS-2c** | **JIT / executable-memory** entitlements | **`v0.13.4`** | **Exact entitlement keys**; per-OS/arch notes; **Apple-facing justification** text for **MAS-7c**; **fallback plan** (e.g. escalate to Tauri upstream / reduce WebView features) if review rejects. |

**Self-review:** direct SKU untouched.

---

### Block MAS-3 — Updater removal + **full capability audit**

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-3a** | Gate **updater** + **`capabilities/updater.json`** off MAS bundle | **`v0.13.5`** | **Full MAS capability matrix** implemented per **MAS-0b** (not updater-only). |
| **MAS-3b** | Hide updater **UI** on MAS | **`v0.13.6`** | Vitest; minimal `cfg` / compile-time injection. |

---

### Block MAS-4 — Bookmarks + **stale access**

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-4a** | Bookmark **blob** + persistence design | **`v0.13.7`** | Implements **MAS-0b** §bookmarks; additive migrations only. |
| **MAS-4b** | Rust helper: create / resolve / **start** / **stop** (RAII) | **`v0.13.8`** | **macOS GHA** tests; Linux compile-only stubs. |
| **MAS-4c** | Local Git scan roots (**transitional** `mas` if needed) | **`v0.13.9`** | Parallel tests; converge with direct per *Single codebase*. |
| **MAS-4d** | Sink paths | **`v0.13.10`** | Same. |
| **MAS-4e** | `dialog.open` persist + rehydrate | **`v0.13.11`** | RTL/IPc where feasible. |
| **MAS-4f** | **Stale bookmark** detection + **error codes** + **re-prompt UX** + logging | **`v0.13.12`** | User-visible recovery; tests. |

---

### Block MAS-5 — Keychain + **`DATA_SUBDIR`** (**may gate OAuth — see graph**)

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-5a** | Keychain audit under sandbox | **`v0.13.13`** | Document in addendum; mock tests unchanged. |
| **MAS-5b1** | **`DATA_SUBDIR` / Application Support** for MAS profile (`startup.rs`, §10) | **`v0.13.14`** | Distinct `state.db` namespace vs direct when both SKUs installed; tests. |
| **MAS-5b2** | Keychain **`service` SKU prefix** + remaining fixes from **MAS-2b** / **MAS-4** + **MAS-5a** follow-ups | **`v0.13.15`** | Regression tests; optional migration / reconnect story for existing MAS installs. |

---

### Block MAS-6 — Networking & OAuth

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-6a** | Network entitlements + smoke | **`v0.13.16`** | Document HTTPS / self-hosted connector pattern (**done:** [`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md) Outbound HTTPS, architecture §13; CI: [`verify-tauri-bundle-entitlements.sh`](../../scripts/ci/verify-tauri-bundle-entitlements.sh)). |
| **MAS-6b** | **OAuth loopback** + **rate-limit / retry parity** vs direct | **`v0.13.17`** | **`network.server`** entitlement for sandbox **bind/accept** + docs + CI verify ([`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md) OAuth loopback, architecture §14). **Rate-limit / retry:** same **`reqwest`** / connector-sdk paths on **`mas`** as direct—documented; add regression test only if a sandbox-only divergence is found. |

---

### Block MAS-7 — Compliance artefacts

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-7a** | Wire **`PrivacyInfo.xcprivacy`** | **`v0.13.18`** | App-level manifest + **`bundle.macOS.files`** (**done:** [`PrivacyInfo.xcprivacy`](../../apps/desktop/src-tauri/PrivacyInfo.xcprivacy), [`verify-bundle-privacy-manifest.sh`](../../scripts/ci/verify-bundle-privacy-manifest.sh), architecture §16). |
| **MAS-7b** | **`MAS-EXPORT-COMPLIANCE.md`** | **`v0.13.19`** | **Done:** [`MAS-EXPORT-COMPLIANCE.md`](../compliance/MAS-EXPORT-COMPLIANCE.md) (crypto inventory, Connect answers, **MAS-8d** checklist); architecture §22 + §20. |
| **MAS-7c** | **`MAS-APP-REVIEW-NOTES.md`** | **`v0.13.20`** | **Done:** [`MAS-APP-REVIEW-NOTES.md`](../compliance/MAS-APP-REVIEW-NOTES.md) (local-first, sandbox + network + **MAS-3**, JIT blockquote + **MAS-JIT** ladder pointer, **§8** subprocess baseline, privacy + export links); architecture §23 + §20; **MAS-JIT** maintenance. |

---

<a id="mas-block-8-ci-release"></a>

### Block MAS-8 — CI & release

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-8a** | GHA: **direct + MAS** bundles on tag / schedule | **`v0.13.21`** | **Done:** [`mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml) — same `tauri build --bundles app` pair as **`ci.yml`** `desktop-bundle-profiles`, [`verify-tauri-bundle-entitlements.sh`](../../scripts/ci/verify-tauri-bundle-entitlements.sh) + [`verify-bundle-privacy-manifest.sh`](../../scripts/ci/verify-bundle-privacy-manifest.sh), [`mas-sandbox-launch-smoke.sh`](../../scripts/ci/mas-sandbox-launch-smoke.sh); triggers: `push` **`v*`** tags, `workflow_dispatch`, weekly `schedule`; `bash -n` on the three verify/smoke scripts + `plutil -lint` on source **`PrivacyInfo.xcprivacy`**. |
| **MAS-8b** | **`scripts/release/`** MAS helper | **`v0.13.22`** | **Done:** [`scripts/release/mas/README.md`](../../scripts/release/mas/README.md) + [`build-mas-app.sh`](../../scripts/release/mas/build-mas-app.sh) — local MAS `Dayseam.app` + CI-parity verify/smoke; **`bash -n`** in [`ci.yml`](../../.github/workflows/ci.yml) + [`mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml); **replace or delete when MAS-8d**; architecture §21. |
| **MAS-8c** | Changelog / **DAY-195** gates for MAS releases | **`v0.13.23`** | **Done:** [`mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml) — **`fetch-depth: 0`** checkout, **`bash -n`** + mock **`semver:patch`** [`check-unreleased-for-semver-pr.sh`](../../scripts/release/check-unreleased-for-semver-pr.sh) (same preflight as [`ci.yml`](../../.github/workflows/ci.yml) **`changelog-semver-pr`**); **`shell-scripts`** **`bash -n`** on PRs; architecture §21. |
| **MAS-8d** | **Automated Connect upload** | **`v0.13.24`** | **Done:** [`mas-connect-upload.yml`](../../.github/workflows/mas-connect-upload.yml) — **`workflow_dispatch`**, **`continue-on-error`**, **TestFlight** via **Transporter** + **`upload-testflight-build`** pinned to **v4.1.0** (**`994cd4f`**, `app-type: macos`, **`DAYSEAM_ASC_*`** secrets, **`uses-non-exempt-encryption: false`**); workspace-relative **`.pkg`** + secret gate before upload; [`MAS-CONNECT-UPLOAD.md`](../../docs/release/MAS-CONNECT-UPLOAD.md) + [`mas-connect-upload-preflight.sh`](../../scripts/release/mas/mas-connect-upload-preflight.sh). **Non-blocking** vs [`release.yml`](../../.github/workflows/release.yml). **`.pkg` CI / artefact wiring** → **`MAS-8d-followup`**. |

---

<a id="mas-block-9-capstone"></a>

### Block MAS-9 — Capstone

| ID | Task | Target version | Sub-tasks |
|----|------|----------------|-----------|
| **MAS-9a** | Full review + **`docs/review/phase-5-mas-review.md`** | **`v0.13.25`** | Lenses: IPC, errors, Keychain, FS, OAuth, **subprocess/helper binaries**, **capability deny-list**, CSP, **cfg inventory** per *Single codebase* exit rule. State **bar A** / **bar B** explicitly. |
| **MAS-9b** | Bugfix sweep | **`v0.13.26`** | No P0/P1 for **bar A**. |
| **MAS-9c** | Dogfood using **Canonical MAS smoke** | **`v0.13.27`** | Evidence in review doc. |

**Self-review (capstone):** **`cargo test --workspace`** + desktop **`pnpm test`**; E2E only with documented gap; **ARCHITECTURE.md** Phase 5 blurb; flag inventory closed or filed.

---

## Dependency graph (high level)

```text
MAS-0a/b (coexistence, bookmarks, capabilities, JIT evidence — blocks implementation)
    │
    ▼
MAS-1a ──► MAS-1b (macOS CI + package proof)
    │
    ▼
MAS-2a ──► MAS-2b (real shell + privacy inventory) ──► MAS-2c (JIT)
    │
    ├──► MAS-7a (privacy manifest)  [feeds from 2b inventory]
    ▼
MAS-3a/b
    │
    ▼
MAS-4a ──► … ──► MAS-4f
    │
    ├──► MAS-5b1 ──► MAS-5b2 (Keychain / paths — may unblock / inform MAS-6b OAuth)
    ▼
MAS-6a ──► MAS-6b
    │
    ▼
MAS-7b ──► MAS-7c ──► MAS-8a ──► MAS-8d (optional for bar A)
    │
    ▼
MAS-9a ──► MAS-9c
```

**Parallelism:** **MAS-7a** can start after **MAS-2b** inventory exists; **MAS-4a–f** remains critical path for FS.

---

## Risk register (living)

| Risk | Mitigation |
|------|------------|
| Sandbox vs libgit2 | Bookmarks + RAII start/stop (**MAS-0b**, **MAS-4**). |
| JIT rejection | Evidence pack + fallback (**MAS-2c**, **MAS-7c**). |
| Dual maintenance | Packaging-only deltas + **MAS-9a** cfg gate. |
| CI flake / Linux blind | **MAS-1b** macOS job; stubs labelled **non-authoritative**. |
| **MAS-8b dead scaffold** | Explicit **remove/replace** when the **MAS-8d** upload pipeline **subsumes** the local helper (`.pkg` / signing in CI — **`MAS-8d-followup`**). |
| **Review discovers late helper binary** | **MAS-9a** subprocess enumeration + sandbox legality. |

---

## Document history

| Date | Change |
|------|--------|
| 2026-04-30 | Initial Phase 5 plan (`v0.13.0` kickoff). |
| 2026-04-30 | Testing / flags / dual-channel / review fixes / #210. |
| 2026-04-30 | **Hardening pass:** locked semver (**minor** → `v0.13.0`); two exit bars; coexistence + migration + bookmark contract; **MAS-4f**; **MAS-6c** merged into **MAS-6b**; real-shell **MAS-2b**; early privacy inventory; codesign CI gates; **MAS-1c** optional; capability full audit; skew/rollback; **MAS-8b** disposable note; graph dependencies; **MAS-0b** expanded obligations. |
| 2026-04-30 | **MAS-0b:** full architecture addendum merged (matrices, bookmarks, coexistence, subprocess baseline, skew, open decisions). |
| 2026-04-30 | **MAS-1a:** Cargo `mas` feature + `tauri.mas.conf.json` merge profile + `tauri:build:mas`; `DISTRIBUTION_PROFILE` in desktop crate. |
| 2026-04-30 | **MAS-1b:** `entitlements.mas.plist` stub + `verify-tauri-bundle-entitlements.sh` + `desktop-bundle (direct + MAS)` CI; `check-entitlements.sh` for MAS plist on macOS matrix leg. |
| 2026-04-30 | **MAS-1c:** skipped — GitHub-hosted `macos-latest` + `desktop-bundle` timeout proved sufficient; no self-hosted runner (documented here per optional row). |
| 2026-04-30 | **MAS-2a:** `com.apple.security.app-sandbox` + `com.apple.security.network.client` in `entitlements.mas.plist` only; CI gate requires embedded keys on MAS bundle. |
| 2026-04-30 | **MAS-2b:** `mas-sandbox-launch-smoke.sh` in `desktop-bundle` MAS leg; §16 inventory in architecture addendum. |
| 2026-05-01 | **MAS-2c:** `docs/compliance/MAS-JIT-ENTITLEMENTS.md` + architecture §5/§7/§21 pointers (JIT keys unchanged). |
| 2026-05-01 | **MAS-3a/b:** Updater + process plugins not registered on `--features mas`; MAS Tauri merge drops updater capability + plugin section; **`distribution_profile`** IPC + frontend gate + Vitest; architecture §15/§16/§21 updated. |
| 2026-05-01 | **MAS-4a:** SQLite **`security_scoped_bookmarks`** table + architecture §9.6; **`dayseam-db` `build.rs`** migration rerun hints; integration tests for FK / `CHECK` / cascade. |
| 2026-05-01 | **MAS-4b:** Desktop **`security_scoped`** module (`objc2-foundation`): bookmark create/resolve + RAII guard; macOS lib tests; Linux stubs. |
| 2026-05-01 | **MAS-4c:** `SecurityScopedBookmarkRepo` + Local Git `sources_add`/`sources_update` sync; **`local_git_scan`** MAS macOS discovery; DB integration test. |
| 2026-05-01 | **MAS-4d:** `sync_markdown_sink_dest_dirs` + **`sinks_add`** MAS sync; DB integration test **`security_scoped_bookmarks_sync_markdown_sink_dest_dirs`**. |
| 2026-05-01 | **MAS-4e:** `create_directory_bookmark` + **`set_*_bookmark_blob`** wired into **`sources_add`** / **`sources_update`** / **`sinks_add`** on **macOS + `mas`**; DB test **`security_scoped_bookmarks_set_blob_updates_rows`**; error codes **`ipc.security_scoped_bookmark.*`**. |
| 2026-05-01 | **MAS-5a:** Keychain audit under sandbox documented in **MAS-0b** §12 (+ §20 checklist); mock tests unchanged per task row. |
| 2026-05-01 | Split **MAS-5b** into **MAS-5b1** (Application Support / `DATA_SUBDIR`) + **MAS-5b2** (Keychain prefix + regression tests); renumbered target **`v0.13.14`**–**`v0.13.27`** from **MAS-6** onward; capstone **`v0.13.25`–`v0.13.27`**. |
| 2026-05-01 | **MAS-7b:** [`MAS-EXPORT-COMPLIANCE.md`](../compliance/MAS-EXPORT-COMPLIANCE.md) + architecture §22; **MAS-8d** linkage; MR review tightened ENC/WebKit/minisign wording + Apple doc link; **MAS-JIT** cross-link. |
| 2026-05-01 | **MAS-7c:** [`MAS-APP-REVIEW-NOTES.md`](../compliance/MAS-APP-REVIEW-NOTES.md) + architecture §23 + **MAS-JIT** / **MAS-EXPORT** cross-links. |
| 2026-05-01 | **MAS-8a:** [`mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml) + **`ci.yml`** cross-reference comment; architecture §21 / §20; MR review: job rename, `permissions`, extra **`bash -n`**, plan row (**direct + MAS**). |
| 2026-05-01 | **MAS-8b:** [`scripts/release/mas/`](../../scripts/release/mas/) README + **`build-mas-app.sh`**; §21 local helper; **MAS-8d** replace/delete contract. |
| 2026-05-02 | **MAS-8c:** [`mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml) DAY-195 mock **`check-unreleased-for-semver-pr.sh`** + full git checkout; plan row; architecture §21 / §20. MR review: **`bash -n`** + **`ci.yml`** **`shell-scripts`**, doc wording. |
| 2026-05-02 | **MAS-8d:** [`mas-connect-upload.yml`](../../.github/workflows/mas-connect-upload.yml) + [`MAS-CONNECT-UPLOAD.md`](../../docs/release/MAS-CONNECT-UPLOAD.md) + preflight script; plan row + risk register; architecture §21 / §20; **MAS-EXPORT** checklist. MR review: runbook **`mas-package-verify.yml`** link; **`.pkg`** workspace gate + ASC secrets; action pin **994cd4f**; Transporter tmp cleanup. |
