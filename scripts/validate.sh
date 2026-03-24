#!/usr/bin/env bash
set -euo pipefail

# SEPA XML Validator - CLI for Linux/macOS
# Validates one or more SEPA XML files against ISO 20022 XSD schemas.
#
# Usage:
#   ./scripts/validate.sh file.xml [file2.xml ...]
#   ./scripts/validate.sh path/to/folder/
#   ./scripts/validate.sh --schema-dir /path/to/xsds file.xml

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
SCHEMA_DIR="${ROOT_DIR}/xml_schema"

# --- Colors ---
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[0;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED='' GREEN='' YELLOW='' BLUE='' BOLD='' RESET=''
fi

# --- Usage ---
usage() {
    cat <<EOF
SEPA XML Validator

Usage:
  $(basename "$0") [options] <file.xml|folder> [file2.xml ...]

Options:
  --schema-dir DIR   Path to directory with XSD schemas (default: xml_schema/)
  --export FILE      Export results to text file
  --csv FILE         Export results to CSV file
  -q, --quiet        Only show errors and summary
  -h, --help         Show this help

Examples:
  $(basename "$0") payment.xml
  $(basename "$0") /path/to/xml/files/
  $(basename "$0") --schema-dir ./schemas *.xml
  $(basename "$0") --export report.txt *.xml
EOF
    exit 0
}

# --- Schema mapping ---
resolve_schema() {
    local ns="$1"
    local xsd=""
    case "${ns}" in
        *pain.001.001.03) xsd="pain.001.001.03.xsd" ;;
        *pain.001.001.09) xsd="pain.001.001.09.xsd" ;;
        *pain.002.001.10) xsd="pain.002.001.10.xsd" ;;
        *pain.007.001.09) xsd="pain.007.001.09.xsd" ;;
        *pain.008.001.02) xsd="pain.008.001.02.xsd" ;;
        *pain.008.001.08) xsd="pain.008.001.08.xsd" ;;
        *camt.054.001.08)  xsd="camt.054.001.08.xsd" ;;
    esac
    if [[ -n "${xsd}" && -f "${SCHEMA_DIR}/${xsd}" ]]; then
        echo "${SCHEMA_DIR}/${xsd}"
    fi
}

# --- Extract namespace ---
get_namespace() {
    local file="$1"
    # Try xmllint first, fall back to grep
    if command -v xmllint >/dev/null 2>&1; then
        xmllint --xpath 'namespace-uri(/*)' "$file" 2>/dev/null || true
    else
        grep -oP 'xmlns="[^"]+"' "$file" | head -1 | grep -oP '(?<=xmlns=")[^"]+' || true
    fi
}

# --- Validate single file ---
validate_file() {
    local file="$1"
    local filename
    filename="$(basename "$file")"

    # Skip non-XML and Zone.Identifier files
    [[ "$file" == *:Zone.Identifier ]] && return 0
    [[ "$file" != *.xml && "$file" != *.XML ]] && return 0

    total=$((total + 1))

    local ns
    ns="$(get_namespace "$file")"

    if [[ -z "$ns" ]]; then
        errors=$((errors + 1))
        printf "${RED}${BOLD}INVALID${RESET}  %s\n" "$file"
        printf "         No XML namespace detected\n\n"
        append_result "$file" "" "" "ERROR" "No XML namespace detected"
        return 0
    fi

    local schema
    schema="$(resolve_schema "$ns")"

    if [[ -z "$schema" ]]; then
        no_schema=$((no_schema + 1))
        printf "${YELLOW}${BOLD}NO SCHEMA${RESET} %s\n" "$file"
        [[ "$quiet" -eq 0 ]] && printf "          Namespace: %s\n\n" "$ns"
        append_result "$file" "$ns" "" "NO_SCHEMA" "No matching schema"
        return 0
    fi

    local schema_name
    schema_name="$(basename "$schema")"

    local output
    if output="$(xmllint --noout --schema "$schema" "$file" 2>&1)"; then
        ok=$((ok + 1))
        if [[ "$quiet" -eq 0 ]]; then
            printf "${GREEN}${BOLD}OK${RESET}       %s\n" "$file"
        fi
        append_result "$file" "$ns" "$schema_name" "OK" ""
    else
        fail=$((fail + 1))
        printf "${RED}${BOLD}INVALID${RESET}  %s\n" "$file"
        printf "         Namespace: %s\n" "$ns"
        printf "         Schema: %s\n" "$schema_name"

        # Print each error line
        local err_num=0
        while IFS= read -r line; do
            # Skip the final "file.xml fails to validate" line
            [[ "$line" == *"fails to validate"* ]] && continue
            [[ -z "$line" ]] && continue
            err_num=$((err_num + 1))
            printf "         ${RED}[%d]${RESET} %s\n" "$err_num" "$line"
        done <<< "$output"
        printf "\n"
        append_result "$file" "$ns" "$schema_name" "FAIL" "$output"
    fi
}

# --- Result collection for export ---
declare -a RESULTS=()

append_result() {
    local file="$1" ns="$2" schema="$3" status="$4" detail="$5"
    RESULTS+=("${file}|${ns}|${schema}|${status}|${detail}")
}

export_txt() {
    local out_file="$1"
    {
        printf "SEPA XML Validation - %s\n" "$(date '+%Y-%m-%d %H:%M:%S')"
        printf "%d files checked | OK: %d | Invalid: %d | No Schema: %d\n" "$total" "$ok" "$fail" "$no_schema"
        printf '%.0s=' {1..80}
        printf '\n'

        for entry in "${RESULTS[@]}"; do
            IFS='|' read -r file ns schema status detail <<< "$entry"
            printf "\nFile: %s\n" "$file"
            printf "Namespace: %s\n" "$ns"
            printf "Schema: %s\n" "$schema"
            printf "Status: %s\n" "$status"
            if [[ -n "$detail" ]]; then
                printf "\n%s\n" "$detail"
            fi
            printf '%.0s-' {1..80}
            printf '\n'
        done
    } > "$out_file"
    echo "Report saved to: ${out_file}"
}

export_csv() {
    local out_file="$1"
    {
        printf 'file,namespace,schema,status,error\n'
        for entry in "${RESULTS[@]}"; do
            IFS='|' read -r file ns schema status detail <<< "$entry"
            # Escape quotes in detail
            detail="${detail//\"/\'}"
            # First line only for CSV
            detail="$(echo "$detail" | head -1)"
            printf '"%s","%s","%s","%s","%s"\n' "$file" "$ns" "$schema" "$status" "$detail"
        done
    } > "$out_file"
    echo "CSV saved to: ${out_file}"
}

# --- Parse arguments ---
files=()
export_file=""
csv_file=""
quiet=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --schema-dir) SCHEMA_DIR="$2"; shift 2 ;;
        --export)     export_file="$2"; shift 2 ;;
        --csv)        csv_file="$2"; shift 2 ;;
        -q|--quiet)   quiet=1; shift ;;
        -h|--help)    usage ;;
        *)            files+=("$1"); shift ;;
    esac
done

if [[ ${#files[@]} -eq 0 ]]; then
    echo "Error: No files or directories specified." >&2
    echo "Run with --help for usage information." >&2
    exit 1
fi

# --- Check prerequisites ---
if ! command -v xmllint >/dev/null 2>&1; then
    echo "Error: xmllint not found. Install libxml2-utils (Debian/Ubuntu) or libxml2 (macOS/brew)." >&2
    exit 1
fi

if [[ ! -d "$SCHEMA_DIR" ]]; then
    echo "Error: Schema directory not found: ${SCHEMA_DIR}" >&2
    echo "Use --schema-dir to specify the path to your XSD files." >&2
    exit 1
fi

# --- Collect XML files ---
xml_files=()
for arg in "${files[@]}"; do
    if [[ -d "$arg" ]]; then
        while IFS= read -r -d '' f; do
            xml_files+=("$f")
        done < <(find "$arg" -type f \( -name '*.xml' -o -name '*.XML' \) ! -name '*:Zone.Identifier' -print0 | sort -z)
    elif [[ -f "$arg" ]]; then
        xml_files+=("$arg")
    else
        echo "Warning: ${arg} not found, skipping." >&2
    fi
done

if [[ ${#xml_files[@]} -eq 0 ]]; then
    echo "No XML files found." >&2
    exit 1
fi

# --- Run validation ---
total=0 ok=0 fail=0 errors=0 no_schema=0

printf "${BLUE}${BOLD}SEPA XML Validator${RESET}\n"
printf "Schema directory: %s\n" "$SCHEMA_DIR"
printf "Files to validate: %d\n\n" "${#xml_files[@]}"

for file in "${xml_files[@]}"; do
    validate_file "$file"
done

# --- Summary ---
printf "${BOLD}Summary:${RESET} %d files | " "$total"
[[ $ok -gt 0 ]] && printf "${GREEN}%d OK${RESET} | " "$ok"
[[ $fail -gt 0 ]] && printf "${RED}%d invalid${RESET} | " "$fail"
[[ $errors -gt 0 ]] && printf "${RED}%d errors${RESET} | " "$errors"
[[ $no_schema -gt 0 ]] && printf "${YELLOW}%d no schema${RESET} | " "$no_schema"
printf "\n"

# --- Export ---
[[ -n "$export_file" ]] && export_txt "$export_file"
[[ -n "$csv_file" ]] && export_csv "$csv_file"

# Exit code: 0 if all OK, 1 if any failures
[[ $fail -eq 0 && $errors -eq 0 ]] || exit 1
