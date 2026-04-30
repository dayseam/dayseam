# Phase 5 (MAS): Architecture addendum

> **Status:** stub until **MAS-0b** lands. Full technical design is **normative** in [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md); this file is the **checklist-shaped** addendum produced by task **[MAS-0b](../plan/2026-phase-5-mas-app-store.md#mas-block-0)**.

## Sections (outline — expand each in MAS-0b PR)

1. **Goals & non-goals** — dual SKU; direct-download remains primary until explicitly otherwise.
2. **Threat model delta** — App Sandbox vs current [`entitlements.md`](../../apps/desktop/src-tauri/entitlements.md) stance.
3. **Packaging vs entitlement vs runtime vs UX** — four columns so “required by Apple” cannot smuggle behavioural `if mas` without landing in the right row.
4. **Entitlement matrix** — `entitlements.plist` (direct) vs `entitlements.mas.plist` (MAS); **deny-list** for MAS.
5. **Tauri capability matrix** — default JSON vs MAS JSON; rationale per capability; updater excluded on MAS; nothing broadened “by accident.”
6. **JIT / executable memory** — exact keys; OS/arch scope; citations (Tauri / WebKit / Apple); **fallback** if App Review rejects.
7. **Subprocesses & helper binaries** — enumerate every spawn and bundled binary; sandbox legality; signing / entitlement inheritance (**feeds MAS-9a**).
8. **Filesystem** — security-scoped bookmarks: **granularity** (dir vs file for scan roots vs sinks; “save as new file” in permitted folder); **descendants** after restart; **symlink** policy (canonicalize vs reject); **stale bookmark** / rename-move recovery (**MAS-4f**); **RAII start/stop** lifetime (no session-wide retention unless justified).
9. **Direct ↔ MAS coexistence** — matrix: bundle ID, app support / container, SQLite DB, lock files, Keychain service names, URL schemes / deep links; **concurrent install** supported / unsupported / blocked; corruption risk if misconfigured.
10. **Migration (direct → MAS)** — what config survives; what breaks until re-consent; updater prefs; OAuth / browser state differences under sandbox.
11. **Keychain** — service identifiers, access groups, sandbox behaviour (**ordering vs OAuth** per plan dependency graph).
12. **Networking** — outbound client; connector endpoints.
13. **OAuth** — loopback redirect constraints under sandbox; collision with second installed SKU.
14. **Updater** — absent on MAS SKU; App Store updates only.
15. **Privacy & third-party SDKs** — inventory table (**MAS-2b** output) mapping SDK → ships manifest? → gap → owner (**feeds MAS-7a**).
16. **Dual-channel release & version skew** — same semver source of truth; review lag (**N** vs **N−k**); backward-compat window **K** for persisted data; rollback / phased release / direct-ahead hotfix policy.
17. **Single codebase exit criteria** — allowed long-term `cfg` / packaging deltas vs **blocked** user-visible `mas` branches without removal issue + **MAS-9a** sign-off.
18. **Testing strategy** — prefer **`cargo test`** / integration / Vitest; Playwright thin smoke only; **macOS CI** as authoritative for bookmark / Keychain tests.
