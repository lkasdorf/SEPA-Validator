#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TARGET_DIR="${1:-to_check}"
OUT_DIR="${2:-analysis}"

cd "${ROOT_DIR}"

if ! command -v xmllint >/dev/null 2>&1; then
  echo "xmllint not found in PATH." >&2
  exit 1
fi

mkdir -p "${OUT_DIR}"
TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
CSV_PATH="${OUT_DIR}/validation_${TIMESTAMP}.csv"
LATEST_PATH="${OUT_DIR}/validation_latest.csv"

printf 'file,namespace,schema,status,error\n' > "${CSV_PATH}"

total=0
ok=0
fail=0
no_schema=0

while IFS= read -r -d '' file; do
  total=$((total + 1))
  ns="$(rg -o 'urn:iso:std:iso:20022:tech:xsd:[^" ]+' "${file}" -N | head -n1 || true)"

  schema=""
  case "${ns}" in
    *pain.001.001.09) schema="xml_schema/pain.001.001.09.xsd" ;;
    *pain.008.001.08) schema="xml_schema/pain.008.001.08.xsd" ;;
    *pain.008.001.02) schema="xml_schema/pain.008.001.02.xsd" ;;
    *pain.001.001.03) schema="xml_schema/pain.001.001.03.xsd" ;;
  esac

  if [[ -z "${schema}" || ! -f "${schema}" ]]; then
    no_schema=$((no_schema + 1))
    printf '"%s","%s","%s","NO_SCHEMA","%s"\n' "${file}" "${ns}" "${schema}" "No mapped schema for namespace" >> "${CSV_PATH}"
    continue
  fi

  if out="$(xmllint --noout --schema "${schema}" "${file}" 2>&1)"; then
    ok=$((ok + 1))
    printf '"%s","%s","%s","OK",""\n' "${file}" "${ns}" "${schema}" >> "${CSV_PATH}"
  else
    fail=$((fail + 1))
    msg="$(printf '%s' "${out}" | head -n1 | tr '"' "'" | tr '|' '/')"
    printf '"%s","%s","%s","FAIL","%s"\n' "${file}" "${ns}" "${schema}" "${msg}" >> "${CSV_PATH}"
  fi
done < <(find "${TARGET_DIR}" -type f -name '*.xml' -print0 | sort -z)

cp -f "${CSV_PATH}" "${LATEST_PATH}"

echo "Validation finished"
echo "Target: ${TARGET_DIR}"
echo "Files: ${total}"
echo "OK: ${ok}"
echo "FAIL: ${fail}"
echo "NO_SCHEMA: ${no_schema}"
echo "Report: ${CSV_PATH}"
echo "Latest: ${LATEST_PATH}"

