#!/usr/bin/env bash
# mas-connect-upload-preflight.sh — **MAS-8d** operator / CI preflight for
# App Store Connect uploads. Never prints secret values.
#
# Environment (set by GHA or locally):
#   INPUTS_DRY_RUN     — `true` / `false` (workflow_dispatch boolean string)
#   INPUTS_MAS_PKG_PATH — optional workspace-relative .pkg path
#   DAYSEAM_ASC_ISSUER_ID, DAYSEAM_ASC_KEY_ID, DAYSEAM_ASC_PRIVATE_KEY — App Store Connect API key material
#
# Exit codes:
#   0  preflight completed (informational summary).
#   1  repo layout drift (expected runbook missing under REPO_ROOT).

set -euo pipefail

REPO_ROOT="${REPO_ROOT:-$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../../.." && pwd)}"
cd "$REPO_ROOT"

dry="${INPUTS_DRY_RUN:-true}"
case "$dry" in
  true | True | 1) dry_run=1 ;;
  false | False | 0) dry_run=0 ;;
  *)
    echo "::error::INPUTS_DRY_RUN must be true or false (got: ${dry})" >&2
    exit 1
    ;;
esac

echo "==> MAS-8d — App Store Connect / TestFlight preflight"
echo "    Runbook: docs/release/MAS-CONNECT-UPLOAD.md"
echo "    Workflow: .github/workflows/mas-connect-upload.yml"

if [[ ! -f "${REPO_ROOT}/docs/release/MAS-CONNECT-UPLOAD.md" ]]; then
  echo "mas-connect-upload-preflight.sh: expected docs/release/MAS-CONNECT-UPLOAD.md" >&2
  exit 1
fi

have() {
  local n="${!1:-}"
  [[ -n "$n" ]]
}

for var in DAYSEAM_ASC_ISSUER_ID DAYSEAM_ASC_KEY_ID DAYSEAM_ASC_PRIVATE_KEY; do
  if have "$var"; then
    echo "    $var: set"
  else
    echo "    $var: MISSING"
  fi
done

pkg="${INPUTS_MAS_PKG_PATH:-}"
if [[ -n "$pkg" ]]; then
  if [[ -f "${REPO_ROOT}/${pkg}" ]]; then
    echo "    mas_pkg_path: found ($pkg)"
  elif [[ -f "$pkg" ]]; then
    echo "    mas_pkg_path: found ($pkg)"
  else
    echo "    mas_pkg_path: NOT FOUND ($pkg) — upload steps will fail until the file exists at repo root or under workspace."
  fi
else
  echo "    mas_pkg_path: (empty — upload steps skipped unless workflow input is set)"
fi

if [[ "$dry_run" -eq 1 ]]; then
  echo "==> dry_run=true — no Transporter install or TestFlight upload will run."
else
  echo "==> dry_run=false — upload steps run only when mas_pkg_path is non-empty and secrets are complete (see workflow if: guards)."
fi
