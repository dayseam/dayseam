# Phase 5 (MAS): Architecture addendum

> **Status:** normative for Mac App Store engineering on the **`0.13.x`** line. The implementation task catalogue and semver ladder live in [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md) (**#210**). This document answers the coexistence, bookmark, capability, JIT, subprocess, and release-skew questions the plan marks as **MAS-0b** outputs.

---

## 1. Goals and non-goals

**Goals**

- Ship a **second distribution SKU** of Dayseam for the **Mac App Store**: sandboxed, signed with a **Mac App Distribution** identity, updated **only** through App Store Connect (no Tauri in-app updater).
- Keep **one Rust + TypeScript codebase**. Long-term divergence is limited to packaging, entitlements, capability JSON, store metadata, and narrowly scoped compile-time distribution profile (see §17).
- Preserve the **direct-download** SKU (Developer ID / notarized DMG, GitHub Releases, in-app updater) with **no behavioural regression** unless a change is explicitly dual-SKU and reviewed.

**Non-goals** (see plan for full list)

- Replacing direct-download as the primary development or power-user channel.
- Windows Store, App Store marketing assets, or click-by-click Apple portal runbooks (operators still own secrets and identifiers outside this repo).

---

## 2. Dual-SKU system picture

One repository produces **two macOS app bundles** with different signing stories, entitlements, and update mechanics. Shared semver on `master` is the **version source of truth** for both channels; release **timing** may differ (§16).

```mermaid
flowchart LR
  subgraph Source["Single `master`"]
    M["Rust core + connectors + sinks"]
    T["Tauri shell + React UI"]
  end

  subgraph Direct["Direct SKU"]
    D1["`tauri.conf.json` defaults"]
    D2["`entitlements.plist`\nno App Sandbox"]
    D3["capabilities:\ndefault + updater"]
    D4["GitHub Release +\n`latest.json` updater"]
  end

  subgraph MAS["MAS SKU"]
    M1["Tauri config merge\n(`mas` / packaging)"]
    M2["`entitlements.mas.plist`\nApp Sandbox + allow-list"]
    M3["capabilities:\nMAS JSON\n(no updater)"]
    M4["App Store Connect\nupload + review"]
  end

  Source --> Direct
  Source --> MAS
```

---

## 3. Packaging vs entitlement vs runtime vs UX

Anything “Apple required” must land in the **correct column** so we do not accidentally justify **runtime `if (mas)`** business logic.

| Column | Holds | Examples |
|--------|--------|----------|
| **Packaging** | Bundle id, product name suffix, targets (`dmg` vs `app` only), `createUpdaterArtifacts`, signing identity, export method. | MAS bundle id **distinct** from direct (§9); updater artifacts **off** on MAS. |
| **Entitlements** | Keys in `entitlements.plist` vs `entitlements.mas.plist`. | Sandbox, network client/server, JIT-related keys (§6), user-selected file access. |
| **Capability allow-lists** | Tauri v2 `capabilities/*.json` grants. | Strip `updater:*`, `process:allow-restart`, and any command not reachable under sandbox (MAS-3). |
| **Store metadata** | Privacy manifest, export compliance prose, review notes. | `PrivacyInfo.xcprivacy` (**MAS-7a**), `MAS-EXPORT-COMPLIANCE.md` (**MAS-7b**). |
| **UX** | User-visible differences tied to distribution. | No “Check for updates” on MAS; single `distribution_profile` enum preferred over scattered checks (plan: *Single codebase*). |

**Runtime behaviour** (bookmarks, scoped FS, Keychain, OAuth) should stay **unified** across SKUs where feasible; the direct build may adopt the same security-scoped access patterns to reduce drift.

---

## 4. Threat model delta (direct vs MAS)

Today’s direct macOS build **opts out of App Sandbox** and instead relies on hardened runtime + narrow IPC + CSP + explicit Tauri capabilities. Rationale is documented in [`apps/desktop/src-tauri/entitlements.md`](../../apps/desktop/src-tauri/entitlements.md) (user-selected read-write, JIT-style allowances for the WebView stack).

**MAS** inverts the constraint: **App Sandbox is mandatory**. That implies:

- **Default-deny filesystem** outside container and without **security-scoped bookmarks** (or picker-granted scope).
- **Outbound network** is entitlement-gated; every connector host must be anticipated or user-driven (HTTPS).
- **Child processes / Mach services** are heavily restricted; anything that today shells out must be audited (§7).
- **Keychain** and **OAuth loopback** remain required but must be validated under sandbox (plan blocks **MAS-5**, **MAS-6**).

---

## 5. Entitlement matrix (direct vs MAS)

**Direct** (`apps/desktop/src-tauri/entitlements.plist`) — current keys (see [`entitlements.md`](../../apps/desktop/src-tauri/entitlements.md) for prose):

| Key | Direct | Notes |
|-----|--------|--------|
| `com.apple.security.files.user-selected.read-write` | **on** | TCC persistence for `dialog.open` grants. |
| `com.apple.security.cs.allow-unsigned-executable-memory` | **on** | Hardened-runtime JIT-style allowance (Tauri / native deps). |
| `com.apple.security.cs.allow-jit` | **on** | Same family as Electron/Tauri guidance. |
| `com.apple.security.app-sandbox` | **off** | Explicit product decision today. |

**MAS** (`entitlements.mas.plist` — introduced in **MAS-1b**, tightened in **MAS-2a+**):

| Key | MAS (initial stub → target) | Notes |
|-----|------------------------------|--------|
| `com.apple.security.app-sandbox` | **on** (**MAS-2a**) | Store requirement. |
| `com.apple.security.network.client` | **on** when needed (**MAS-2a** / **MAS-6a**) | All connectors are HTTPS clients. |
| `com.apple.security.files.user-selected.read-write` | **TBD with bookmarks** | Under sandbox, picker + bookmark flow must match **MAS-4**; may differ from direct’s standalone key semantics — validate against Apple’s matrix for sandboxed apps. |
| `com.apple.security.cs.allow-jit` / `…allow-unsigned-executable-memory` | **on** (**MAS-2c**) | Same keys as direct [`entitlements.plist`](../../apps/desktop/src-tauri/entitlements.plist); justified for WKWebView + in-process native deps — canonical text in [`docs/compliance/MAS-JIT-ENTITLEMENTS.md`](../compliance/MAS-JIT-ENTITLEMENTS.md) (feeds **MAS-7c**). **Fallback** if App Review rejects: WebKit/Tauri narrowing → upstream issue → hold SKU (see that doc). |

**Footnote (MAS-2a vs MAS-4):** `user-selected.read-write` stays **on** in `entitlements.mas.plist` at **MAS-2a** for parity with the direct picker story; **MAS-4** defines the security-scoped bookmark contract that makes that key meaningful under sandbox — the matrix “TBD” row is about *semantics*, not “key absent”.

**MAS deny-list (entitlements)**

- No **hardened runtime–incompatible** “escape hatch” entitlements unless justified and declared for review (e.g. temporary exceptions Apple grants in writing).
- No **debugging** entitlements in shipping store builds.
- Anything that grants **unscoped filesystem** or **arbitrary IPC** to other apps is incompatible with store policy — if a feature needs it, the feature must be redesigned (not smuggled as entitlement).

---

## 6. Tauri capability matrix (direct vs MAS)

**Direct production** merges:

- [`apps/desktop/src-tauri/capabilities/default.json`](../../apps/desktop/src-tauri/capabilities/default.json) — IPC command allow-list + `dialog:allow-open` + `core:default`.
- [`apps/desktop/src-tauri/capabilities/updater.json`](../../apps/desktop/src-tauri/capabilities/updater.json) — `updater:allow-check`, `updater:allow-download-and-install`, `process:allow-restart`.

**MAS target** (concrete JSON delivered in **MAS-3a**; this subsection is the **intent matrix**):

| Area | Direct | MAS |
|------|--------|-----|
| Core / IPC surface used by production UI | `default.json` as today | **Same command set** unless a command is provably unsandboxable — then gate or replace with scoped alternative. |
| `dialog:allow-open` | allowed | **allowed** (required for folder pickers + bookmark seeding). |
| Updater plugin permissions | `updater.json` merged | **omit entire file** — no `updater:*`, no `process:allow-restart`. |
| Dev-only commands | `dev-commands` feature only | **never** in store bundle (already true for direct release builds). |

**Deny-list summary for MAS bundle**

- All `updater:*` permissions.
- `process:allow-restart` (only used for post-update relaunch).
- Any future permission that implies **unsandboxed** power (broad shell, arbitrary code load) without App Store narrative.

Nothing in the MAS matrix should **widen** the attack surface “because MAS is safer” — CSP and IPC allow-list discipline stay identical where possible.

---

## 7. JIT / executable memory (evidence and fallback)

**MAS-2c** is documented in [`docs/compliance/MAS-JIT-ENTITLEMENTS.md`](../compliance/MAS-JIT-ENTITLEMENTS.md): exact entitlement keys, macOS **arm64 / x86_64** scope, engineering rationale, an **App Review–ready prose seed** for **MAS-7c**, and the **fallback** ladder (**maintain the numbered steps only in that file** so App Review copy and engineering narrative stay one source of truth).

**Version inventory** for compliance lives in **§16** (`Cargo.lock` snapshot). Optional deep evidence (`nm` / dylib maps) is **on demand** for App Review or legal — not a standing gate for every patch.

---

## 8. Subprocesses and helper binaries (baseline for MAS-9a)

This table is the **authoritative enumeration baseline** for capstone subprocess review. Update when adding spawns.

| # | Mechanism | Call sites / crate | What it spawns | Sandbox notes |
|---|-----------|--------------------|----------------|---------------|
| 1 | `opener::open` | `shell_open` in [`commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) | macOS: hand-off to **`/usr/bin/open`** (user-initiated; scheme allow-list includes `http`, `https`, `file`, …). | Must remain **user-driven**; no background open. URL policy unchanged. |
| 2 | `opener::open_browser` | OAuth in [`oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) | Default browser for authorize URL. | Same as above; paired with loopback listener. |
| 3 | `tokio::net::TcpListener` | `oauth.rs` — `127.0.0.1` loopback for OAuth redirect | No child process; **inbound localhost** socket. | Requires **network entitlement** analysis (**MAS-6a**); document loopback port pinning vs ephemeral tests. |
| 4 | **libgit2** (vendored) | `connector-local-git` via `git2` crate with `vendored-libgit2` | **No** `git` CLI subprocess; native library inside Dayseam address space. | Sandboxed FS access must go through **security-scoped** paths from bookmarks (**MAS-4**), not arbitrary POSIX paths from persisted config. |
| 5 | **Tests / dev only** | `MetadataCommand`, `Command::new("git")` in various `tests/` crates | `cargo test` helpers | Not shipped in production bundle. |

**Bundled “binaries” inside `.app`**

- Main executable `dayseam-desktop`, embedded WebView content, static assets — all covered by Tauri’s bundle.
- No separate helper **agent** binaries in-repo today; if added later (e.g. standalone scheduler), each requires its own sandbox story + review row.

---

## 9. Security-scoped bookmarks (design contract for MAS-4)

This section satisfies the plan’s **bookmark contract** checklist; implementation tasks are **MAS-4a–f**.

### 9.1 Granularity

- **Scan roots (local Git)** — persist a **directory** security-scoped bookmark per configured root (the directory the user chose in `dialog.open`). Nested repositories are discovered **under** that directory.
- **Sink folders (markdown file / Obsidian)** — persist a **directory** bookmark for each sink root the user grants.
- **Saving a new file inside an already-bookmarked sink folder** — **reuse the parent directory bookmark** for writes within that tree; do **not** require a per-file bookmark for routine report writes. If the user picks a **new** output path outside granted dirs, show picker again.

### 9.2 Descendants and cold start

After relaunch, the app must **resolve** each stored bookmark to a file URL before passing paths to `git2` or sink adapters. **Nested repos** under a bookmarked scan root are accessible **iff** they remain within the resolved directory subtree and the bookmark is still valid. Implementation must not assume POSIX access without `startAccessingSecurityScopedResource` (or RAII equivalent) around each batch of filesystem work (**MAS-4b**). **MAS-4c:** IPC-time Local Git discovery after `sources_add` / `sources_update` uses [`local_git_scan`](../../apps/desktop/src-tauri/src/local_git_scan.rs) on **macOS + `mas`** when `bookmark_blob` is set; connector runtime refresh still uses plain `discover_repos` until a later task threads bookmarks through the orchestrator path. **MAS-4d:** `sinks_add` (MAS builds) syncs `security_scoped_bookmarks` placeholder rows for **`MarkdownFile.dest_dirs`** via [`SecurityScopedBookmarkRepo::sync_markdown_sink_dest_dirs`](../../crates/dayseam-db/src/repos/security_scoped_bookmarks.rs). **MAS-4e:** On **macOS + `mas`**, the same IPC handlers **materialize** [`create_directory_bookmark`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) bytes into **`bookmark_blob`** after placeholder sync (Linux `mas` CI skips; blobs stay **`NULL`**). Runtime Markdown sink writes still use plain paths until orchestrator threading for sinks.

### 9.3 Rename / move / stale bookmarks

- Detect resolution failures and `ENOENT` after successful resolve as **stale bookmark**.
- Map to **`DayseamError`** with stable **`error_codes`** (allocated in **MAS-4f**).
- UX: toast + **“Reselect folder in Settings”** (or source/sink edit sheet) that reopens `dialog.open` and replaces the bookmark blob.

### 9.4 Symlinks

- **Policy:** when persisting a bookmark, resolve the user’s selection to a **canonical real path** (`std::fs::canonicalize` or equivalent) and store metadata indicating whether the path was symlinked.
- **Scan roots:** follow symlinks **only** if the canonicalized path still lies under the user-selected root **after** canonicalization; otherwise **reject** with user-facing copy (“alias escapes the selected folder”).
- Document edge cases (macOS **firmlinks**, `/private` prefixes) in **MAS-4** tests.

### 9.5 Access lifetime (RAII)

- **No session-wide blanket** `startAccessing…` for the whole app lifetime.
- Use a **RAII guard** (or explicit `defer`-style scope) per **operation batch** (single sync walk, single report generation, single sink write).
- **Long-running jobs** (scheduled catch-up, large repo walk): one guard spanning the **job lifecycle** only; release promptly on completion/cancel.
- **Implementation (MAS-4b):** Rust helpers live in [`security_scoped/mod.rs`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) (`create_directory_bookmark`, `resolve_bookmark`, `SecurityScopedGuard`). Prefer **`SecurityScopedGuard::from_bookmark`** after rehydrating stored bytes so `startAccessing…` runs on the resolved `NSURL`, not only a reconstituted file path.

### 9.6 Persistent storage (**MAS-4a**)

- **Table:** `security_scoped_bookmarks` — migration [`0007_security_scoped_bookmarks.sql`](../../crates/dayseam-db/migrations/0007_security_scoped_bookmarks.sql) in [`dayseam-db`](../../crates/dayseam-db/).
- **Owner shape:** exactly one of `owner_source_id` (**`role = local_git_scan_root`**) or `owner_sink_id` (**`role = markdown_sink_dest`**), enforced with `CHECK` constraints; both FKs **`ON DELETE CASCADE`**.
- **`logical_path`:** must match the corresponding path string in `sources.config_json` (`LocalGit.scan_roots`) or `sinks.config_json` (`MarkdownFile.dest_dirs`); partial **`UNIQUE`** indexes block duplicate grants per owner + path.
- **`bookmark_blob`:** opaque macOS bookmark bytes — created by [`create_directory_bookmark`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) and persisted under **MAS-4e** when sources/sinks are saved on **macOS** (`NULL` until then, including Linux **`mas`** CI where materialization is skipped). **`meta_json`:** optional §9.4 metadata (canonical path, symlink policy).

---

## 10. Direct ↔ MAS coexistence

**Decision: concurrent installation is allowed** once the MAS bundle uses a **distinct bundle identifier** and **distinct on-disk state namespace**. Until the MAS bundle id is minted in App Store Connect, treat the literal string as **`TBD_MAS_BUNDLE_ID`** in engineering docs — the **implementation** must replace placeholders before shipping.

| Concern | Direct (today) | MAS (required) |
|---------|----------------|----------------|
| **Bundle id** | `dev.dayseam.desktop` from [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) | **Distinct** App Store id (e.g. `dev.dayseam.mas` — final choice is operator-owned). |
| **Application Support path** | `~/Library/Application Support/dev.dayseam.desktop/` via [`startup.rs`](../../apps/desktop/src-tauri/src/startup.rs) `DATA_SUBDIR` (direct SKU, default features) | `~/Library/Application Support/dev.dayseam.mas/` when built with **`mas`** (**MAS-5b1**): same source file selects `DATA_SUBDIR` via `#[cfg(feature = "mas")]`, matching [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) **`identifier`**. SQLite + logs never share a directory with direct when both SKUs are installed. |
| **SQLite `state.db`** | One file per install | **Two independent files** when both SKUs installed — **no** automatic merge. |
| **Lock files** (e.g. markdown sink `.dayseam.lock`) | Per sink path | Same as SQLite — separate installs mean separate lock namespaces unless user points both apps at the **same** folder (advanced; see risk). |
| **Keychain** | Rows keyed by `service::account` strings; service names `dayseam.<connector>` via [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) | **MAS-5b2:** `mas` builds use `dayseam.mas.<connector>` (same module). **MAS-5a:** bundle-id isolation still applies; prefixes are for Keychain Access clarity when both SKUs are installed. |
| **Custom URL schemes / deep links** | Minimal / none for OAuth (loopback HTTP) | If a **registered scheme** is added later, it **must not** collide between SKUs (Apple registers schemes per bundle id — still document for support). |

**Risk:** user configures **both** apps to write into the **same** Obsidian vault without coordination — possible **write races**. Mitigation: support docs recommend one active writer per vault; not a code blocker for Phase 5.

---

## 11. Migration (direct → MAS)

| Artifact | Behaviour |
|----------|-------------|
| **SQLite rows** (sources, sinks, settings) | **Logical migration** only: export/import or “fresh start” is acceptable for v1 MAS; absolute paths in rows may be **invalid** under sandbox until user re-picks via bookmark flow. |
| **Absolute paths in config** | Likely **break** until user re-authorizes through security-scoped bookmarks — do not silently rewrite paths across different volume / sandbox semantics. |
| **Keychain tokens** | **Not** auto-migrated between different service prefixes; user reconnects OAuth / PAT once per MAS install (or explicit migration tool in a future phase if product demands it). |
| **Updater prefs / `latest.json` cache** | **Ignored** on MAS — no in-app updater UI or network calls (**MAS-3**). |
| **Scheduler / background agent** | If enabled on direct, MAS build may need **different** entitlements or user education — track under **MAS-9a** if agent ships before MAS launch. |

---

## 12. Keychain (**MAS-5a** audit)

### 12.1 Storage model

- **`SecretRef`** (`dayseam-core`) records `keychain_service` + `keychain_account`; IPC composes a single lookup key `service::account` via [`secret_store_key`](../../apps/desktop/src-tauri/src/ipc/commands.rs).
- **`KeychainStore`** ([`keychain.rs`](../../crates/dayseam-secrets/src/keychain.rs)) delegates to the **`keyring`** crate on macOS (Security framework / Keychain Services). Desktop depends on **`dayseam-secrets`** with **`features = ["keychain"]`** ([`Cargo.toml`](../../apps/desktop/src-tauri/Cargo.toml)).

### 12.2 Service / account matrix (production)

**MAS-5b2:** [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) defines connector service literals per SKU — direct builds keep `dayseam.<connector>`; **`mas`** builds use **`dayseam.mas.<connector>`**. Account shapes are unchanged.

| Integration | `keychain_service` (direct / **`mas`**) | Account shape | Primary Rust source |
|-------------|-------------------|---------------|---------------------|
| GitLab PAT | `dayseam.gitlab` / **`dayseam.mas.gitlab`** | `source:{SourceId}` | [`commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs) (`gitlab_secret_ref`) |
| GitHub PAT | `dayseam.github` / **`dayseam.mas.github`** | `source:{SourceId}` | [`github.rs`](../../apps/desktop/src-tauri/src/ipc/github.rs) |
| Atlassian PAT | `dayseam.atlassian` / **`dayseam.mas.atlassian`** | `slot:{Uuid}` | [`atlassian.rs`](../../apps/desktop/src-tauri/src/ipc/atlassian.rs) |
| Outlook OAuth | `dayseam.outlook` / **`dayseam.mas.outlook`** | `source:{SourceId}.oauth.access` / `.oauth.refresh` | [`outlook.rs`](../../apps/desktop/src-tauri/src/ipc/outlook.rs) |

### 12.3 App Sandbox + entitlements

- **`entitlements.mas.plist`** enables App Sandbox ([`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist)) but does **not** declare a custom Keychain **access-group** plist entry. That matches typical **single-app** macOS sandbox usage: the process reads and writes Keychain items owned by **the same signed application** without an extra entitlement beyond sandbox + signing.
- **Manual validation still required on real hardware:** confirm PAT/OAuth flows persist and reload tokens after cold start on the signed **MAS** bundle ([`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) sets **`identifier`** to **`dev.dayseam.mas`**), including any Keychain authorization prompts. Track regressions under **MAS-5b2** (Keychain) or **MAS-5b1** (`DATA_SUBDIR` / DB path) as appropriate.

### 12.4 Coexistence (direct + MAS installed)

- **§10 optional policy** — distinct `service` prefixes per SKU (**MAS-5b2**, [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs)): **`mas`** builds use `dayseam.mas.*`; direct unchanged.
- **Platform isolation:** macOS ties Keychain items to the **app’s code signing identity / bundle**. The direct bundle (`dev.dayseam.desktop`) and MAS bundle (`dev.dayseam.mas`) are **different apps**; tokens created by one SKU should **not** overwrite rows belonging to the other **even when service + account strings match**. The prefix policy is therefore **defensive / UX clarity**, not the only isolation mechanism.
- **SQLite `SecretRef` rows** remain per-database-file; co-installed apps use **different** Application Support paths (**MAS-5b1**, §10) and **distinct `keychain_service` strings on `mas`** (**MAS-5b2**).

### 12.5 Boot-time behaviour + tests

- **Orphan-secret audit** ([`startup.rs`](../../apps/desktop/src-tauri/src/startup.rs)) probes `SecretRef` slots asynchronously so the UI is not blocked by sequential Keychain prompts — behaviour unchanged by **MAS-5a**.
- **Automated tests:** `dayseam-secrets` unit-tests `split_key` on every platform; Linux CI does not exercise real Keychain I/O. **MAS-5a** adds no new Keychain-focused tests (per plan: mock tests unchanged).

### 12.6 Ordering vs OAuth

- OAuth loopback completes **before** persisting tokens via `outlook_sources_add` / reconnect — unchanged intent. Network + loopback parity under sandbox is **MAS-6b**; Keychain write failures after OAuth remain surfaced as today.

### 12.7 Follow-ups → **MAS-5b1** / **MAS-5b2** (plan)

| Item | Plan ID | Notes |
|------|---------|--------|
| **`DATA_SUBDIR` / Application Support** | **MAS-5b1** | MAS-specific Application Support / `state.db` path in [`startup.rs`](../../apps/desktop/src-tauri/src/startup.rs) (§10 / §20); orthogonal to Keychain `service` strings. |
| **SKU-specific `keychain_service` prefix** (`dayseam.mas.*` when **`mas`**) | **MAS-5b2** | [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs); regression tests; pre-release MAS testers reconnect once if `SecretRef` rows still reference unprefixed services. |
| **Sandbox smoke** | Evidence in **MAS-9c** / manual passes | Connect each connector once on a signed MAS `.app`, quit, relaunch, verify token read. |

---

## 13. Networking

- **Outbound:** connectors use `reqwest` (HTTPS). MAS entitlements must allow **client TLS** to user-configured hosts (GitLab self-host, enterprise GitHub, etc.) — exact pattern in **MAS-6a** (broad client entitlement vs per-host plist keys is an Apple-policy choice).
- **Inbound:** OAuth loopback listener on `127.0.0.1` (**MAS-6b** parity with direct; rate limits and retry behaviour unchanged unless a sandbox bug forces a delta).

---

## 14. OAuth

- **Loopback redirect** is core to Outlook (and future OAuth) — documented in [`oauth.rs`](../../apps/desktop/src-tauri/src/ipc/oauth.rs) module docs.
- **Collision with two SKUs:** two apps → two independent loopback servers **only if** both run OAuth simultaneously; same `127.0.0.1` port conflicts are possible if Microsoft ever forces a **fixed** port collision — today production uses a pinned port constant (**DAY-205**); document test vs prod divergence and mitigation (serialize logins, ephemeral port where IdP allows).

---

## 15. Updater

- **Direct:** Tauri updater + [`updater.json`](../../apps/desktop/src-tauri/capabilities/updater.json) capability + `createUpdaterArtifacts: true` in [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json).
- **MAS:** **MAS-3** — no in-app auto-update, no `latest.json` polling, no swap-and-relaunch — updates **only** via App Store. [`main.rs`](../../apps/desktop/src-tauri/src/main.rs) registers **`tauri-plugin-updater`** / **`tauri-plugin-process`** only when **`not(feature = "mas")`**; [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) merges **`app.security.capabilities: ["default"]`** and **`plugins: {}`** so **`updater.json`** is not active. The webview reads **`distribution_profile`** ([`commands.rs`](../../apps/desktop/src-tauri/src/ipc/commands.rs)) once and [`useUpdater`](../../apps/desktop/src/features/updater/useUpdater.ts) skips plugin calls when the profile is **`mas`** (Vitest covers the negative path). Cargo still lists the updater crates so **`cargo test --workspace --all-features`** does not need optional-deps juggling; dead registration paths are compile-time gated only.

---

## 16. Privacy and third-party SDK inventory (**MAS-2b** → **MAS-7a**)

**MAS-2b** (this table) inventories embedded / linked behaviour the App Store privacy manifest must eventually describe (**MAS-7a**). **`PrivacyInfo.xcprivacy` is absent** in-tree today — every row’s manifest column is **no (MAS-7a)** until that task lands.

**Version source:** `Cargo.lock` at **2026-04-30** on `master` (refresh rows when upgrading these crates).

| SDK / component | Version(s) in tree | Ships in MAS bundle? | `PrivacyInfo.xcprivacy`? | Gap / owner |
|-----------------|-------------------|----------------------|--------------------------|-------------|
| **Tauri** (shell, IPC, bundler) | `tauri` **2.10.3** | yes | no (MAS-7a) | Required-use APIs + any declared data types — Desktop |
| **WRY** (WebView host) | `wry` **0.54.4** | yes | no (MAS-7a) | WebKit / file URL / drag-drop — Desktop |
| **TAO** (windowing) | `tao` **0.34.8** | yes | no (MAS-7a) | Native window / menu / tray integration — Desktop |
| **WebKit** (system) | OS-provided WKWebView | yes | no (MAS-7a) | Apple-supplied framework; manifest must align with actual WebView usage — Desktop |
| **`sqlx` + SQLite** | `sqlx` **0.8.6** (`libsqlite3-sys` **0.30.1**) | yes | no (MAS-7a) | Disk persistence, migrations — Core |
| **`reqwest` + TLS** | `reqwest` **0.12.28** / **0.13.2**; `rustls` **0.23.38**; `webpki-roots` **1.0.7** | yes | no (MAS-7a) | Outbound HTTPS to user-configured hosts — Connectors |
| **`git2` / libgit2** | `git2` **0.20.4**; `libgit2-sys` **0.18.3+1.9.2** | yes | no (MAS-7a) | Local repo read/write — Local-git |
| **`opener`** | **0.7.2** | yes | no (MAS-7a) | Opens URLs / paths in user’s default apps — Desktop |
| **`keyring`** | **2.3.3** | yes (macOS) | no (MAS-7a) | OS credential storage — Secrets |
| **`minisign-verify`** (via `tauri-plugin-updater`) | **0.2.5** | yes (crate still linked; updater plugin **not** initialized on MAS — **MAS-3**) | no (MAS-7a) | Ed25519 verify path inside the updater plugin — Desktop |
| **`minisign`** (test helper crate) | **0.9.1** | **no** (dev-dependency only; not in `cargo tree -p dayseam-desktop -e normal`) | n/a | Updater signature tests only — Desktop |
| **`tauri-plugin-updater`** | **2.10.1** | yes (dependency present; **MAS-3:** not registered when `--features mas`) | no (MAS-7a) | In-app updater inactive on MAS; policy surface is capability merge + JS gate — Desktop |
| **`tray-icon`** | **0.21.3** | yes | no (MAS-7a) | Status-item / menu bar — Desktop |

---

## 17. Dual-channel release, version skew, rollback

- **Same semver** on `master` for both channels; **GitHub tag** tracks direct channel artifact; **App Store Connect** tracks MAS binary after upload.
- **Skew:** direct users may run **`v0.13.N`** while MAS users remain on **`v0.13.(N−k)`** due to review lag — **expected**.
- **Backward compatibility window (`K`):** persisted SQLite schema + IPC must tolerate **at least `K = 3` patch releases** of skew (tune with product; never less than **2** without explicit decision). Migrations must **never strand** older MAS builds without a documented floor.
- **Rollback / incident:** direct channel may ship a **hotfix patch** ahead of MAS; support must acknowledge two channels. Phased release / manual “hold” on Connect before “Release to App Store” is operator procedure (**MAS-8** / **MAS-9** docs).

**MAS-8d** (automated upload) should use **`continue-on-error`** vs direct `release.yml` unless the team explicitly couples them (plan).

---

## 18. Single codebase exit criteria

| Allowed long-term | **Blocked** without removal issue + **MAS-9a** sign-off |
|-------------------|----------------------------------------------------------|
| Packaging-only cfg (`bundle`, signing, targets) | User-visible business rules duplicated in `#[cfg(feature = "mas")]` |
| Entitlement / plist / capability JSON differences | Scattershot `if (isMas)` in React for non-UX reasons |
| Compile-time `distribution_profile` enum for updater visibility | “MAS special case” connectors that diverge from direct for the same `SourceKind` |

---

## 19. Testing strategy

- **Unit / integration / Vitest** first; **Playwright** only for thin smoke where unavoidable (plan).
- **macOS GitHub Actions** is **authoritative** for bookmark + Keychain + codesign entitlements checks (**MAS-1b+**); Linux jobs remain compile-only for non-desktop crates.
- Do not weaken existing tests when adding MAS scaffolding — add **parallel** MAS-specific tests (**plan testing discipline**).

---

## 20. Open decisions checklist (pre–App Store submission)

- [x] **MAS bundle identifier (scaffold)** — `tauri.mas.conf.json` sets **`dev.dayseam.mas`** for merge builds (**MAS-1a**). Replace with the final App Store Connect bundle id when registered.
- [x] Confirm **`DATA_SUBDIR`** for the MAS profile in Rust (`startup.rs`) so Application Support / `state.db` paths do not collide with direct when both SKUs are installed (§10) — **MAS-5b1**.
- [x] **Keychain (MAS-5a):** audit documented in §12 — bundle-id isolation expected.
- [x] **Keychain service SKU prefix (MAS-5b2):** [`keychain_profile.rs`](../../apps/desktop/src-tauri/src/keychain_profile.rs) — `dayseam.mas.*` when built with **`mas`**.
- [ ] Confirm **JIT entitlement** narrative with legal/compliance if Apple pushes back.
- [ ] Confirm **network entitlement** shape for self-hosted connector domains.

---

## 21. Build profiles (**MAS-1a** + **MAS-1b** + **MAS-2a** + **MAS-2b** + **MAS-2c**)

| Profile | Command | Cargo features | Tauri config | Entitlements plist |
|---------|---------|----------------|--------------|-------------------|
| **Direct (default)** | `pnpm --filter @dayseam/desktop tauri build` | none (release) | [`tauri.conf.json`](../../apps/desktop/src-tauri/tauri.conf.json) only | [`entitlements.plist`](../../apps/desktop/src-tauri/entitlements.plist) |
| **MAS (sandbox plist)** | `pnpm --filter @dayseam/desktop tauri:build:mas` | `mas` | base `tauri.conf.json` merged with [`tauri.mas.conf.json`](../../apps/desktop/src-tauri/tauri.mas.conf.json) (overrides **`identifier`** to `dev.dayseam.mas` and **`bundle.macOS.entitlements`** to [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist)) | [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist) — **MAS-2a:** App Sandbox + **`network.client`** + user-selected + JIT-class keys; **MAS-2c:** JIT justification in [`docs/compliance/MAS-JIT-ENTITLEMENTS.md`](../compliance/MAS-JIT-ENTITLEMENTS.md) ([`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md)); direct [`entitlements.plist`](../../apps/desktop/src-tauri/entitlements.plist) stays **without** App Sandbox |

The desktop crate exposes [`DISTRIBUTION_PROFILE`](../../apps/desktop/src-tauri/src/lib.rs) (`"direct"` \| `"mas"`). **`distribution_profile`** IPC (**MAS-3b**) exposes it to the webview so updater UX gates without a second bundle.

CI (`desktop-bundle (direct + MAS)` + `shell-scripts` on macOS) runs [`verify-tauri-bundle-entitlements.sh`](../../scripts/ci/verify-tauri-bundle-entitlements.sh) and [`check-entitlements.sh`](../../scripts/ci/check-entitlements.sh) against both plists so merge regressions fail before release. Those bundle-only builds merge **`bundle.createUpdaterArtifacts: false`** so PR runners do not need **`TAURI_SIGNING_PRIVATE_KEY`** (release workflow still signs updater artifacts with the real secret).

**MAS-2b:** after the MAS bundle passes entitlement verification, CI runs [`mas-sandbox-launch-smoke.sh`](../../scripts/ci/mas-sandbox-launch-smoke.sh) against the signed **`Dayseam.app`** — the **real** `CFBundleExecutable` stays alive for a bounded interval so crashes during sandboxed bootstrap / WebView init fail the job (not a plist-only or stub-binary gate).

---

## Document history

| Date | Change |
|------|--------|
| 2026-04-30 | **MAS-0b:** initial full addendum (matrices, bookmarks, coexistence, subprocess baseline, skew, testing). |
| 2026-04-30 | **MAS-1a:** §21 build profiles + open-decisions checkbox for scaffold bundle id. |
| 2026-04-30 | **MAS-1b:** §21 entitlements column + CI script references. |
| 2026-04-30 | **MAS-2a:** §21 MAS row — App Sandbox + `network.client` in `entitlements.mas.plist`; verify script requires those keys on `mas` profile. |
| 2026-04-30 | **MAS-2a review:** §5 footnote — `user-selected.read-write` on at MAS-2a vs bookmark semantics in **MAS-4**. |
| 2026-04-30 | **MAS-2b:** §16 privacy/SDK inventory (versions + `PrivacyInfo.xcprivacy` gaps); §21 CI — [`mas-sandbox-launch-smoke.sh`](../../scripts/ci/mas-sandbox-launch-smoke.sh) on MAS bundle after codesign verification. |
| 2026-05-01 | **MAS-2c:** §5 JIT matrix row + §7 pointer to [`MAS-JIT-ENTITLEMENTS.md`](../compliance/MAS-JIT-ENTITLEMENTS.md); §21 MAS column cites compliance doc. |
| 2026-05-01 | **MAS-4a:** §9.6 **`security_scoped_bookmarks`** SQLite mapping + crate [`build.rs`](../../crates/dayseam-db/build.rs) rerun hints for migrations. |
| 2026-05-01 | **MAS-4b:** §9.5 pointer to desktop [`security_scoped`](../../apps/desktop/src-tauri/src/security_scoped/mod.rs) module. |
| 2026-05-01 | **MAS-4c:** §9.2 — IPC discovery vs connector `discover_repos`; [`local_git_scan`](../../apps/desktop/src-tauri/src/local_git_scan.rs) + [`SecurityScopedBookmarkRepo`](../../crates/dayseam-db/src/repos/security_scoped_bookmarks.rs). |
| 2026-05-01 | **MAS-5a:** §12 Keychain — App Sandbox + coexistence audit (service matrix, entitlements, **MAS-5b1** / **MAS-5b2** follow-ups); §20 — split **`DATA_SUBDIR`** vs Keychain audit checkbox. |
| 2026-05-01 | Plan: split **MAS-5b** into **MAS-5b1** / **MAS-5b2**; §12.7 table + §10 / §12.3 / §20 pointers updated. |
