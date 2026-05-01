#!/usr/bin/env bash
# mas-sandbox-launch-smoke.sh — **MAS-2b** (sandboxed real shell smoke).
#
# Runs **after** `verify-tauri-bundle-entitlements.sh` in CI — entitlement
# embedding (**MAS-6a** `network.client` among others) is proven before this
# launch smoke.
#
# Starts the **production** Dayseam bundle executable for a short window
# so CI proves the MAS-signed `.app` stays alive past native bootstrap +
# WebView init — not a stub binary or plist-only gate.
#
# Usage:
#   mas-sandbox-launch-smoke.sh <path-to-Dayseam.app> [seconds_alive]
#
# Environment:
#   RUST_LOG — defaults to `error` to keep CI logs readable.
#
# Exit codes: 0 process survived the window; 1 bundle/launch failure;
# 2 invalid arguments.

set -euo pipefail

APP="${1-}"
SECS="${2:-18}"

if [[ -z "$APP" ]]; then
  echo "usage: mas-sandbox-launch-smoke.sh <path-to-Dayseam.app> [seconds_alive]" >&2
  exit 2
fi

if ! [[ "$SECS" =~ ^[1-9][0-9]*$ ]] || (( 10#$SECS > 600 )); then
  echo "mas-sandbox-launch-smoke.sh: seconds_alive must be a positive integer ≤ 600 (got: ${SECS})" >&2
  exit 2
fi

if [[ ! -d "$APP" ]]; then
  echo "mas-sandbox-launch-smoke.sh: bundle not found at ${APP}" >&2
  exit 1
fi

INFO="${APP}/Contents/Info.plist"
if [[ ! -f "$INFO" ]]; then
  echo "mas-sandbox-launch-smoke.sh: missing ${INFO}" >&2
  exit 1
fi

if ! command -v /usr/libexec/PlistBuddy >/dev/null 2>&1; then
  echo "mas-sandbox-launch-smoke.sh: PlistBuddy not found (macOS only)" >&2
  exit 1
fi

EX="$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$INFO" 2>/dev/null || true)"
if [[ -z "$EX" ]]; then
  echo "mas-sandbox-launch-smoke.sh: could not read CFBundleExecutable from ${INFO}" >&2
  exit 1
fi

BIN="${APP}/Contents/MacOS/${EX}"
if [[ ! -f "$BIN" ]]; then
  echo "mas-sandbox-launch-smoke.sh: executable missing at ${BIN}" >&2
  exit 1
fi
if [[ ! -x "$BIN" ]]; then
  echo "mas-sandbox-launch-smoke.sh: ${BIN} is not executable" >&2
  exit 1
fi

export RUST_LOG="${RUST_LOG:-error}"

echo "mas-sandbox-launch-smoke.sh: launching ${BIN} for ${SECS}s..."
"$BIN" &
pid=$!
# Give the runtime + WKWebView pipeline time to fault if entitlements
# or sandbox policy are incompatible with the real shell.
sleep "$((10#$SECS))"

if kill -0 "$pid" 2>/dev/null; then
  echo "mas-sandbox-launch-smoke.sh: pid ${pid} still alive after ${SECS}s — terminating"
  kill "$pid" 2>/dev/null || true
  # SIGTERM first; if the shell ignores it, `wait` could hang CI — cap with SIGKILL.
  for _ in 1 2 3 4 5 6 7 8 9 10; do
    if ! kill -0 "$pid" 2>/dev/null; then
      wait "$pid" 2>/dev/null || true
      exit 0
    fi
    sleep 1
  done
  kill -9 "$pid" 2>/dev/null || true
  wait "$pid" 2>/dev/null || true
  exit 0
fi

echo "mas-sandbox-launch-smoke.sh: pid ${pid} exited before ${SECS}s (sandbox/boot regression?)" >&2
wait "$pid" 2>/dev/null || true
exit 1
