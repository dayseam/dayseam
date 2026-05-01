# `entitlements.mas.plist` — Mac App Store SKU

**Do not merge prose into `entitlements.mas.plist`.** The same AMFI rule
as [`entitlements.md`](./entitlements.md) applies: `codesign` rejects XML
comments in entitlements plists.

## Purpose

This file is the **MAS merge-profile** entitlements source referenced
from [`tauri.mas.conf.json`](./tauri.mas.conf.json) (`bundle.macOS.entitlements`).
CI runs `codesign -d --entitlements` on a real `.app` built with the MAS
Tauri config merge ([`verify-tauri-bundle-entitlements.sh`](../../../scripts/ci/verify-tauri-bundle-entitlements.sh)).

## Evolution

| Phase | `entitlements.mas.plist` |
|-------|---------------------------|
| **MAS-1b** | Stub: mirrored direct WebView keys **without** App Sandbox (packaging + CI wiring only). |
| **MAS-2a (today)** | **`com.apple.security.app-sandbox`** + **`com.apple.security.network.client`** plus the same user-selected + JIT-class keys as direct until **MAS-2c** revisits JIT evidence. |
| **MAS-2c+** | May narrow JIT / executable-memory keys per App Review evidence — see [`docs/design/2026-phase-5-mas-architecture.md`](../../../docs/design/2026-phase-5-mas-architecture.md) §5–6. |

Rationale for retaining user-selected + JIT-style keys for now matches
[`entitlements.md`](./entitlements.md) (WebView / folder picker); sandbox
changes *how* paths are obtained (**MAS-4** bookmarks) without dropping
keys prematurely.
