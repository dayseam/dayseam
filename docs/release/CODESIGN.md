# macOS Code Signing — Live Runbook

**Status:** Infrastructure implemented in DAY-124 (v0.6.7). The
release workflow auto-activates Developer ID signing + notarization
when the `APPLE_*` secrets are set on the repo, and falls back to
ad-hoc unsigned-friendly output when they are not. This document
is the source of truth for:

- The secrets the `developer-id` path expects, what format each
  takes, and where each comes from.
- How to verify a given release ran in the right mode after the
  fact.
- How to rotate a cert, notarytool password, or Team ID when they
  expire or change.
- The failure modes you will actually see in the wild, with the
  fix for each.

If you are setting this up for the first time, start at
[First-time setup](#first-time-setup). If you are responding to a
red release workflow, jump straight to
[Troubleshooting](#troubleshooting).

## Two modes, one workflow

The `release.yml` workflow resolves the signing mode from a single
signal: the presence of the `APPLE_CERTIFICATE` repo secret.

| Mode | Trigger | Output | Gatekeeper | Audience |
|---|---|---|---|---|
| `ad-hoc` | `APPLE_CERTIFICATE` secret **unset** | `.dmg` signed with `codesign -s -` (cdhash-based DR, no notarization) | Rejected on first launch; documented bypass in [`UNSIGNED-FIRST-RUN.md`](./UNSIGNED-FIRST-RUN.md) | Forks, contributors without Apple creds |
| `developer-id` | `APPLE_CERTIFICATE` secret **set** | `.dmg` signed with Developer ID Application cert + hardened runtime + notarized + stapled ticket | Accepted; double-click runs | Real users downloading from GitHub Releases |

Fallback is automatic and silent — removing the secrets produces
an ad-hoc build on the next release run, no workflow edit needed.
Adding them back flips the next run to developer-id.

## First-time setup

### 1. Apple Developer Program enrollment

Active Apple Developer Program membership on the Apple ID that
will own the releases. Individual enrollment is enough; org
enrollment is only required if the end-user-facing
`Developer ID Application: <Legal Name>` string should say a
company name instead of a person's name.

Record the **Team ID** (10-character alphanumeric, shown on the
Account → Membership page). You will paste this into the
`APPLE_TEAM_ID` secret below.

### 2. Developer ID Application certificate

From any Mac with Xcode installed:

1. Open **Xcode → Settings → Accounts**, add your Apple ID, select
   the team, click **Manage Certificates…**
2. Click **+** → **Developer ID Application**. Xcode generates
   the private key in your login keychain and registers the cert
   with Apple.
3. Open **Keychain Access**, filter to `My Certificates`, find the
   `Developer ID Application: <Your Name> (<TeamID>)` entry,
   right-click → **Export "Developer ID Application…"**. Save as
   `.p12` with a strong passphrase.

Verify it is usable for codesigning before you leave the Mac:

```sh
security find-identity -v -p codesigning
# should show at least one line like:
#   1) ABCD1234EF... "Developer ID Application: Your Name (ABCD123456)"
```

The full quoted string — including the `Developer ID Application:`
prefix and the `(TeamID)` suffix — is the `APPLE_SIGNING_IDENTITY`
value below.

Base64-encode the `.p12` for the GitHub secret:

```sh
openssl base64 -A -in dayseam-dev-id.p12 -out dayseam-dev-id.p12.b64
pbcopy < dayseam-dev-id.p12.b64
```

### 3. App-Specific Password for notarization

1. Go to <https://appleid.apple.com> → **Sign-In and Security** →
   **App-Specific Passwords** → **+**.
2. Label it something recognisable, e.g. `Dayseam notarytool (CI)`.
3. Copy the generated password immediately; Apple never shows it
   again. This is `APPLE_PASSWORD` below.

### 4. Configure GitHub Actions secrets

Repo **Settings → Secrets and variables → Actions**, add the
following six secrets (names are exact; the workflow reads them
by name):

| Secret | Value |
|---|---|
| `APPLE_CERTIFICATE` | base64 string from step 2 (the full `.p12.b64` file contents, single line, no header/footer) |
| `APPLE_CERTIFICATE_PASSWORD` | the `.p12` passphrase you set when exporting |
| `APPLE_SIGNING_IDENTITY` | full quoted cert common name, e.g. `Developer ID Application: Your Name (ABCD123456)` |
| `APPLE_ID` | Apple ID email the membership is on |
| `APPLE_PASSWORD` | app-specific password from step 3 |
| `APPLE_TEAM_ID` | 10-character Team ID from step 1 |

The `Resolve Apple codesign mode` step in `release.yml` treats
`APPLE_CERTIFICATE` as the activation signal; the next step
(`Preflight — assert Apple codesign secrets are complete`) then
fails the run fast if any of the other five is missing or
mal-formatted. Half-configured secret sets never reach the 4-minute
universal cargo build.

### 5. Dry-run the first signed release

Before merging a release PR that will hit real users:

```sh
gh workflow run release.yml --ref <your-branch> \
  -f level=patch -f dry_run=true
```

The `dry_run=true` path runs the full build, signing, and
notarization but skips the tag push and GitHub Release upload.
Watch for:

- **Signature mode parity** step: must print `Developer ID signature verified`.
- **Gatekeeper verdict** step: must print `Gatekeeper verdict: accepted`.

If either fails, see [Troubleshooting](#troubleshooting) before
merging a real release.

## Verifying a production release

On any Mac, download the `.dmg` from the GitHub Release and run:

```sh
# 1. Confirm the signature is a real Developer ID sig (not ad-hoc).
codesign -dvv /Volumes/Dayseam/Dayseam.app 2>&1 | grep -E 'Authority|flags'
# Expected:
#   Authority=Developer ID Application: <Your Name> (TEAMID)
#   Authority=Developer ID Certification Authority
#   Authority=Apple Root CA
#   flags=0x10000(runtime)
# NOT: flags=0x20002(adhoc,linker-signed)

# 2. Confirm Gatekeeper accepts (this is what an end-user's Mac does).
spctl --assess --type open --context context:primary-signature --verbose Dayseam-v*.dmg
# Expected: Dayseam-v0.6.7.dmg: accepted
#           source=Notarized Developer ID

# 3. Confirm the stapled ticket survived the download.
xcrun stapler validate Dayseam-v*.dmg
# Expected: The validate action worked!
```

A production release that fails any of these should be yanked
(`gh release delete v<x.y.z>`) and re-cut after fixing the root
cause — shipping a supposedly-notarized bundle that Gatekeeper
rejects is a larger user-facing regression than an ad-hoc bundle
(users no longer expect the `UNSIGNED-FIRST-RUN` workaround).

## Rotation

### Cert renewal (annual)

Developer ID Application certs last 5 years but the Apple Developer
Program membership under them renews annually. On membership
renewal:

1. Regenerate the cert in Xcode (step 2 of first-time setup). The
   old cert keeps working until it expires; there's no forced-migration
   window.
2. Re-export the `.p12`, re-base64, update the `APPLE_CERTIFICATE`
   and `APPLE_CERTIFICATE_PASSWORD` secrets. `APPLE_SIGNING_IDENTITY`
   usually does not change (same CN format).
3. Run a dry-run release to confirm the new cert signs correctly.

If you let the cert expire before rotating: no catastrophe. The next
release falls back to ad-hoc mode automatically (`APPLE_CERTIFICATE`
still points at an expired cert, Tauri's `codesign` call fails mid-
build and the workflow red-lights). Update the secret, re-run.

Already-shipped notarized builds are **not** invalidated by cert
renewal — the notarization ticket Apple issued stays valid for the
life of that specific artefact. Only *new* releases need the new
cert.

### App-specific password rotation

Best practice is annual, or immediately if leaked/lost. Generate a
new one at <https://appleid.apple.com>, update `APPLE_PASSWORD`, and
revoke the old one. Stapled past releases are unaffected.

### Team ID change

Only happens if you migrate from individual to org enrollment (or
vice versa). The Team ID change flips the `APPLE_SIGNING_IDENTITY`
CN string too, so update both. The first release after the change
will have a *different* Developer ID cdhash and may re-prompt the
Keychain ACL once on upgrade — this is macOS behaviour, not a
regression.

## Troubleshooting

### `::error::APPLE_CERTIFICATE is set (activating developer-id mode) but these companion secrets are missing`

Half-configured secret set. The preflight deliberately refuses to
start a signed build unless all six secrets are present — see the
[First-time setup](#first-time-setup) § 4 table for the full list.
Set the listed missing secrets and re-run the workflow.

### `::error::APPLE_SIGNING_IDENTITY does not start with 'Developer ID Application:'`

Common mistake: copying just the `TeamID` hash or the short name.
The secret must be the **full quoted string** from
`security find-identity -v -p codesigning`, e.g.
`Developer ID Application: Your Name (ABCD123456)`. No leading or
trailing whitespace, no surrounding quotes.

### `::error::APPLE_TEAM_ID '…' is not a 10-character alphanumeric Team ID`

Find the correct value at
<https://developer.apple.com/account> → **Membership details** →
**Team ID**. It's always 10 characters, uppercase letters + digits.
Do not confuse with the Apple ID email or the subscription
reference number.

### `::error::developer-id mode was active but the bundle shipped with an ad-hoc signature`

Tauri did not honour the `APPLE_SIGNING_IDENTITY` env var. Two
causes seen in the wild:

1. **Tauri CLI upgrade** that changed env-var precedence. Check
   the Tauri release notes for the version bumped in
   `apps/desktop/package.json`; if the precedence inverted, we
   need to patch `tauri.conf.json`'s `signingIdentity` dynamically
   in the workflow (jq replace) before the build. Convert the env-
   var path into a config-patch path and file a bug upstream.
2. **The env var is unexpectedly empty at build time.** Add an
   `echo "SIGNING_IDENTITY=$APPLE_SIGNING_IDENTITY"` line in the
   build step (without printing the value to logs — GitHub
   auto-redacts secrets, but mask-handling has edge cases).
   Usually traces back to the secret not being defined in the
   repo environment the workflow is using.

### `::error::Gatekeeper rejected the signed .dmg`

Notarization ticket is missing or the signature is invalid. In
order of likelihood:

1. **Notarization failed mid-build but Tauri swallowed the error.**
   Check the `Build universal .dmg` step log for any
   `notarytool` / `stapler` output. If you see an `Invalid` reply
   from Apple, they've flagged something in the bundle — most
   commonly a binary without hardened runtime or a non-signed
   embedded framework. File an issue and attach the log.
2. **App-specific password rotated out of band.** `APPLE_PASSWORD`
   must match the current password on
   <https://appleid.apple.com>; rotating at Apple does not
   auto-update the secret. Rotate per § [Rotation](#rotation).
3. **Apple's notarization service is having an outage.** Check
   <https://developer.apple.com/system-status/>. Wait, re-run.

### Expired or revoked cert mid-release

Symptom: `Build universal .dmg` step fails with
`errSecInternalComponent` or `CSSMERR_TP_CERT_EXPIRED`. Rotate
per § [Rotation](#rotation).

### `::error::Failed to parse .p12 certificate` / `Mac verify error`

Surfaced by the `Preflight — validate .p12 certificate payload`
step before the build starts. Either:

- **`APPLE_CERTIFICATE` is not valid base64.** Re-encode with
  `openssl base64 -A -in cert.p12` (the `-A` flag collapses the
  output to a single line, which is what the GitHub secret store
  expects).
- **`APPLE_CERTIFICATE_PASSWORD` does not match the .p12.** The
  passphrase is what you set when you clicked "Export Developer
  ID Application…" in Keychain Access, not your Apple ID password
  or your macOS login password. Re-export if you've forgotten it.

### `codesign: ambiguous` inside `tauri build`

Symptom: Tauri's build step fails with `Developer ID Application:
<User> (<Team ID>): ambiguous (matches … and … in …)`. Cause: two
keychains on the runner each contain an identity with the same
CN, and codesign can't pick one.

Dayseam's workflow is deliberately structured to avoid this — we
let Tauri's bundler do the only keychain import (it creates its
own `tauri-build.keychain` internally). If this error surfaces
anyway, check whether a runner-image update started pre-installing
a Developer ID cert in `login.keychain-db`; if so, Tauri's import
of the same cert into `tauri-build.keychain` creates the
ambiguity. Workaround: add a pre-build step that deletes the cert
from `login.keychain-db` before Tauri imports.

## Related docs

- [`UNSIGNED-FIRST-RUN.md`](./UNSIGNED-FIRST-RUN.md) — the
  end-user-facing Gatekeeper-bypass guide that applies to
  `ad-hoc` mode releases. Becomes historically-interesting-only
  once all shipped releases are in `developer-id` mode.
- [`PHASE-3-5-CODESIGN.md`](./PHASE-3-5-CODESIGN.md) — historical
  spec for the work DAY-124 implemented. Left in place as a
  pointer with links back here.
- [`../../entitlements.md`](../../apps/desktop/src-tauri/entitlements.md)
  — rationale for each key in `entitlements.plist`. Every
  entitlement we set is compatible with notarization's hardened-
  runtime requirements.
