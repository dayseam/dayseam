#!/usr/bin/env bash
# verify-tauri-bundle-entitlements.sh — **MAS-1b** codesign gate.
#
# After `pnpm exec tauri build --bundles app`, assert the built `.app`
# carries the entitlement keys we require for CI (same three keys as
# `entitlements.plist` / stub `entitlements.mas.plist` today). This goes
# beyond `plutil -lint` on the source file: it exercises what `codesign`
# actually embedded.
#
# Usage:
#   verify-tauri-bundle-entitlements.sh <path-to-Dayseam.app> <direct|mas>
#
# Exit codes: 0 ok, 1 verification failed, 2 bad args / missing bundle.

set -euo pipefail

APP="${1-}"
MODE="${2-}"

if [[ -z "$APP" || -z "$MODE" ]]; then
  echo "usage: verify-tauri-bundle-entitlements.sh <path-to.app> <direct|mas>" >&2
  exit 2
fi

if [[ "$MODE" != "direct" && "$MODE" != "mas" ]]; then
  echo "verify-tauri-bundle-entitlements.sh: mode must be direct or mas, got \`$MODE\`" >&2
  exit 2
fi

if [[ ! -d "$APP" ]]; then
  echo "verify-tauri-bundle-entitlements.sh: bundle not found at ${APP}" >&2
  exit 1
fi

ENT_TMP="$(mktemp)"
trap 'rm -f "$ENT_TMP"' EXIT

# Embed what the signature actually carries (XML plist on stdout).
# Do not hide stderr — CI needs codesign diagnostics when this fails.
if ! codesign -d --entitlements :- --xml "$APP" >"$ENT_TMP"; then
  echo "verify-tauri-bundle-entitlements.sh: codesign could not read entitlements from ${APP}" >&2
  exit 1
fi

if ! plutil -lint "$ENT_TMP" >/dev/null; then
  echo "verify-tauri-bundle-entitlements.sh: embedded entitlements are not valid XML plist" >&2
  plutil -lint "$ENT_TMP" >&2 || true
  exit 1
fi

require_key() {
  local key="$1"
  if ! grep -qF "<key>${key}</key>" "$ENT_TMP"; then
    echo "verify-tauri-bundle-entitlements.sh: missing entitlement key \`${key}\` in ${APP}" >&2
    exit 1
  fi
}

require_key "com.apple.security.files.user-selected.read-write"
require_key "com.apple.security.cs.allow-unsigned-executable-memory"
require_key "com.apple.security.cs.allow-jit"

if [[ "$MODE" == "mas" ]]; then
  # **MAS-2a** adds `com.apple.security.app-sandbox`. Until then the
  # stub deliberately matches the direct plist so dual-SKU CI stays
  # green without claiming a sandboxed store build.
  if grep -qF "<key>com.apple.security.app-sandbox</key>" "$ENT_TMP"; then
    echo "verify-tauri-bundle-entitlements.sh: MAS stub must not include com.apple.security.app-sandbox until MAS-2a (got key in ${APP})" >&2
    exit 1
  fi
fi

echo "verify-tauri-bundle-entitlements.sh: ${APP} (${MODE}) embeds expected entitlements."
