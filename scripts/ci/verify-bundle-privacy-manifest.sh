#!/usr/bin/env bash
# verify-bundle-privacy-manifest.sh — **MAS-7a** bundle gate.
#
# App Store Connect expects PrivacyInfo.xcprivacy under the app bundle
# Resources/ folder for required-reason API declarations. Tauri copies the
# checked-in file via bundle.macOS.files in apps/desktop/src-tauri/tauri.conf.json.
#
# Usage:
#   verify-bundle-privacy-manifest.sh <path-to-Dayseam.app>
#
# Exit codes: 0 ok, 1 verification failed, 2 bad args / missing bundle.

set -euo pipefail

APP="${1-}"

if [[ -z "$APP" ]]; then
  echo "usage: verify-bundle-privacy-manifest.sh <path-to.app>" >&2
  exit 2
fi

if [[ ! -d "$APP" ]]; then
  echo "verify-bundle-privacy-manifest.sh: bundle not found at ${APP}" >&2
  exit 1
fi

MANIFEST="${APP}/Contents/Resources/PrivacyInfo.xcprivacy"
if [[ ! -f "$MANIFEST" ]]; then
  echo "verify-bundle-privacy-manifest.sh: missing ${MANIFEST}" >&2
  exit 1
fi

if ! plutil -lint "$MANIFEST" >/dev/null; then
  plutil -lint "$MANIFEST" >&2 || true
  exit 1
fi

echo "verify-bundle-privacy-manifest.sh: ${MANIFEST} ok"
