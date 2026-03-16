# Repository Guidelines

## Project Structure & Module Organization
This repository is a SEPA XML validation and curation workspace, not an application codebase.

- `to_check/`: working data area for XML files.
- `to_check/inbox/`: landing zone for newly received XML files before triage.
- `to_check/valid/`: XML files passing schema validation.
- `to_check/invalid/`: XML files with schema errors.
- `to_check/duplicates/`: hash-identical duplicates.
- `to_check/archive/original_snapshot/`: preserved originals.
- `xml_schema/`: XSD schemas (for example `pain.001.001.09.xsd`, `pain.008.001.08.xsd`).
- `scripts/`: operational scripts for validation and renaming.
- `analysis/`: generated reports (CSV/Markdown).
- `Reference/`: external specification documents (PDF/XLSX).

## Build, Test, and Development Commands
No build step exists. Use shell scripts for data operations:

- `./scripts/validate_all.sh to_check analysis`
  Validates XMLs recursively and writes `analysis/validation_latest.csv`.
- New inbox checks should be validated first and then moved out of `to_check/inbox/` into `valid/`, `invalid/`, or `duplicates/` as appropriate.
- `./scripts/rename_xml_by_date_company_format.sh to_check analysis`
  Renames XMLs to `YYYYMMDD_COMPANY_FORMAT.xml` (adds `_1`, `_2` on collisions).
- `./scripts/rename_xml_by_schema.sh to_check analysis`
  Alternate renaming to `YYYYMMDD_BIC_FORMAT.xml`.

Prerequisites: `bash`, `xmllint`, `rg` (ripgrep), and standard Unix tools.

## Coding Style & Naming Conventions
- Shell scripts must use `set -euo pipefail`.
- Prefer POSIX-safe quoting and explicit paths.
- Keep scripts idempotent where possible and log outputs to `analysis/`.
- Ignore Windows sidecar files such as `*.xml:Zone.Identifier`; they are metadata, not XML inputs.
- File naming for XMLs:
  - Primary: `Ausführungsdatum_Firmenname_Format.xml`
  - Technical form: `YYYYMMDD_COMPANY_FORMAT.xml`

## Testing Guidelines
- Treat schema validation as the test gate.
- Before and after any bulk rename/move, run:
  - `./scripts/validate_all.sh to_check analysis`
- Review `analysis/validation_latest.csv` for `FAIL` and `NO_SCHEMA`.
- For newly arrived inbox files, keep a short Markdown note in `analysis/` summarizing disposition and required corrections.
- Keep validation reports for traceability; do not delete prior timestamped reports.

## Commit & Pull Request Guidelines
No Git metadata is currently present in this folder. Use this convention when versioning:

- Commit format: `type(scope): short summary` (for example `chore(scripts): add company-based rename tool`).
- PRs should include:
  - purpose and impacted directories,
  - sample commands run,
  - key report files (for example `analysis/validation_*.csv`),
  - rollback notes for bulk file operations.
