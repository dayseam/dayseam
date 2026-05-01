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
| **MAS-6b** | **`com.apple.security.network.server`** — OAuth PKCE **loopback** `TcpListener` bind+accept on **`127.0.0.1`** ([`oauth.rs`](./src/ipc/oauth.rs)); production pins [**DAY-205**](./src/oauth_config.rs) port. Pairs with **`network.client`** for authorize + token **HTTPS**. |

## Outbound HTTPS (**MAS-6a**)

Connectors reach SaaS and self-hosted APIs over **HTTPS** using **`reqwest`** (TLS). Users configure **base URLs** (GitLab self-host, GitHub Enterprise Server, Atlassian cloud or DC, Microsoft Graph, …); hostnames are **not** baked into the app binary.

**Decision:** The Mac App Store SKU keeps the standard App Sandbox boolean entitlement **`com.apple.security.network.client`** in [`entitlements.mas.plist`](./entitlements.mas.plist). That is Apple’s documented pattern for **outbound client sockets** from sandboxed apps—**no** per-host or per-domain keys are required in the plist for typical HTTPS client traffic to user-chosen hosts.

**Trade-off:** This is a **broad outbound client** allowance. Narrowing to explicit hostname entries would force a plist change for every new enterprise endpoint class and does not match how Dayseam already lets users type arbitrary connector URLs.

**Verification:** CI runs [`verify-tauri-bundle-entitlements.sh`](../../../scripts/ci/verify-tauri-bundle-entitlements.sh) on a real **`Dayseam.app`** built with the MAS merge profile and asserts **`com.apple.security.network.client`** is embedded—see Phase 5 architecture §13.

## OAuth loopback inbound TCP (**MAS-6b**)

Apple’s App Sandbox treats **TCP `bind` + `accept`** as **incoming** connections. The Outlook PKCE flow listens on **`127.0.0.1`** for the browser redirect, then uses **`reqwest`** (still **`network.client`**) for the token exchange—see [`oauth.rs`](./src/ipc/oauth.rs) module docs and **DAY-205** fixed port in [`oauth_config.rs`](./src/oauth_config.rs).

**Decision:** [`entitlements.mas.plist`](./entitlements.mas.plist) sets **`com.apple.security.network.server`** to **`true`** alongside **`network.client`**. This is the standard Xcode “Incoming Connections (Server)” pairing for sandboxed apps that both **listen locally** and **call HTTPS APIs**; it is **not** a public WAN listener—production binds **`127.0.0.1`** to the **fixed** port constant from **DAY-205** ([`MICROSOFT_LOOPBACK_PORT`](./src/oauth_config.rs) in the IANA dynamic/private range), while integration tests still use **`127.0.0.1:0`** for parallelism.

**App Review framing:** OAuth loopback on localhost is a **documented RFC 8252** pattern; declare it in **MAS-7c** review notes with the subprocess table row (architecture §8).

**Verification:** [`verify-tauri-bundle-entitlements.sh`](../../../scripts/ci/verify-tauri-bundle-entitlements.sh) asserts **`network.server`** is embedded on signed **`mas`** bundles.

Rationale for retaining user-selected + JIT-style keys for now matches
[`entitlements.md`](./entitlements.md) (WebView / folder picker); sandbox
changes *how* paths are obtained (**MAS-4** bookmarks) without dropping
keys prematurely.
