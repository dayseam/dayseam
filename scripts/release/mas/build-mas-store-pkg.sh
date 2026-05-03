#!/usr/bin/env bash
# build-mas-store-pkg.sh — Mac App Store **distribution-signed** `.app` + signed `.pkg`
#
# Implements the Tauri App Store flow (provision profile embed → `tauri build`
# with Apple Distribution signing → `productbuild` with a Mac **installer**
# identity). See docs/release/MAS-STORE-PKG.md for secrets and GitHub wiring.
#
# Usage:
#   build-mas-store-pkg.sh [version]
#
# Environment (CI + local; never commit values):
#   DAYSEAM_MAS_APP_CERTIFICATE              — base64 `.p12` (**Apple Distribution**)
#   DAYSEAM_MAS_APP_CERTIFICATE_PASSWORD     — `.p12` passphrase
#   DAYSEAM_MAS_APP_SIGNING_IDENTITY         — full CN, e.g. "Apple Distribution: Name (TEAMID)"
#   DAYSEAM_MAS_INSTALLER_CERTIFICATE        — base64 `.p12` (**Mac Installer** / App Store installer)
#   DAYSEAM_MAS_INSTALLER_CERTIFICATE_PASSWORD
#   DAYSEAM_MAS_INSTALLER_SIGNING_IDENTITY   — full CN for productbuild --sign
#   DAYSEAM_MAS_PROVISIONING_PROFILE_BASE64  — base64 of `*.mobileprovision` (Mac App Store Connect profile)
#
# Optional:
#   REPO_ROOT — monorepo root (defaults from script location)
#
# Exit codes: 0 success (prints final `.pkg` path on last line of stdout), 1 failure, 2 bad args.

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)}"
VERSION_FILE="${REPO_ROOT}/VERSION"

if [[ $# -gt 1 ]]; then
  echo "usage: build-mas-store-pkg.sh [version]" >&2
  exit 2
fi

if [[ $# -eq 1 ]]; then
  VERSION="$1"
else
  VERSION="$(tr -d '[:space:]' <"$VERSION_FILE")"
fi

if [[ ! "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "build-mas-store-pkg.sh: version '$VERSION' is not a valid semver triple" >&2
  exit 2
fi

require_env() {
  local n="$1"
  if [[ -z "${!n:-}" ]]; then
    echo "build-mas-store-pkg.sh: missing required environment variable: $n (see docs/release/MAS-STORE-PKG.md)" >&2
    exit 1
  fi
}

for v in DAYSEAM_MAS_APP_CERTIFICATE DAYSEAM_MAS_APP_CERTIFICATE_PASSWORD DAYSEAM_MAS_APP_SIGNING_IDENTITY \
  DAYSEAM_MAS_INSTALLER_CERTIFICATE DAYSEAM_MAS_INSTALLER_CERTIFICATE_PASSWORD DAYSEAM_MAS_INSTALLER_SIGNING_IDENTITY \
  DAYSEAM_MAS_PROVISIONING_PROFILE_BASE64; do
  require_env "$v"
done

PROFILE_PATH="${REPO_ROOT}/apps/desktop/src-tauri/MasStore.provisionprofile"
KEYCHAIN_PATH="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/mas-store-build.keychain-db"
KEYCHAIN_PASSWORD="$(openssl rand -base64 32)"
INST_P12="${RUNNER_TEMP:-${TMPDIR:-/tmp}}/mas-installer-$$.p12"

cleanup() {
  rm -f "$PROFILE_PATH" "$INST_P12" 2>/dev/null || true
  if [[ -f "$KEYCHAIN_PATH" ]]; then
    security delete-keychain "$KEYCHAIN_PATH" 2>/dev/null || true
  fi
}
trap cleanup EXIT

echo "==> Writing embedded provisioning profile"
if ! echo "$DAYSEAM_MAS_PROVISIONING_PROFILE_BASE64" | base64 --decode >"$PROFILE_PATH" 2>/dev/null; then
  echo "build-mas-store-pkg.sh: DAYSEAM_MAS_PROVISIONING_PROFILE_BASE64 is not valid base64" >&2
  exit 1
fi
if [[ ! -s "$PROFILE_PATH" ]]; then
  echo "build-mas-store-pkg.sh: decoded provisioning profile is empty" >&2
  exit 1
fi

echo "==> Lint MAS entitlements (AMFI-safe)"
export ENTITLEMENTS_FILE="${REPO_ROOT}/apps/desktop/src-tauri/entitlements.mas.plist"
bash "${REPO_ROOT}/scripts/ci/check-entitlements.sh"

echo "==> plint PrivacyInfo.xcprivacy (source)"
plutil -lint "${REPO_ROOT}/apps/desktop/src-tauri/PrivacyInfo.xcprivacy"

cd "$REPO_ROOT"
echo "==> pnpm install (frozen lockfile)"
pnpm install --frozen-lockfile

echo "==> Vite build (@dayseam/desktop)"
pnpm --filter @dayseam/desktop build

echo "==> rustup targets for universal-apple-darwin"
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# Tauri reads APPLE_* for app signing. Do **not** pass APPLE_ID / APPLE_PASSWORD
# here — that path triggers Developer-ID notarization, not App Store packaging.
export APPLE_CERTIFICATE="$DAYSEAM_MAS_APP_CERTIFICATE"
export APPLE_CERTIFICATE_PASSWORD="$DAYSEAM_MAS_APP_CERTIFICATE_PASSWORD"
export APPLE_SIGNING_IDENTITY="$DAYSEAM_MAS_APP_SIGNING_IDENTITY"
unset APPLE_ID APPLE_PASSWORD APPLE_API_ISSUER APPLE_API_KEY APPLE_API_KEY_PATH 2>/dev/null || true

MAS_FILES_JSON='{"bundle":{"macOS":{"files":{"embedded.provisionprofile":"MasStore.provisionprofile"}}}}'
NO_UPDATER_JSON='{"bundle":{"createUpdaterArtifacts":false}}'

echo "==> Tauri build — MAS universal .app (App Store–signed)"
(
  cd "${REPO_ROOT}/apps/desktop"
  pnpm exec tauri build --target universal-apple-darwin --bundles app \
    --config src-tauri/tauri.mas.conf.json \
    --config "$NO_UPDATER_JSON" \
    --config "$MAS_FILES_JSON" \
    --features mas
)

TARGET_DIR="$(cargo metadata --format-version 1 --no-deps --manifest-path "${REPO_ROOT}/Cargo.toml" | jq -r '.target_directory')"
if [[ -z "$TARGET_DIR" || "$TARGET_DIR" == "null" ]]; then
  echo "build-mas-store-pkg.sh: cargo metadata did not return target_directory" >&2
  exit 1
fi

APP="${TARGET_DIR}/universal-apple-darwin/release/bundle/macos/Dayseam.app"
if [[ ! -d "$APP" ]]; then
  echo "build-mas-store-pkg.sh: expected bundle at ${APP}" >&2
  exit 1
fi

echo "==> Verify embedded entitlements + privacy manifest"
bash "${REPO_ROOT}/scripts/ci/verify-tauri-bundle-entitlements.sh" "$APP" mas
bash "${REPO_ROOT}/scripts/ci/verify-bundle-privacy-manifest.sh" "$APP"

if ! codesign --verify --deep --strict "$APP"; then
  echo "build-mas-store-pkg.sh: codesign --verify failed on ${APP}" >&2
  exit 1
fi

echo "==> Import Mac Installer certificate for productbuild"
if ! echo "$DAYSEAM_MAS_INSTALLER_CERTIFICATE" | base64 --decode >"$INST_P12" 2>/dev/null; then
  echo "build-mas-store-pkg.sh: DAYSEAM_MAS_INSTALLER_CERTIFICATE is not valid base64" >&2
  exit 1
fi
security create-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
security set-keychain-settings -lut 3600 -u "$KEYCHAIN_PATH"
security unlock-keychain -p "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH"
security list-keychains -d user -s "$KEYCHAIN_PATH" login.keychain-db
security default-keychain -s "$KEYCHAIN_PATH"
security import "$INST_P12" -k "$KEYCHAIN_PATH" -P "$DAYSEAM_MAS_INSTALLER_CERTIFICATE_PASSWORD" \
  -T /usr/bin/productbuild -T /usr/bin/codesign
security set-key-partition-list -S apple-tool:,apple: -s -k "$KEYCHAIN_PASSWORD" "$KEYCHAIN_PATH" >/dev/null

OUT_DIR="${REPO_ROOT}/dist/release"
mkdir -p "$OUT_DIR"
PKG_TMP="${OUT_DIR}/Dayseam-mas-v${VERSION}-tmp.pkg"
PKG_OUT="${OUT_DIR}/Dayseam-mas-v${VERSION}.pkg"

echo "==> productbuild signed .pkg"
xcrun productbuild --sign "$DAYSEAM_MAS_INSTALLER_SIGNING_IDENTITY" \
  --component "$APP" /Applications "$PKG_TMP"

mv -f "$PKG_TMP" "$PKG_OUT"
( cd "$OUT_DIR" && shasum -a 256 "$(basename "$PKG_OUT")" >"${PKG_OUT}.sha256" )

echo "==> Built ${PKG_OUT}"
echo "$PKG_OUT"
