#!/usr/bin/env bash
# verify-tauri-bundle-entitlements.sh — **MAS-1b** + **MAS-2a** codesign gate.
#
# After `pnpm exec tauri build --bundles app`, assert the built `.app`
# carries the entitlement keys we require for CI (`direct`: same three
# keys as `entitlements.plist`; `mas`: those plus App Sandbox + network
# client per `entitlements.mas.plist`). This goes
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

forbid_key() {
  local key="$1"
  if grep -qF "<key>${key}</key>" "$ENT_TMP"; then
    echo "verify-tauri-bundle-entitlements.sh: \`${MODE}\` bundle must not embed \`${key}\` (got key in ${APP})" >&2
    exit 1
  fi
}

require_key "com.apple.security.files.user-selected.read-write"
require_key "com.apple.security.cs.allow-unsigned-executable-memory"
require_key "com.apple.security.cs.allow-jit"

if [[ "$MODE" == "direct" ]]; then
  # Direct SKU matches [`entitlements.plist`]: no App Sandbox, no
  # outbound-network entitlement (connectors use unsandboxed paths).
  forbid_key "com.apple.security.app-sandbox"
  forbid_key "com.apple.security.network.client"
fi

if [[ "$MODE" == "mas" ]]; then
  # **MAS-2a:** store-bound SKU must ship App Sandbox + outbound TLS
  # (connectors, OAuth, WKWebView). JIT-class keys stay until **MAS-2c**
  # narrows them with review evidence. (In-app updater is **off** on MAS;
  # see architecture §6.)
  require_key "com.apple.security.app-sandbox"
  require_key "com.apple.security.network.client"
fi

echo "verify-tauri-bundle-entitlements.sh: ${APP} (${MODE}) embeds expected entitlements."
