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
| **MAS-2a (today)** | **`com.apple.security.app-sandbox`** + **`com.apple.security.network.client`** plus the same user-selected + JIT-class keys as direct [`entitlements.plist`](./entitlements.plist) (**MAS-2c** documents why those keys stay — next row). |
| **MAS-2c** | JIT + executable-memory keys **retained** with canonical justification, platform scope, App Review seed, and fallback ladder in [`docs/compliance/MAS-JIT-ENTITLEMENTS.md`](../../../docs/compliance/MAS-JIT-ENTITLEMENTS.md) (feeds **MAS-7c**). CI keys unchanged. |
| **MAS-2c+ / review** | May **narrow** JIT / executable-memory keys only with App Review or engineering evidence — see architecture §5–§7 and the compliance doc. |

Rationale for retaining user-selected + JIT-style keys for now matches
[`entitlements.md`](./entitlements.md) (WebView / folder picker); sandbox
changes *how* paths are obtained (**MAS-4** bookmarks) without dropping
keys prematurely.
