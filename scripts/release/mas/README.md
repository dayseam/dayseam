# MAS local build helper (**MAS-8b**)

**Disposable scaffolding.** This directory exists so operators can reproduce the **Mac App Store merge profile** `Dayseam.app` on a developer machine with the **same post-build gates** as [`.github/workflows/mas-package-verify.yml`](../../../.github/workflows/mas-package-verify.yml) (**MAS-8a**), without waiting for CI.

## Lifecycle

When **MAS-8d** (automated App Store Connect upload) lands, **replace** this flow with the chosen upload/notarization pipeline or **delete** this directory if the workflow alone is sufficient. The Phase 5 plan requires that explicit cleanup — do not let this rot as a second source of truth.

## Contents

| File | Role |
|------|------|
| [`build-mas-app.sh`](build-mas-app.sh) | `pnpm` install (if needed), Vite build, `tauri build` (**`mas`**), [`verify-tauri-bundle-entitlements.sh`](../../ci/verify-tauri-bundle-entitlements.sh) + [`verify-bundle-privacy-manifest.sh`](../../ci/verify-bundle-privacy-manifest.sh), optional [`mas-sandbox-launch-smoke.sh`](../../ci/mas-sandbox-launch-smoke.sh). |

## Prerequisites

- **macOS** (same constraints as Tauri bundling + `plutil`).
- **Node 20**, **pnpm**, **Rust** toolchain matching [`rust-toolchain.toml`](../../../rust-toolchain.toml).
- **`jq`** — used to parse `cargo metadata` for the workspace `target/` directory (same pattern as [`build-dmg.sh`](../build-dmg.sh)); install via Homebrew (`brew install jq`) or your package manager if it is not on `PATH`.
- Invoke from any directory (`./scripts/release/mas/build-mas-app.sh`, or a path to the script); it resolves the monorepo root from the script location (`REPO_ROOT` override optional).

## Related

- [`docs/plan/2026-phase-5-mas-app-store.md`](../../../docs/plan/2026-phase-5-mas-app-store.md) — Block **MAS-8**.
- [`docs/design/2026-phase-5-mas-architecture.md`](../../../docs/design/2026-phase-5-mas-architecture.md) — §21 build profiles.
- [`.github/workflows/mas-package-verify.yml`](../../../.github/workflows/mas-package-verify.yml) — **MAS-8c:** **`bash -n`** + mock **`semver:patch`** run of [`check-unreleased-for-semver-pr.sh`](../check-unreleased-for-semver-pr.sh) (DAY-195 / same **`extract-release-notes.sh`** gate as **`ci.yml`** **`changelog-semver-pr`**) before the bundle job; PRs also **`bash -n`** that script from **`shell-scripts`**.
