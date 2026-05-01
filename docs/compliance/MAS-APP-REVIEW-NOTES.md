# Mac App Store — App Review notes (**MAS-7c**)

Paste-ready (or lightly edited) material for **App Store Connect** → **App Review Information** and for resolution-center replies. It consolidates pointers required by the **MAS-7c** plan row: **local-first** positioning, **sandbox** behaviour, **JIT** justification (with engineering detail delegated), **subprocess** baseline, plus cross-links to **privacy**, **export**, and **entitlements** prose.

**Do not** fork the numbered **JIT fallback** ladder — that stays only in [`MAS-JIT-ENTITLEMENTS.md`](MAS-JIT-ENTITLEMENTS.md) so engineering and App Review stay aligned.

**Related:** [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md) (**#210**), [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md), [`MAS-JIT-ENTITLEMENTS.md`](MAS-JIT-ENTITLEMENTS.md), [`MAS-EXPORT-COMPLIANCE.md`](MAS-EXPORT-COMPLIANCE.md), [`apps/desktop/src-tauri/entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md).

---

## How to use this file

1. Prefer **short answers** in Connect; paste **subsections** only when the reviewer asks for detail.
2. Before submission, confirm **pinned dependency versions** in architecture **§16** still match `Cargo.lock` — update that table in-repo first, then refresh any version numbers you quote manually.
3. If Apple challenges **JIT**, follow the **fallback ladder** in [`MAS-JIT-ENTITLEMENTS.md`](MAS-JIT-ENTITLEMENTS.md) and update **both** that file and the **JIT blockquote** in this file in the same change set.

---

## Product summary (local-first)

Dayseam is a **local-first** macOS productivity app: it aggregates evidence from **user-configured** sources (Local Git folders, GitLab, GitHub, Jira, Confluence, Outlook) into an **editable report** the user keeps on disk (Markdown / Obsidian-style vaults). There is **no** Dayseam-operated cloud service; outbound HTTPS goes only to **hosts the user configures**. Secrets (PATs, OAuth tokens) are stored in the **macOS Keychain**. The Mac App Store build uses **App Sandbox** and receives updates **only through the App Store** (no in-app updater).

---

## Sandboxing, filesystem, and networking

- **App Sandbox** is enabled (`com.apple.security.app-sandbox` in [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist)).
- **Filesystem:** access to user-chosen scan roots and sink folders uses **security-scoped bookmarks** persisted in app-local SQLite — see architecture **§9** and [`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md).
- **Outbound HTTPS** uses **`com.apple.security.network.client`** to user-configured SaaS / self-hosted hosts (**MAS-6a**).
- **OAuth (e.g. Outlook)** uses a **localhost loopback** listener for the authorization redirect; the MAS plist includes **`com.apple.security.network.server`** for that **bind/accept** path plus **`network.client`** for IdP HTTPS (**MAS-6b**, [`entitlements.mas.md`](../../apps/desktop/src-tauri/entitlements.mas.md) **OAuth loopback**).
- **In-app updater** and **process relaunch** plugins are **not** registered on the **`mas`** feature build — updates are **App Store–only** (**MAS-3**, architecture **§15**).

---

## JIT and executable memory (WKWebView + native deps)

Engineering rationale, entitlement keys, and the **numbered fallback ladder** live in [`MAS-JIT-ENTITLEMENTS.md`](MAS-JIT-ENTITLEMENTS.md). For App Review paste convenience, the **Apple-facing seed** from that file is reproduced in this section (keep **verbatim** unless legal/engineering jointly edits the master in **MAS-JIT**):

> Dayseam is a local-first productivity app built with Tauri 2 and the system WKWebView. We request `com.apple.security.cs.allow-jit` and `com.apple.security.cs.allow-unsigned-executable-memory` so the WebView and in-process native libraries (e.g. libgit2) can use the same JIT / executable-memory allowances already present on our direct-distribution macOS build, under App Sandbox plus outbound HTTPS client access. We do not execute arbitrary user-supplied native code; the embedded web UI is shipped inside the app bundle. If Apple prefers a narrower configuration, we will follow WebKit/Tauri release guidance or reduce WebView features per our documented fallback plan.

---

## Subprocesses and helper binaries

Authoritative **enumeration baseline** (what can spawn, what is in-process only, sandbox expectations): architecture **§8** — [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) *Subprocesses and helper binaries*. Highlights for reviewers: **no `git` CLI** in production (libgit2 in-process); **`open` / browser** hand-offs are **user-driven** for links and OAuth; loopback listener is **OAuth only**.

---

## Privacy manifest and third-party SDKs

The app bundle includes **`PrivacyInfo.xcprivacy`** under **`Contents/Resources`** (required-reason API declarations; **`NSPrivacyTracking`** false). Pinned SDK / crate context: architecture **§16** and [`PrivacyInfo.xcprivacy`](../../apps/desktop/src-tauri/PrivacyInfo.xcprivacy).

---

## Export compliance

Encryption / export assumptions and App Store Connect answers: [`MAS-EXPORT-COMPLIANCE.md`](MAS-EXPORT-COMPLIANCE.md) (**MAS-7b**).

---

## Maintenance

When **changing** sandbox narrative, OAuth loopback, JIT wording, subprocess inventory, or privacy/export cross-links, update this file together with [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) **§20** checklist and the relevant compliance doc so the paste pack does not drift.
