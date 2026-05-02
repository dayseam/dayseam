# Phase 5 (MAS) capstone review

**Task:** **MAS-9a** — full review + written artefact ([plan — Block MAS-9](../plan/2026-phase-5-mas-app-store.md#mas-block-9-capstone))  
**Tracking issue:** [#210](https://github.com/dayseam/dayseam/issues/210) (Phase 5 umbrella)  
**Branch:** `DAY-210-mas-9a-lenses-bars-39` · **PR:** [#252](https://github.com/dayseam/dayseam/pull/252)  
**Semver label:** *(typically `semver:patch` when closing **MAS-9a** with substantive findings; `semver:none` is OK for doc-only scaffolding PRs)*  
**Review date:** *(YYYY-MM-DD when sign-off is recorded)*  
**Release / commit under review:** first-parent **`c9eb8d7`..`8cb1362`** (**MAS-1a** [#216](https://github.com/dayseam/dayseam/pull/216) through **MAS-9a** §3.8 lens [#251](https://github.com/dayseam/dayseam/pull/251); captured 2026-05-09). **[#252](https://github.com/dayseam/dayseam/pull/252)** extends **§2** (post-#251 tip) + **§0 exit bars** + **§3.9** inventory sign-off + **§5** handoff notes — after **#252** lands on **`master`**, bump **§2** (**Head**, compare, shortstat, **§2.2** row **#252**) through the **#252** merge commit.

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
| **A. Engineering complete** | Sandboxed MAS build is **store-compatible**: real shell, bookmarks + Keychain + OAuth + connectors + sinks under sandbox; **manual** upload to App Store Connect succeeds if that is the path in use; **MAS-9a** (this doc) + **MAS-9c** dogfood done; **MAS-9b** clears P0/P1 for bar A. | **Partial** — **§3.1–§3.9** desk lenses are recorded (**Partial** where each lens defers hardware dogfood, exhaustive error mapping, or line-by-line audits); **MAS-9b** (P0/P1 sweep) + **MAS-9c** (signed **MAS** evidence + **§1** smoke) still gate a **Pass** for bar A per plan. |
| **B. Release automation complete** | Automated Connect upload on the same cadence as GitHub Releases (or successor); export compliance docs align with upload metadata. | **Partial** — **MAS-8d** [`mas-connect-upload.yml`](../../.github/workflows/mas-connect-upload.yml) + [`MAS-CONNECT-UPLOAD.md`](../release/MAS-CONNECT-UPLOAD.md) + [`MAS-EXPORT-COMPLIANCE.md`](../compliance/MAS-EXPORT-COMPLIANCE.md) are merged; store-signed **`.pkg` in CI** may remain **MAS-8d-followup** (architecture §21). This review iteration does **not** assert a specific operator upload path — record **manual** vs **MAS-8d** `workflow_dispatch` in **Appendix A**, **§5**, or **MAS-9c** notes when evidence exists. |

**MAS-9a** must state whether production uploads were **manual** or **automated**
(per plan executive summary) — **deferred** to **MAS-9c** / operator notes until a dogfood run files evidence.

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

**GitHub compare (full diff):** [`8aaab40...8cb1362`](https://github.com/dayseam/dayseam/compare/8aaab40...8cb1362) — includes **MAS-0b** merge **#214** for context; capstone narrative below starts at **MAS-1a**.

### 2.1 Baseline and head

| | Commit | Label |
|---|--------|-------|
| Baseline (context) | `8aaab40` | [#214](https://github.com/dayseam/dayseam/pull/214) — **MAS-0b** architecture addendum; last first-parent merge before **MAS-1a** |
| In-scope start | `c9eb8d7` | [#216](https://github.com/dayseam/dayseam/pull/216) — **MAS-1a** (first shipped MAS app-code on **`0.13.x`**) |
| Head (capture) | `8cb1362` | [#251](https://github.com/dayseam/dayseam/pull/251) — **MAS-9a** §2 post-#250 + **§3.8 CSP / WebView** lens; tip of **`master`** at that merge |

### 2.2 PRs / merges in scope (first-parent, `c9eb8d7^..8cb1362`, excluding `chore(release)`)

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
| 245 | [#245](https://github.com/dayseam/dayseam/pull/245) | MAS-9a §3.4 filesystem lens + MR review polish |
| 246 | [#246](https://github.com/dayseam/dayseam/pull/246) | MAS-9a §2 post-#245 + §3.2 Errors lens |
| 247 | [#247](https://github.com/dayseam/dayseam/pull/247) | MAS-9a §2 post-#246 + §3.3 Keychain lens |
| 248 | [#248](https://github.com/dayseam/dayseam/pull/248) | MAS-9a §2 post-#247 + §3.5 OAuth lens |
| 249 | [#249](https://github.com/dayseam/dayseam/pull/249) | MAS-9a §2 post-#248 + §3.6 Subprocesses lens |
| 250 | [#250](https://github.com/dayseam/dayseam/pull/250) | MAS-9a §2 post-#249 + §3.7 Capability lens |
| 251 | [#251](https://github.com/dayseam/dayseam/pull/251) | MAS-9a §2 post-#250 + §3.8 CSP / WebView lens |

### 2.3 Surface under review

```text
$ git diff --shortstat 8aaab40..8cb1362
 64 files changed, 4049 insertions(+), 403 deletions(-)
```

Rough centres: `apps/desktop/src-tauri/` (sandbox, bookmarks, Keychain, IPC + **OAuth loopback**, `shell_open` / **`opener`**, `distribution_profile`, **`capabilities/*.json`** + **`tauri.mas.conf.json`** merge), [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) (**WebView CSP**), `apps/desktop/index.html` + `public/` bootstrap scripts, `crates/connectors/` (**libgit2** Local Git), `apps/desktop/src/distribution/` + updater hooks, `docs/compliance/`, `docs/design/2026-phase-5-mas-architecture.md`, `.github/workflows/mas-*.yml`, `scripts/release/mas/`, [`scripts/ci/mas-sandbox-launch-smoke.sh`](../../scripts/ci/mas-sandbox-launch-smoke.sh).

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

- **Status:** **Partial** — `DayseamError` + allocated **`ipc.*`** codes for bookmark flows are line-sourced; stable **`code`** ↔ `error_codes` coverage for every handler remains iterative (**§3.5** OAuth, **§3.7** for Tauri **`allow-*`** parity only).

- **Evidence:** [`error.rs`](../../crates/dayseam-core/src/error.rs) (`DayseamError` variants and stable `code` on every IPC-facing shape), [`error_codes.rs`](../../crates/dayseam-core/src/error_codes.rs) (`IPC_SECURITY_SCOPED_BOOKMARK_*` and adjacent IPC constants), [`ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) (`invalid_config` / `internal`, `map_bookmark_materialize_db_error`, bookmark materialize call sites), [`ipc/oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) + [`oauth_config.rs`](../../apps/desktop/src-tauri/src/oauth_config.rs) (**`OAUTH_LOGIN_*`** — see **§3.5**).

**Taxonomy:** The core crate documents that every UI-visible failure crosses the IPC boundary as **`DayseamError`** with a stable dot-delimited `code` so the frontend can key copy and retry behaviour without parsing prose (`error.rs` module docs). Variants separate auth (`Auth` with `retryable`), structural config (`InvalidConfig`), transport / OS (`Io`, `Network`), operational internals (`Internal`), and non-errors (`Cancelled`).

**MAS bookmark materialization (`MAS-4e–f`):** [`IPC_SECURITY_SCOPED_BOOKMARK_MATERIALIZE_FAILED`](../../crates/dayseam-core/src/error_codes.rs) covers logical-path encoding failures and `create_directory_bookmark` failures — surfaced as **`InvalidConfig`** so the dialog can prompt re-grant or path fixes. [`IPC_SECURITY_SCOPED_BOOKMARK_ROW_MISSING`](../../crates/dayseam-core/src/error_codes.rs) maps `DbError::InvalidData` from transactional blob replacement via `map_bookmark_materialize_db_error` in **`ipc/commands.rs`**. [`IPC_SECURITY_SCOPED_BOOKMARK_STALE_OR_UNUSABLE_SCAN_ROOT`](../../crates/dayseam-core/src/error_codes.rs) is reserved for **MAS-4f** stale-root diagnostics; the constant’s doc comment states it is **not** emitted as a command error in the initial slice — discovery logs and toasts instead (cross-ref **§3.1** / **§3.4**).

**Sync vs materialize errors:** `sync_local_git_security_scoped_rows` / `sync_markdown_sink_security_scoped_rows` map DB failures through `internal("security_scoped_bookmarks.sync…", e)` (stable log `ctx`, generic **`Internal`** to the client) whereas macOS materialization uses the explicit bookmark codes above — intentional split between placeholder alignment vs user-actionable grant failures.

**Other sandbox-adjacent IPC codes (hooks for §3.3 / §3.5 / §3.7):** [`IPC_SHELL_OPEN_FAILED`](../../crates/dayseam-core/src/error_codes.rs) explicitly includes sandbox denial in its doc string; connector `ipc.*.keychain_*` constants cover Keychain read/write failures during source add/reconnect; **`oauth.login.*`** loopback / session failures are desk-reviewed under **§3.5**.

**Gap / follow-up:** No issue opened — **§3.7** now documents **`default.json`** ↔ **`PROD_COMMANDS`** parity (`tests/capabilities.rs`) and **MAS-3** deny wiring; confirming every registered command’s **`DayseamError`** / `error_codes` story (including **`ipc/oauth.rs`** `oauth_*`) remains **§3.2** / **§3.5** desk review.

### 3.3 Keychain (SKU prefix, coexistence with direct build)

- **Status:** **Partial** — compile-time **MAS-5b2** service literals + architecture **§12** contract are line-sourced; **MAS-9c** still owns cold-start / prompt behaviour on signed **MAS** hardware (per **MAS-0b** §12.3 / §12.7).

- **Evidence:** [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) (**MAS-5b2** — `dayseam.mas.*` vs `dayseam.*` under `#[cfg(feature = "mas")]`, unit tests for prefix + distinctness), [**MAS-0b** §12 Keychain](../design/2026-phase-5-mas-architecture.md) (service/account matrix, sandbox entitlements posture, coexistence with direct), [`keychain.rs`](../../crates/dayseam-secrets/src/keychain.rs) (`KeychainStore` → `keyring::Entry` from composite **`service::account`** keys; `split_key` on first **`::`**), [`ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) (`gitlab_secret_ref` + `secret_store_key` composition), connector modules [`github.rs`](../../apps/desktop/src-tauri/src/ipc/github.rs), [`atlassian.rs`](../../apps/desktop/src-tauri/src/ipc/atlassian.rs), [`outlook.rs`](../../apps/desktop/src-tauri/src/ipc/outlook.rs), [`oauth_persister.rs`](../../apps/desktop/src-tauri/src/oauth_persister.rs) (Outlook token rows), [`startup.rs`](../../apps/desktop/src-tauri/src/startup.rs) (orphan-secret audit still probes `SecretRef` slots — **MAS-5a** note in architecture §12.5), [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist) (sandbox on; no custom Keychain access-group entry per §12.3).

**SKU prefix policy:** **`mas`** builds use **`dayseam.mas.<connector>`** service strings; direct keeps historical **`dayseam.<connector>`**. Account shapes (`source:{id}`, `slot:{uuid}`, OAuth suffixes) are unchanged — **MAS-0b** §12.4 treats prefixing as **defensive / UX clarity** on top of **distinct bundle IDs** (`dev.dayseam.desktop` vs `dev.dayseam.mas` per [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json)).

**IPC / error surface:** Keychain write/read failures continue to map to connector-specific **`ipc.*.keychain_*`** codes (see **§3.2**); rollback semantics for partial source add are unchanged from **MAS-5a** audit narrative.

**Gap / follow-up:** No new issue — record **MAS-9c** smoke evidence for “connect → quit → relaunch → token still readable” per architecture §12.7 when dogfood runs.

### 3.4 Filesystem (security-scoped bookmarks, stale/rename, symlinks per **MAS-0b**)

- **Status:** **Partial** — desk review of bookmark persistence + runtime helpers; symlink “escape hatch” behaviour is **not** audited line-by-line in this lens (policy in **MAS-0b** §9.4; exercise via **Canonical MAS smoke** **§1**, item 3 — Local Git scan on nested repo layout).
- **Evidence:** [`security_scoped/mod.rs`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) (**MAS-4b** — `SecurityScopedGuard` / `from_bookmark`, `ResolvedBookmark::is_stale`, `create_directory_bookmark`, non-macOS stubs), [`security_scoped_bookmarks.rs`](../../crates/dayseam-db/src/repos/security_scoped_bookmarks.rs) + [`0007_security_scoped_bookmarks.sql`](../../crates/dayseam-db/migrations/0007_security_scoped_bookmarks.sql) (**MAS-4a**), [`ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) sync/materialize + **MAS-4f** stale-root toasts (cross-ref **§3.1**), [`local_git_scan.rs`](../../apps/desktop/src-tauri/src/local_git_scan.rs) (**MAS-4c** discovery).

**Symlink / rename policy:** [**MAS-0b**](../design/2026-phase-5-mas-architecture.md#94-symlinks) (architecture **§9.4 Symlinks**) documents canonicalization on persist, scan-root containment, and `meta_json` for bookmark rows. This lens assumes implementation tracks that doc; **Gap / follow-up:** file an issue if dogfood (**MAS-9c**) finds a divergence.

### 3.5 OAuth (loopback, parity with direct)

- **Status:** **Partial** — PKCE loopback IPC + **MAS-6b** entitlement story are line-sourced; **MAS-9c** still owns browser → callback → token exchange on a **signed MAS** bundle (**Canonical MAS smoke** **§1**, item 6).

- **Evidence:** [`ipc/oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) (`oauth_begin_login` / `oauth_cancel_login` / `oauth_session_status` — `tokio::net::TcpListener` on **`127.0.0.1`**, module docs for redirect URL + background driver), [`oauth_config.rs`](../../apps/desktop/src-tauri/src/oauth_config.rs) (**DAY-205** — production **`MICROSOFT_LOOPBACK_PORT`** **53691**; tests use ephemeral **`0`**), [`oauth_session.rs`](../../apps/desktop/src-tauri/src/oauth_session.rs) (in-memory session registry; tokens do not cross IPC), [`ipc/outlook.rs`](../../apps/desktop/src-tauri/src/ipc/outlook.rs) + [`oauth_persister.rs`](../../apps/desktop/src-tauri/src/oauth_persister.rs) (post-login Keychain persistence — cross-ref **§3.3**), [**MAS-0b** §13 Networking / §14 OAuth](../design/2026-phase-5-mas-architecture.md) + [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist) (**`com.apple.security.network.client`** + **`network.server`** for sandbox **bind/accept**), [`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md) (**OAuth loopback** prose), [`error_codes.rs`](../../crates/dayseam-core/src/error_codes.rs) (**`OAUTH_LOGIN_*`** definitions).

**`oauth.login.*` vs `DayseamError` (cross-ref **§3.2**):** [`lookup_provider`](../../apps/desktop/src-tauri/src/oauth_config.rs) surfaces **`InvalidConfig`** for **`OAUTH_LOGIN_NOT_CONFIGURED`** (unset **`client_id`**) and **`OAUTH_LOGIN_PROVIDER_UNKNOWN`**. **`oauth_begin_login`** uses **`Internal`** for loopback bind / browser-open failures (**`OAUTH_LOGIN_LOOPBACK_BIND_FAILED`**, **`OAUTH_LOGIN_BROWSER_OPEN_FAILED`**) and for authorize-endpoint URL parse errors (**`build_authorize_url`** reuses the **`OAUTH_LOGIN_NOT_CONFIGURED`** code on **`Internal`** when the configured URL is malformed). The background driver records timeout / CSRF / IdP errors as **`OAuthSessionStatus::Failed`** carrying **`oauth.login.*`** codes for **`oauth_session_status`** / events; successful callback **`exchange_code`** failures propagate the connector’s existing **`DayseamError`** code (not necessarily an **`oauth.login.*`** prefix).

**SKU parity:** The loopback driver carries **no** `#[cfg(feature = "mas")]` fork — direct and **MAS** share the same **`reqwest`** token-exchange path; sandbox differences are **entitlements-only** (**MAS-6b**), asserted in CI via [`verify-tauri-bundle-entitlements.sh`](../../scripts/ci/verify-tauri-bundle-entitlements.sh) (**`direct`** mode **`forbid_key`**-s both network entitlements so a mis-merge cannot ship on Developer ID).

**Gap / follow-up:** No new issue — capture **MAS-9c** evidence for item **6** once dogfood runs on the store SKU; watch **MAS-0b** §14 collision note if a future IdP forces a conflicting fixed port.

### 3.6 Subprocesses / helper binaries (enumeration + sandbox legality — **MAS-0b** §8 baseline)

- **Status:** **Partial** — shipped **macOS** paths line up with **MAS-0b** §8 rows **1–4**; no standalone helper **`.app`** / XPC binaries ship from this repo today (**MAS-0b** “Bundled binaries inside `.app`”).

- **Evidence:** [**MAS-0b** §8 Subprocesses and helper binaries](../design/2026-phase-5-mas-architecture.md) (authoritative enumeration table + “update when adding spawns”), [`shell_open`](../../apps/desktop/src-tauri/src/ipc/commands.rs) (`opener::open` on **`tokio::task::spawn_blocking`** — macOS hands URLs to **`/usr/bin/open`** per **MAS-0b** §8 row **1**; scheme allow-list + errors **`IPC_SHELL_*`**, **§3.2**), [`ipc/oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) (`SystemBrowserOpener` → **`opener::open_browser`** for the authorize URL — **MAS-0b** §8 row **2**; OAuth flow desk-reviewed under **§3.5**), same file **`TcpListener`** loopback (**MAS-0b** §8 row **3** — inbound socket, not a child process), [`connector-local-git` / **`git2`**](../../crates/connectors/connector-local-git/Cargo.toml) (**`vendored-libgit2`** — **MAS-0b** §8 row **4**, no **`git`** CLI on production scan paths). **MAS-0b** §8 row **5** (`MetadataCommand`, `Command::new("git")` in **`tests/`**) is **not** in the shipped bundle.

**Sandbox alignment:** **`shell_open`** stays **user-driven** (no background `open`); **OAuth** browser launch is paired with the loopback listener (**§3.5** / **MAS-6b**). Local Git repo access remains **bookmark-scoped** under **MAS** (**§3.4**), not arbitrary POSIX reads.

**Gap / follow-up:** No issue opened — any new **`Command::new`**, bundled helper, or XPC agent must extend **MAS-0b** §8 **and** be re-reviewed here + in **§3.7** (capability matrix).

### 3.7 Capability deny-list vs **MAS-0b** matrix

- **Status:** **Partial** — **Tauri** capability split (**MAS-0b** §6) is line-sourced against committed JSON + **MAS** merge config; **entitlement** deny-list prose (**MAS-0b** §5) is not re-derived cell-by-cell here (tracked via **§2** / **MAS-2a+** workstreams). **CSP** / **WKWebView** policy string is **§3.8**.

- **Evidence:** [**MAS-0b** §6 Tauri capability matrix (direct vs MAS)](../design/2026-phase-5-mas-architecture.md) (same production **`allow-*`** command surface on both SKUs; **MAS** omits **`updater.json`** and denies **`updater:*`** + **`process:allow-restart`**), [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) (**`app.security.capabilities`: [`"default"`]** only; **`plugins`: {}** so the **MAS** bundle never merges the updater capability identifier), [`capabilities/default.json`](../../apps/desktop/src-tauri/capabilities/default.json) (**`core:default`**, one **`allow-<command>`** per [`PROD_COMMANDS`](../../apps/desktop/src-tauri/src/ipc/commands.rs) entry, plus **`dialog:allow-open`** — enforced by [`tests/capabilities.rs`](../../apps/desktop/src-tauri/tests/capabilities.rs)), [`capabilities/updater.json`](../../apps/desktop/src-tauri/capabilities/updater.json) (direct-only **`updater:allow-check`** / **`updater:allow-download-and-install`** + **`process:allow-restart`**), [`main.rs`](../../apps/desktop/src-tauri/src/main.rs) (**MAS-3a** — **`#[cfg(not(feature = "mas"))]`** registers **`tauri-plugin-updater`** + **`tauri-plugin-process`**; **`mas`** build skips both), [`build.rs`](../../apps/desktop/src-tauri/build.rs) (single **`PROD_COMMANDS`** manifest for **`tauri-build`**; header documents the **four** touch points when adding a command).

**Deny-list alignment:** Effective **MAS** runtime grants match **MAS-0b** §6 “**MAS**” column: no updater permissions, no process relaunch permission, **`dialog:allow-open`** retained for bookmark pickers (**§3.1** / **MAS-4**).

**Gap / follow-up:** No issue opened — new **`#[tauri::command]`** handlers must stay in lockstep with **`default.json`** and **`@dayseam/ipc-types`** per **`build.rs`**; re-run **`tests/capabilities.rs`** after edits. If a command is **unsandboxable**, gate or replace it before widening **`default.json`** (architecture §6 “same command set unless provably unsandboxable”).

### 3.8 CSP / WebView exposure

- **Status:** **Partial** — production **Content-Security-Policy** is **SKU-neutral** today (**MAS-1a** single webview bundle); **MAS** does not fork the CSP string in [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json).

- **Evidence:** [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) **`app.security.csp`**: `default-src 'self'`, `img-src 'self' data:`, `style-src 'self' 'unsafe-inline'`, `script-src 'self'` (no remote script origins, no `'unsafe-eval'`); [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) **`app.security`** carries **no** **`csp`** key — **MAS** builds inherit the merge-base policy from [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json); [`entitlements.md`](../../apps/desktop/src-tauri/entitlements.md) (CSP + hardened-runtime framing); [**MAS-0b** §6 Tauri capability matrix](../design/2026-phase-5-mas-architecture.md) ("CSP and IPC allow-list discipline stay identical where possible"); [`index.html`](../../apps/desktop/index.html) + [`public/hydrate-theme.js`](../../apps/desktop/public/hydrate-theme.js) (external **`/hydrate-theme.js`** script stays on **`script-src 'self'`** without CSP hashes — documented in-tree).

**JIT / WebKit entitlements:** **MAS** ships **MAS-2c** keys for **WKWebView** (`entitlements.mas.plist` — cross-ref **MAS-0b** §5 / [`MAS-JIT-ENTITLEMENTS.md`](../compliance/MAS-JIT-ENTITLEMENTS.md)); that is **not** a CSP relaxation — App Review copy still pairs them ([`MAS-APP-REVIEW-NOTES.md`](../compliance/MAS-APP-REVIEW-NOTES.md)).

**Gap / follow-up:** No issue opened — widening **`connect-src`**, **`frame-src`**, **`script-src`**, or adding **`unsafe-inline`** on scripts requires **MAS-0b** §6 alignment + **MAS-7c** paste-pack refresh; **MAS-9c** should exercise any future **`blob:`** navigations or **`data:`** uses **outside** the existing **`img-src 'self' data:`** allowance in the signed **MAS** bundle.

### 3.9 **`cfg` / `feature = "mas"` inventory** (*Single codebase* exit rule)

- **Status:** **Partial** — the table + **`rg`** sweeps below satisfy the plan's *Single codebase* rule alongside **§3.1–§3.8**; each row is a deliberate **`mas`** / packaging / entitlements / capability / UX delta documented under [**MAS-0b** §3 — Packaging vs entitlement vs runtime vs UX](../design/2026-phase-5-mas-architecture.md#3-packaging-vs-entitlement-vs-runtime-vs-ux) (taxonomy of **Packaging**, **Entitlements**, **Capability allow-lists**, **Store metadata**, **UX**).

- **Evidence:** Rows in this subsection (initial **[#243](https://github.com/dayseam/dayseam/pull/243)** scaffold, extended with **`tauri.conf.json`** in **[#251](https://github.com/dayseam/dayseam/pull/251)**); cross-check **§3.1** (`distribution_profile`, bookmark IPC), **§3.2** (errors / **`DayseamError`** / **`error_codes`**), **§3.3** (`keychain_profile`), **§3.4** (`local_git_scan`), **§3.5–§3.6** (`ipc/oauth`), **§3.7** (capabilities / **`main.rs`** plugins), **§3.8** (shared CSP).

- **Gap / follow-up:** No issue opened — re-run both **`rg`** commands after any **`mas`** / IPC / distribution edit; new user-visible **`mas`** branching outside the plan buckets still needs a **linked removal issue** or explicit sign-off (capture in **§4**).

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
| [`apps/desktop/src-tauri/tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) | WebView CSP (`app.security.csp`); `build.devUrl` dev-only | Yes — **MAS-0b** §6 + **§3.8** (shared across SKUs) | — |
| [`apps/desktop/package.json`](../../apps/desktop/package.json) | `tauri:build:mas` script | Yes — **packaging** | — |
| [`apps/desktop/src-tauri/src/startup.rs`](../../apps/desktop/src-tauri/src/startup.rs) | `DATA_SUBDIR` / path roots under `#[cfg(feature = "mas")]` | Yes — **MAS-5b1** coexistence / **MAS-0b** §10 | — |
| [`apps/desktop/src-tauri/src/lib.rs`](../../apps/desktop/src-tauri/src/lib.rs) | Tauri builder + capability split | Yes — **packaging / capabilities**; **§3.7** matrix vs **MAS-0b** §6 | — |
| [`apps/desktop/src-tauri/src/keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) | Keychain service / account strings | Yes — **MAS-5b2** (**§3.3** desk review) | — |
| [`apps/desktop/src-tauri/src/main.rs`](../../apps/desktop/src-tauri/src/main.rs) | Updater / menu / single-instance registration | Yes — **MAS-3** updater removal | — |
| [`apps/desktop/src-tauri/src/local_git_scan.rs`](../../apps/desktop/src-tauri/src/local_git_scan.rs) | Default scan roots vs security-scoped MAS discovery | Yes — **MAS-4c** filesystem contract | — |
| [`apps/desktop/src-tauri/src/ipc/commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) | Bookmarks, `distribution_profile`, `shell_open` (**`opener::open`**), folder pickers, `secret_store_key` / connector secret refs, `#[cfg(all(feature = "mas", target_os = "macos"))]` branches | Yes — **IPC + FS** tasks **MAS-4a–f**; **§3.1 / §3.2 / §3.3 / §3.4 / §3.6 / §3.8** must still sign off pass vs gap | — |
| [`apps/desktop/src-tauri/src/ipc/oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) | PKCE loopback listener + session registry (no `mas` cfg fork in flow) | Yes — **MAS-6b** entitlements + documented **DAY-205** port policy; **§3.5** + **§3.6** desk review | — |
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

**Lens coverage (this iteration):** **§3.1–§3.8** + **§3.9** inventory are desk-reviewed; remaining **Partial** flags in each lens defer to **MAS-9c** (hardware-signed runs, smoke checklist **§1**) and/or **MAS-9b** (code defects). **§0** bar **A** / **B** stay **Partial** until that evidence lands.

---

## Appendix A — Optional: command log / screenshots

*Reserve for paste-heavy evidence.*
