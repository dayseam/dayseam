#!/usr/bin/env bash
# build-mas-app.sh — **MAS-8b** disposable local helper: MAS merge-profile
# `Dayseam.app` + the same verify + sandbox smoke gates as `mas-package-verify.yml`.
#
# Replace or delete when **MAS-8d** lands — see `scripts/release/mas/README.md`.
#
# Usage:
#   build-mas-app.sh [--no-smoke]
#
# Environment:
#   REPO_ROOT — optional; defaults to monorepo root (three levels above this file).
#
# Exit codes:
#   0  success (prints absolute path to Dayseam.app on stdout), or **`--help`**.
#   1  unsupported OS, missing prerequisite, or a build/verify step failed.

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)}"
RUN_SMOKE=1

while [[ $# -gt 0 ]]; do
  case "$1" in
    --no-smoke) RUN_SMOKE=0 ;;
    -h | --help)
      echo "usage: build-mas-app.sh [--no-smoke]" >&2
      exit 0
      ;;
    *)
      echo "build-mas-app.sh: unknown option: $1" >&2
      exit 1
      ;;
  esac
  shift
done

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "build-mas-app.sh: macOS required (Tauri macOS bundle + plutil gates)." >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "build-mas-app.sh: jq is required to read target_directory from cargo metadata (same as scripts/release/build-dmg.sh). Install e.g. brew install jq" >&2
  exit 1
fi

cd "$REPO_ROOT"

export ENTITLEMENTS_FILE="${REPO_ROOT}/apps/desktop/src-tauri/entitlements.mas.plist"
bash "${REPO_ROOT}/scripts/ci/check-entitlements.sh"

echo "==> pnpm install (frozen lockfile)"
pnpm install --frozen-lockfile

echo "==> Vite build (@dayseam/desktop)"
pnpm --filter @dayseam/desktop build

echo "==> plutil -lint PrivacyInfo.xcprivacy (source)"
plutil -lint "${REPO_ROOT}/apps/desktop/src-tauri/PrivacyInfo.xcprivacy"

echo "==> Tauri bundle — MAS merge profile (app only, no updater artifacts)"
(
  cd "${REPO_ROOT}/apps/desktop"
  pnpm exec tauri build --bundles app \
    --config src-tauri/tauri.mas.conf.json \
    --config '{"bundle":{"createUpdaterArtifacts":false}}' \
    --features mas
)

TARGET_DIR="$(cargo metadata --format-version 1 --no-deps --manifest-path "${REPO_ROOT}/Cargo.toml" | jq -r '.target_directory')"
if [[ -z "$TARGET_DIR" || "$TARGET_DIR" == "null" ]]; then
  echo "build-mas-app.sh: cargo metadata did not return target_directory" >&2
  exit 1
fi

APP="${TARGET_DIR}/release/bundle/macos/Dayseam.app"
if [[ ! -d "$APP" ]]; then
  echo "build-mas-app.sh: expected bundle at ${APP}" >&2
  exit 1
fi

echo "==> verify-tauri-bundle-entitlements.sh (mas)"
bash "${REPO_ROOT}/scripts/ci/verify-tauri-bundle-entitlements.sh" "$APP" mas

echo "==> verify-bundle-privacy-manifest.sh"
bash "${REPO_ROOT}/scripts/ci/verify-bundle-privacy-manifest.sh" "$APP"

if [[ "$RUN_SMOKE" -eq 1 ]]; then
  echo "==> mas-sandbox-launch-smoke.sh"
  bash "${REPO_ROOT}/scripts/ci/mas-sandbox-launch-smoke.sh" "$APP" 22
else
  echo "==> skipping mas-sandbox-launch-smoke (--no-smoke)" >&2
fi

printf '%s\n' "$APP"
