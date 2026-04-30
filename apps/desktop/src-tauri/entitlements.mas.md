# `entitlements.mas.plist` — Mac App Store SKU (stub)

**Do not merge prose into `entitlements.mas.plist`.** The same AMFI rule
as [`entitlements.md`](./entitlements.md) applies: `codesign` rejects XML
comments in entitlements plists.

## Purpose (**MAS-1b**)

This file is the **MAS merge-profile** entitlements source referenced
from [`tauri.mas.conf.json`](./tauri.mas.conf.json) (`bundle.macOS.entitlements`).
It exists so CI can run `codesign -d --entitlements` on a real `.app`
built with the MAS Tauri config merge **before** App Sandbox keys land
in **MAS-2a**.

## Stub vs target state

| Phase | `entitlements.mas.plist` |
|-------|---------------------------|
| **MAS-1b (today)** | Mirrors the direct [`entitlements.plist`](./entitlements.plist) keys (no `com.apple.security.app-sandbox` yet) so packaging + CI gates are wired. |
| **MAS-2a+** | Adds **`com.apple.security.app-sandbox`** and the network / bookmark entitlements required by the addendum — see [`docs/design/2026-phase-5-mas-architecture.md`](../../../docs/design/2026-phase-5-mas-architecture.md). |

Rationale for the direct keys in the stub matches [`entitlements.md`](./entitlements.md)
(WebView / user-selected persistence) until sandbox policy replaces or
narrows them under review.
