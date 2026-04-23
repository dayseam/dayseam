# macOS Gatekeeper caveat — unsigned auto-updates (v0.6.0+)

**Status:** Known limitation accepted for the current single-user
blast radius. Tracked in [#59](https://github.com/vedanthvdev/dayseam/issues/59)
(Apple Developer ID codesigning + notarization).

## What this covers

DAY-108 shipped the Tauri v2 auto-updater, which verifies every
downloaded `.app.tar.gz` against the Ed25519 public key embedded in
`apps/desktop/src-tauri/tauri.conf.json` and swaps the `.app` bundle
in place on success. That key is a **publishing** signature
(minisign) — it proves the archive came from whoever holds the
private key in the `TAURI_SIGNING_PRIVATE_KEY` GitHub Actions
secret. It is **not** an Apple Developer ID codesign, and the DMGs
the release workflow publishes today are not notarized.

This document names every failure mode that falls out of that
gap, what the user sees, and what a future codesign PR would flip
from "accepted for now" to "resolved".

## What works today

- The Tauri updater plugin fetches `latest.json` from
  `https://github.com/vedanthvdev/dayseam/releases/latest/download/latest.json`,
  pulls the signed `.app.tar.gz`, verifies the minisign signature
  against the bundled pubkey, and replaces the running `.app` in
  place.
- The replacement bundle has the **same ad-hoc signature** as the
  running one (because `tauri build` ad-hoc signs every bundle it
  emits). Apple's Gatekeeper recognises the signature is unchanged
  across the swap, so in the common case the relaunch Just Works.
- The user sees: banner → "Install and restart" → short progress
  → app relaunches into the new version. No extra prompts.

## When it does NOT Just Work (the caveat)

An ad-hoc signature is tied to a locally-generated identity that
is not backed by an Apple-trusted certificate. Two known edge
cases can surface a Gatekeeper prompt or an outright refusal:

1. **First launch after a fresh DMG install** — unchanged from
   pre-v0.6.0: the user has to right-click → Open the first time
   to acknowledge the unsigned bundle. Subsequent launches (and
   subsequent updater-installed versions) are silent because
   Gatekeeper remembers the approval by code-directory hash,
   which the auto-update preserves as long as the ad-hoc identity
   is stable.

2. **Gatekeeper re-prompt after auto-update** — if macOS sees the
   post-update `.app` as a "new" app (different code-directory
   than the previous launch recorded), the user will see a
   Gatekeeper confirmation on the next launch. This is most
   likely when:

   - The user's macOS quarantine extended-attribute was cleared
     between launches (unusual — requires active `xattr` use or
     a Migration Assistant copy between machines).
   - A future `tauri build` upgrade changes the ad-hoc identity
     embedding in a way that breaks the code-directory stability
     contract (has happened once in Tauri's history; the
     `minimumSystemVersion = "13.0"` floor in `tauri.conf.json`
     is why we're not worse off).

   In both cases the user-visible outcome is the same: a single
   "Dayseam wants to run" prompt on the first relaunch after an
   auto-update. Accepting it once is sufficient.

## Why we accepted this for v0.6.0

Current user population is 1 (dogfood). The cost of a one-time
Gatekeeper prompt is a click; the cost of setting up Apple
Developer ID + notarization is ~$99/year plus a non-trivial CI
refactor to add the signing keychain and notarytool submission
steps. Deferred to [#59](https://github.com/vedanthvdev/dayseam/issues/59)
/ [#108](https://github.com/vedanthvdev/dayseam/issues/108).

## What the v0.6.1 smoke-test (DAY-115) should verify

When the next release cuts and an installed v0.6.0 client
auto-upgrades to v0.6.1 on the dogfood Mac, record the observed
Gatekeeper behaviour in the capstone PR body:

- ✅ Relaunch was silent — caveat did not trigger, no action needed.
- ⚠️  Single Gatekeeper prompt on relaunch — expected; note which
    macOS version the prompt appeared on so future reports have
    a baseline.
- ❌ macOS refused to launch the new app — escalate to
    [#59](https://github.com/vedanthvdev/dayseam/issues/59) and
    roll back the release.

Only the third outcome blocks shipping; the first two are
acceptable under the current trade-off.

## Related

- `apps/desktop/src-tauri/tauri.conf.json` — `plugins.updater`
  stanza with public key and endpoint URL.
- `apps/desktop/src-tauri/capabilities/updater.json` — narrow
  permission grant for the updater plugin.
- `apps/desktop/src/features/updater/useUpdater.ts` — the React
  binding that drives check/install/relaunch.
- `scripts/release/generate-latest-json.sh` — manifest generator
  invoked from `.github/workflows/release.yml`.
