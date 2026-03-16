#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
TARGET_DIR="${1:-to_check}"
OUT_DIR="${2:-analysis}"

cd "${ROOT_DIR}"
mkdir -p "${OUT_DIR}"

if ! command -v xmllint >/dev/null 2>&1; then
  echo "xmllint not found in PATH." >&2
  exit 1
fi

timestamp="$(date +%Y%m%d_%H%M%S)"
map_csv="${OUT_DIR}/rename_xml_company_${timestamp}.csv"
latest_csv="${OUT_DIR}/rename_xml_company_latest.csv"

printf 'old_path,new_path,date,company,format,status\n' > "${map_csv}"

extract_first_text() {
  local file="$1"
  local xpath="$2"
  xmllint --xpath "string(${xpath})" "${file}" 2>/dev/null || true
}

normalize_company() {
  local raw="$1"
  local normalized=""

  if command -v iconv >/dev/null 2>&1; then
    normalized="$(printf '%s' "${raw}" | iconv -f UTF-8 -t ASCII//TRANSLIT 2>/dev/null || printf '%s' "${raw}")"
  else
    normalized="${raw}"
  fi

  normalized="$(printf '%s' "${normalized}" | tr '[:lower:]' '[:upper:]')"
  normalized="$(printf '%s' "${normalized}" | sed -E 's/[^A-Z0-9]+/_/g; s/^_+//; s/_+$//; s/_+/_/g')"
  if [[ -z "${normalized}" ]]; then
    normalized="NOFIRMA"
  fi
  printf '%s' "${normalized}"
}

extract_date() {
  local file="$1"
  local d=""
  d="$(extract_first_text "${file}" "//*[local-name()='ReqdExctnDt'][1]")"
  if [[ -z "${d}" ]]; then
    d="$(extract_first_text "${file}" "//*[local-name()='ReqdColltnDt'][1]")"
  fi
  if [[ -z "${d}" ]]; then
    d="$(extract_first_text "${file}" "//*[local-name()='CreDtTm'][1]")"
  fi
  d="$(printf '%s' "${d}" | rg -o '[0-9]{4}-?[0-9]{2}-?[0-9]{2}' -N | head -n1 || true)"
  d="${d//-/}"
  if [[ "${#d}" -ne 8 ]]; then
    d="00000000"
  fi
  printf '%s' "${d}"
}

extract_company() {
  local file="$1"
  local c=""
  c="$(extract_first_text "${file}" "//*[local-name()='InitgPty']/*[local-name()='Nm'][1]")"
  if [[ -z "${c}" ]]; then
    c="$(extract_first_text "${file}" "//*[local-name()='Dbtr']/*[local-name()='Nm'][1]")"
  fi
  if [[ -z "${c}" ]]; then
    c="$(extract_first_text "${file}" "//*[local-name()='Cdtr']/*[local-name()='Nm'][1]")"
  fi
  normalize_company "${c}"
}

extract_format() {
  local file="$1"
  local ns=""
  ns="$(rg -o 'urn:iso:std:iso:20022:tech:xsd:[^" ]+' "${file}" -N | head -n1 || true)"
  if [[ -z "${ns}" ]]; then
    printf 'UNKNOWN'
    return
  fi
  ns="${ns##*:}"
  ns="$(printf '%s' "${ns}" | tr '[:lower:]' '[:upper:]' | tr -cd 'A-Z0-9')"
  if [[ -z "${ns}" ]]; then
    ns="UNKNOWN"
  fi
  printf '%s' "${ns}"
}

renamed=0
skipped=0

while IFS= read -r -d '' file; do
  dir="$(dirname "${file}")"
  date_part="$(extract_date "${file}")"
  company_part="$(extract_company "${file}")"
  format_part="$(extract_format "${file}")"
  stem="${date_part}_${company_part}_${format_part}"
  candidate="${stem}.xml"
  new_path="${dir}/${candidate}"

  n=1
  while [[ -e "${new_path}" && "${new_path}" != "${file}" ]]; do
    candidate="${stem}_${n}.xml"
    new_path="${dir}/${candidate}"
    n=$((n + 1))
  done

  if [[ "${file}" == "${new_path}" ]]; then
    skipped=$((skipped + 1))
    printf '"%s","%s","%s","%s","%s","SKIPPED"\n' "${file}" "${new_path}" "${date_part}" "${company_part}" "${format_part}" >> "${map_csv}"
    continue
  fi

  mv "${file}" "${new_path}"
  renamed=$((renamed + 1))
  printf '"%s","%s","%s","%s","%s","RENAMED"\n' "${file}" "${new_path}" "${date_part}" "${company_part}" "${format_part}" >> "${map_csv}"
done < <(find "${TARGET_DIR}" -type f -name '*.xml' -print0 | sort -z)

cp -f "${map_csv}" "${latest_csv}"

echo "Rename finished"
echo "Target: ${TARGET_DIR}"
echo "Renamed: ${renamed}"
echo "Skipped: ${skipped}"
echo "Mapping: ${map_csv}"
echo "Latest: ${latest_csv}"

