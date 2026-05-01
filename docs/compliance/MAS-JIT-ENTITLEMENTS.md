# MAS JIT and executable-memory entitlements (**MAS-2c**)

Normative engineering record for the Mac App Store SKU. It satisfies the **MAS-2c** plan row (exact keys, platform scope, Apple-facing justification seed, fallback). **`MAS-7c`** (`MAS-APP-REVIEW-NOTES.md`) should copy or adapt the “App Review notes” section here when filing.

**Related:** [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md) (#210), [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) §5–§7, [`apps/desktop/src-tauri/entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist), [`apps/desktop/src-tauri/entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md).

---

## Platform and architecture scope

- **OS:** **macOS App Store bundle only** — this document does not describe Windows / Linux WebView stacks (other channels use different engines).
- **CPU:** **arm64** (Apple Silicon) and **x86_64** use the **same** entitlement keys below; there is no arch-specific plist fork today.
- **Runtime:** The main executable is hardened; **App Sandbox** is enabled for MAS (`com.apple.security.app-sandbox` in [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist)). JIT-class keys are **additive** on top of sandbox + network client + user-selected file access (**MAS-2a**).

---

## Entitlement keys (exact)

These keys are **`true`** in [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist) and are **asserted** on the signed bundle by [`scripts/ci/verify-tauri-bundle-entitlements.sh`](../../scripts/ci/verify-tauri-bundle-entitlements.sh) in **`mas`** mode (alongside sandbox, `network.client`, and `user-selected.read-write`).

| Key | Role |
|-----|------|
| `com.apple.security.cs.allow-jit` | Allows the hardened runtime / WebKit stack to use **JIT** pages where the system WebView implementation requires them for JavaScript execution and related runtime features. |
| `com.apple.security.cs.allow-unsigned-executable-memory` | Allows **writable-then-executable** mappings used by **WKWebView** (with `allow-jit`) and for **in-process native code** that maps executable pages (e.g. **libgit2** and other Rust/native deps — not a second arbitrary-code surface). |

**Direct SKU parity:** the non-sandbox [`entitlements.plist`](../../apps/desktop/src-tauri/entitlements.plist) already carries the **same two keys** so local Developer ID builds and MAS store builds stay aligned on WebView expectations until evidence says one key can be dropped.

---

## Why Dayseam needs them (engineering → App Review)

Dayseam is a **Tauri 2** desktop shell: the UI runs in **WKWebView** (via **WRY**). Apple’s WebKit stack historically relies on JIT or JIT-like memory permissions for performant JS and rendering. Without these entitlements, **cold boot or first paint can fault** under the hardened runtime—exactly what **MAS-2b** (`mas-sandbox-launch-smoke.sh`) guards in CI.

The app is **not** a generic code host: there is no `eval`-style user script surface beyond the shipped frontend bundle and Tauri’s controlled bridge.

---

## Evidence pointers (versions and inventory)

- **Pinned third-party versions** (Tauri, WRY, WebKit as system component, etc.) live in the **MAS-2b** table in [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) §16, sourced from `Cargo.lock` at the stated date—refresh that table when upgrading dependencies, then re-validate this document if WebKit/Tauri guidance changes.
- **Optional deep evidence** (binary / `nm` notes on which dylibs map executable pages) is **not** required for day-to-day engineering; collect only if App Review or compliance asks—track under **MAS-7c** if produced.

---

## Apple-facing notes (seed for **MAS-7c**)

> Dayseam is a local-first productivity app built with Tauri 2 and the system WKWebView. We request `com.apple.security.cs.allow-jit` and `com.apple.security.cs.allow-unsigned-executable-memory` so the WebView and in-process native libraries (e.g. libgit2) can use the same JIT / executable-memory allowances already present on our direct-distribution macOS build, under App Sandbox plus outbound HTTPS client access. We do not execute arbitrary user-supplied native code; the embedded web UI is shipped inside the app bundle. If Apple prefers a narrower configuration, we will follow WebKit/Tauri release guidance or reduce WebView features per our documented fallback plan.

---

## Fallback if App Review rejects

1. **Reduce WebView surface** — disable specific accelerated or experimental WebKit paths per Tauri / WebKit release notes, then re-run **MAS-2b** smoke and manual UI checks.
2. **Upstream** — file a minimal repro with **Tauri** / **WRY** and track a version bump that aligns with Apple’s guidance.
3. **Hard stop** — if neither is viable, **document the blocker** and hold the MAS SKU until resolved. **Do not** widen entitlements silently or add unrelated escape hatches.

---

## Maintenance

When **changing** `entitlements.mas.plist` JIT keys or CI expectations, update this file, [`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md), and [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) §5 / §7 in the same change set so the plan, plist prose, and compliance narrative stay aligned.
