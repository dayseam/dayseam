# Mac App Store — App Store Connect upload (**MAS-8d**)

This runbook describes the **TestFlight-first** automation wired in [`.github/workflows/mas-connect-upload.yml`](../../.github/workflows/mas-connect-upload.yml). It is **separate from** [`.github/workflows/release.yml`](../../.github/workflows/release.yml) (direct / GitHub Releases + Developer ID DMG) so Connect upload failures **never block** the direct channel.

**Export compliance:** answers filed in App Store Connect must stay aligned with [`docs/compliance/MAS-EXPORT-COMPLIANCE.md`](../compliance/MAS-EXPORT-COMPLIANCE.md) — the workflow passes **`uses-non-exempt-encryption: "false"`** to the uploader only when that remains accurate per that document.

## Repository secrets (GitHub Actions only)

Create an [App Store Connect API key](https://developer.apple.com/documentation/appstoreconnectapi/creating-api-keys-for-app-store-connect-api) with at least **App Manager** (or **Admin**). Store **only** in repo **Secrets** (never commit the `.p8` file):

| Secret | Contents |
|--------|-----------|
| **`DAYSEAM_ASC_ISSUER_ID`** | Issuer ID from App Store Connect → Users and Access → Integrations → **Keys** |
| **`DAYSEAM_ASC_KEY_ID`** | Key ID of the API key |
| **`DAYSEAM_ASC_PRIVATE_KEY`** | Full **`.p8`** PEM text (including `-----BEGIN/END PRIVATE KEY-----` lines) |

Optional: keep Issuer ID / Key ID as **Variables** instead if you prefer separating non-secret metadata — the workflow reads **secrets** above; adjust the workflow env wiring if you split.

## Inputs (`workflow_dispatch`)

| Input | Default | Meaning |
|-------|---------|---------|
| **`dry_run`** | `true` | When `true`, runs [`mas-connect-upload-preflight.sh`](../../scripts/release/mas/mas-connect-upload-preflight.sh) only (secret presence + doc link). No Transporter install, no upload. |
| **`mas_pkg_path`** | *(empty)* | Workspace-relative path to a **signed Mac App Store `.pkg`** (e.g. produced locally after store signing). Required when **`dry_run`** is `false` and you intend to upload. |

The job uses **`continue-on-error: true`** at the **job** level so a red upload attempt does not block unrelated automation. **GitHub still shows the workflow as green (checkmark) when the job “succeeds with errors”** — you must open the run, confirm each step, and look for `##[error]` / failed Transporter logs. Treat the badge as untrusted for this workflow; fix credentials or the package and re-run until every step is cleanly green if you believed the upload shipped.

## Transporter on GitHub-hosted macOS

Hosted runners do not ship Apple’s **Transporter** CLI. The workflow installs Apple’s official **iTMSTransporter** package from Apple’s download endpoint before calling [`apple-actions/upload-testflight-build`](https://github.com/apple-actions/upload-testflight-build) at commit **`994cd4f`** (**v4.1.0**) with **`backend: transporter`** and **`app-type: macos`** (`.pkg` uploads are **not** supported on the default App Store Connect API-only backend).

## Building the `.pkg` (today)

CI today produces a sandboxed **`Dayseam.app`** (see [`scripts/release/mas/build-mas-app.sh`](../../scripts/release/mas/build-mas-app.sh) and [`.github/workflows/mas-package-verify.yml`](../../.github/workflows/mas-package-verify.yml)). **Store-signed `.pkg` export** (provisioning profile + `productbuild` / Xcode Organizer flow) is **not** yet automated in this repository — track packaging + signing as **`MAS-8d-followup`** until the default `mas_pkg_path` can be filled from an artefact. Until then, operators build/sign the `.pkg` out-of-band, commit or attach nowhere (secrets stay in GHA), and pass **`mas_pkg_path`** relative to the checked-out tag.

## Manual smoke

1. Confirm secrets: run **`dry_run: true`** and read the preflight log (each `DAYSEAM_ASC_*` should report **set**).
2. Place a signed `.pkg` at a path inside a throwaway branch or use **`workflow_dispatch`** from a tag that contains the file if you are testing with a committed artefact (usually **avoid** committing `.pkg`; prefer uploading from a trusted local machine by pushing a tag is awkward — prefer **self-hosted** runner with the `.pkg` already on disk, or future artefact download step).
3. **`dry_run: false`** + **`mas_pkg_path`** → expect TestFlight processing in App Store Connect.

## Troubleshooting (TestFlight)

- **`ITMS-90886`** (“signature … missing an application identifier” while the embedded provisioning profile has one) — Apple’s [TN 733942](https://developer.apple.com/forums/thread/733942): **`com.apple.application-identifier`** / **`com.apple.developer.team-identifier`** are **restricted** and must be signed **only** into the **main** app, not into nested `.app` helpers. [`build-mas-store-pkg.sh`](../../scripts/release/mas/build-mas-store-pkg.sh) re-signs nested bundles with [`entitlements.mas.nested.plist`](../../apps/desktop/src-tauri/entitlements.mas.nested.plist) (sandbox + inherit), then re-signs the root with full [`entitlements.mas.plist`](../../apps/desktop/src-tauri/entitlements.mas.plist) plus an explicit `codesign --requirements` designated requirement ([tauri-apps/tauri#15230](https://github.com/tauri-apps/tauri/issues/15230)).

## Related

- Phase 5 plan: [`docs/plan/2026-phase-5-mas-app-store.md`](../plan/2026-phase-5-mas-app-store.md) — **MAS-8d**
- Architecture: [`docs/design/2026-phase-5-mas-architecture.md`](../design/2026-phase-5-mas-architecture.md) — §21 / **MAS-8d**
