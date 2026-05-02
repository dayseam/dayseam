# MAS local build helper (**MAS-8b**)

**Disposable scaffolding.** This directory exists so operators can reproduce the **Mac App Store merge profile** `Dayseam.app` on a developer machine with the **same post-build gates** as [`.github/workflows/mas-package-verify.yml`](../../../.github/workflows/mas-package-verify.yml) (**MAS-8a**), without waiting for CI.

## Lifecycle

**MAS-8d** [`mas-connect-upload.yml`](../../../.github/workflows/mas-connect-upload.yml) + [`MAS-CONNECT-UPLOAD.md`](../../../docs/release/MAS-CONNECT-UPLOAD.md) now cover **TestFlight / `.pkg` upload** (repo secrets only). **Replace or delete** this directory once that pipeline **subsumes** local **`Dayseam.app`** iteration (**store-signed `.pkg` in CI** is tracked as **`MAS-8d-followup`**). The Phase 5 plan requires explicit cleanup — do not let this rot as a second source of truth.

## Contents

| File | Role |
|------|------|
| [`build-mas-app.sh`](build-mas-app.sh) | `pnpm` install (if needed), Vite build, `tauri build` (**`mas`**), [`verify-tauri-bundle-entitlements.sh`](../../ci/verify-tauri-bundle-entitlements.sh) + [`verify-bundle-privacy-manifest.sh`](../../ci/verify-bundle-privacy-manifest.sh), optional [`mas-sandbox-launch-smoke.sh`](../../ci/mas-sandbox-launch-smoke.sh). |
| [`mas-connect-upload-preflight.sh`](mas-connect-upload-preflight.sh) | **MAS-8d** — GHA + local preflight for App Store Connect secrets / `.pkg` path (never prints secret values). |

## Prerequisites

- **macOS** (same constraints as Tauri bundling + `plutil`).
- **Node 20**, **pnpm**, **Rust** toolchain matching [`rust-toolchain.toml`](../../../rust-toolchain.toml).
- **`jq`** — used to parse `cargo metadata` for the workspace `target/` directory (same pattern as [`build-dmg.sh`](../build-dmg.sh)); install via Homebrew (`brew install jq`) or your package manager if it is not on `PATH`.
- Invoke from any directory (`./scripts/release/mas/build-mas-app.sh`, or a path to the script); it resolves the monorepo root from the script location (`REPO_ROOT` override optional).

## Related

- [`docs/plan/2026-phase-5-mas-app-store.md`](../../../docs/plan/2026-phase-5-mas-app-store.md) — Block **MAS-8**.
- [`docs/design/2026-phase-5-mas-architecture.md`](../../../docs/design/2026-phase-5-mas-architecture.md) — §21 build profiles.
- [`.github/workflows/mas-package-verify.yml`](../../../.github/workflows/mas-package-verify.yml) — **MAS-8c:** **`bash -n`** + mock **`semver:patch`** run of [`check-unreleased-for-semver-pr.sh`](../check-unreleased-for-semver-pr.sh) (DAY-195 / same **`extract-release-notes.sh`** gate as **`ci.yml`** **`changelog-semver-pr`**) before the bundle job; PRs also **`bash -n`** that script from **`shell-scripts`**.
- [`.github/workflows/mas-connect-upload.yml`](../../../.github/workflows/mas-connect-upload.yml) — **MAS-8d:** **`workflow_dispatch`** TestFlight upload for **macOS `.pkg`** (see [`MAS-CONNECT-UPLOAD.md`](../../../docs/release/MAS-CONNECT-UPLOAD.md)).
