#!/usr/bin/env bash
# Tests for the bash CLI scripts. Needs xmllint and rg (CI installs them).
# Usage: bash scripts/validate.test.sh
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCHEMAS="${ROOT}/app/src-tauri/tests/fixtures/schemas" # self-written mini XSD (pain.008.001.02 ns)
WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT
NS="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"
failures=0
n=0

check() { # check <name> <command...>
  local name="$1"; shift
  n=$((n + 1))
  if "$@"; then echo "ok ${n} - ${name}"; else echo "not ok ${n} - ${name}"; failures=$((failures + 1)); fi
}

tx() { printf '<Tx><Nm>%s</Nm><IBAN>%s</IBAN><BIC>%s</BIC></Tx>' "$1" "$2" "$3"; }
doc() { printf '<?xml version="1.0" encoding="UTF-8"?>\n<Document xmlns="%s">\n%s\n</Document>\n' "${NS}" "$1"; }

doc "$(tx 'Muster GmbH' 'DE89370400440532013000' 'COBADEFFXXX')" > "${WORK}/good.xml"
# Two schema errors: IBAN pattern and BIC pattern.
doc "$(tx 'Muster, "Co"' 'BAD-IBAN' 'bad-bic')" > "${WORK}/two errors.xml"

run_validate() { bash "${ROOT}/scripts/validate.sh" --schema-dir "${SCHEMAS}" "$@" >"${WORK}/stdout" 2>&1 || true; }

# --- TXT export keeps every error, not just the first line ---
run_validate --export "${WORK}/report.txt" "${WORK}/good.xml" "${WORK}/two errors.xml"
check "TXT export lists the IBAN error" grep -q "IBAN" "${WORK}/report.txt"
check "TXT export lists the BIC error too" grep -q "BIC" "${WORK}/report.txt"
check "TXT export shows the OK file" grep -q "Status: OK" "${WORK}/report.txt"

# --- CSV export is RFC 4180 (quotes doubled, one row per file, all errors) ---
cp "${WORK}/two errors.xml" "${WORK}/quote \"q\".xml"
run_validate --csv "${WORK}/report.csv" "${WORK}/good.xml" "${WORK}/quote \"q\".xml"
check "CSV has a header and one row per file" test "$(wc -l < "${WORK}/report.csv")" -eq 3
check "CSV doubles quotes inside fields" grep -qF 'quote ""q"".xml' "${WORK}/report.csv"
check "CSV row carries all errors" bash -c "grep -F 'quote' '${WORK}/report.csv' | grep -q 'IBAN.*BIC'"

# --- argument errors are reported, not a bash 'unbound variable' crash ---
out="$(bash "${ROOT}/scripts/validate.sh" --schema-dir 2>&1 || true)"
check "--schema-dir without a value is a clear error" bash -c "[[ '${out}' == *'--schema-dir needs a value'* ]]"

# --- every xmllint call forbids network access ---
calls="$(grep -h 'xmllint ' "${ROOT}"/scripts/*.sh | grep -v -e 'command -v' -e 'not found' -e '^\s*#' -e 'validate.test.sh' | grep -v -- '--nonet' || true)"
check "all xmllint calls use --nonet" test -z "${calls}"

# --- scripts that use ripgrep stop early with a clear message when it's missing ---
mkdir -p "${WORK}/bin"
for tool in dirname xmllint; do ln -s "$(command -v "${tool}")" "${WORK}/bin/${tool}"; done
for script in validate_all.sh rename_xml_by_date_company_format.sh rename_xml_by_schema.sh; do
  out="$(PATH="${WORK}/bin" "$(command -v bash)" "${ROOT}/scripts/${script}" "${WORK}" "${WORK}/out" 2>&1 || true)"
  check "${script} requires rg" bash -c "[[ '${out}' == *'ripgrep (rg) not found'* ]]"
done

echo "${n} checks, ${failures} failed"
[[ ${failures} -eq 0 ]]
