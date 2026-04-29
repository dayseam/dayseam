#!/usr/bin/env bash
# check-unreleased-for-semver-pr.sh — CI preflight for DAY-195.
#
# When a pull request carries semver:patch|minor|major, the release
# workflow runs `extract-release-notes.sh` against the version
# `bump-version.sh` would emit. If `[Unreleased]` is empty (and no
# matching `[$VERSION]` block exists), the release job exits 2 —
# which bit #183 — too late. This script runs the same extraction
# at PR time so contributors fix CHANGELOG before merge.
#
# Inputs:
#   PR_LABELS_JSON — JSON array of GitHub label objects (GitHub
#   Actions: `toJson(github.event.pull_request.labels)`).
#
# Environment:
#   REPO_ROOT — optional repo root (defaults to this script's ../../).
#
# Exit codes:
#   0  extraction succeeded (changelog ready for that release)
#   1  usage / missing jq / no semver bump label when invoked
#   2  extract-release-notes.sh failed (same as that script)

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)}"

if [[ -z "${PR_LABELS_JSON:-}" ]]; then
  echo "check-unreleased-for-semver-pr.sh: PR_LABELS_JSON is required (CI-only)" >&2
  exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "check-unreleased-for-semver-pr.sh: jq is required" >&2
  exit 1
fi

semver_label="$(printf '%s' "$PR_LABELS_JSON" | jq -r '.[].name | select(. == "semver:patch" or . == "semver:minor" or . == "semver:major")' | head -n 1)"
if [[ -z "$semver_label" ]]; then
  echo "check-unreleased-for-semver-pr.sh: expected semver:patch|minor|major on PR labels" >&2
  exit 1
fi

level="${semver_label#semver:}"

prev="$(REPO_ROOT="$REPO_ROOT" "${REPO_ROOT}/scripts/release/resolve-prev-version.sh")"
export DAYSEAM_PREV_VERSION="$prev"
target="$(REPO_ROOT="$REPO_ROOT" "${REPO_ROOT}/scripts/release/bump-version.sh" --dry-run "$level")"

echo "==> DAY-195 changelog preflight: prev=$prev label=$semver_label → would ship v$target" >&2

if ! "${REPO_ROOT}/scripts/release/extract-release-notes.sh" "$target" >/dev/null; then
  echo "check-unreleased-for-semver-pr.sh: fix CHANGELOG.md — see extract-release-notes.sh hints above." >&2
  exit 2
fi

echo "==> CHANGELOG ok for would-be v$target" >&2
