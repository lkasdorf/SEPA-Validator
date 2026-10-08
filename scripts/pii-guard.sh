#!/usr/bin/env bash
# PII guard: payment files (XML) and non-redistributable schemas (XSD) must never
# be committed. Fails if any .xml/.xsd is tracked outside the synthetic test
# fixtures. With a base revision it also checks every file added or renamed by the
# commits in <base>..HEAD, so a file added and later deleted in a PR is caught too
# (merge commits keep the PR's history).
#
# Usage: scripts/pii-guard.sh [<base-rev>]
set -euo pipefail

pattern='\.(xml|xsd)$'
# Synthetic test data, plus Tauri's generated Android launcher resources.
allowed='^(app/src-tauri/tests/fixtures/|app/src-tauri/icons/android/)'

list_tracked() { git -c core.quotepath=off ls-files -z | tr '\0' '\n'; }
list_added() {
  git -c core.quotepath=off log --format= --name-only -z --diff-filter=AR "$1..HEAD" | tr '\0' '\n'
}

files=$(list_tracked)
if [[ $# -ge 1 ]] && git rev-parse --verify -q "$1^{commit}" >/dev/null; then
  files=$(printf '%s\n%s\n' "$files" "$(list_added "$1")")
fi

violations=$(printf '%s\n' "$files" | grep -Ei "$pattern" | grep -Ev "$allowed" | sort -u || true)
if [[ -n "$violations" ]]; then
  echo "::error::XML/XSD files outside app/src-tauri/tests/fixtures/ - payment data and schemas must not be committed:"
  printf '%s\n' "$violations" | sed 's/^/  /'
  exit 1
fi
echo "PII guard: OK (no XML/XSD outside app/src-tauri/tests/fixtures/)"
