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
map_csv="${OUT_DIR}/rename_xml_${timestamp}.csv"
latest_csv="${OUT_DIR}/rename_xml_latest.csv"

printf 'old_path,new_path,date,bic,format,status\n' > "${map_csv}"

extract_first_text() {
  local file="$1"
  local tag="$2"
  xmllint --xpath "string(//*[local-name()='${tag}'][1])" "${file}" 2>/dev/null || true
}

extract_date() {
  local file="$1"
  local d=""
  d="$(extract_first_text "${file}" "ReqdExctnDt")"
  if [[ -z "${d}" ]]; then
    d="$(extract_first_text "${file}" "ReqdColltnDt")"
  fi
  if [[ -z "${d}" ]]; then
    d="$(extract_first_text "${file}" "CreDtTm")"
  fi
  d="$(printf '%s' "${d}" | rg -o '[0-9]{4}-?[0-9]{2}-?[0-9]{2}' -N | head -n1 || true)"
  d="${d//-/}"
  if [[ "${#d}" -ne 8 ]]; then
    d="00000000"
  fi
  printf '%s' "${d}"
}

extract_bic() {
  local file="$1"
  local bic=""
  bic="$(extract_first_text "${file}" "BICFI")"
  if [[ -z "${bic}" ]]; then
    bic="$(extract_first_text "${file}" "BIC")"
  fi
  bic="$(printf '%s' "${bic}" | tr '[:lower:]' '[:upper:]' | tr -cd 'A-Z0-9')"
  if [[ -z "${bic}" ]]; then
    bic="NOBIC"
  fi
  printf '%s' "${bic}"
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
declare -A seen

while IFS= read -r -d '' file; do
  dir="$(dirname "${file}")"
  old_base="$(basename "${file}")"
  date_part="$(extract_date "${file}")"
  bic_part="$(extract_bic "${file}")"
  format_part="$(extract_format "${file}")"
  stem="${date_part}_${bic_part}_${format_part}"
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
    printf '"%s","%s","%s","%s","%s","SKIPPED"\n' "${file}" "${new_path}" "${date_part}" "${bic_part}" "${format_part}" >> "${map_csv}"
    continue
  fi

  mv "${file}" "${new_path}"
  renamed=$((renamed + 1))
  printf '"%s","%s","%s","%s","%s","RENAMED"\n' "${file}" "${new_path}" "${date_part}" "${bic_part}" "${format_part}" >> "${map_csv}"
done < <(find "${TARGET_DIR}" -type f -name '*.xml' -print0 | sort -z)

cp -f "${map_csv}" "${latest_csv}"

echo "Rename finished"
echo "Target: ${TARGET_DIR}"
echo "Renamed: ${renamed}"
echo "Skipped: ${skipped}"
echo "Mapping: ${map_csv}"
echo "Latest: ${latest_csv}"

