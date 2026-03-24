# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SEPA XML Validator — a native Windows GUI tool (PowerShell/WinForms) that validates SEPA payment XML files against ISO 20022 XSD schemas. Also serves as a local data curation workspace for triaging XML payment files.

## Key Commands

### Windows GUI Tool (on Windows)
```powershell
# Run the validator
powershell -ExecutionPolicy Bypass -STA -File windows\SEPA-Validator.ps1

# Build standalone EXE (requires: Install-Module ps2exe -Scope CurrentUser)
cd windows
powershell -ExecutionPolicy Bypass -File .\build.ps1
# Output: windows/dist/SEPA-Validator.exe
```

### CLI validation (Linux/macOS/WSL)
```bash
./scripts/validate.sh file.xml                           # Validate single file
./scripts/validate.sh path/to/folder/                    # Validate all XMLs in folder
./scripts/validate.sh --csv report.csv *.xml             # Export results to CSV
./scripts/validate_all.sh to_check analysis              # Batch validate, write CSV report
./scripts/rename_xml_by_date_company_format.sh to_check analysis  # Rename by date/company
```

Prerequisites: `bash`, `xmllint` (for validate.sh), `rg` (ripgrep, for validate_all.sh).

## Architecture

### Windows GUI (`windows/SEPA-Validator.ps1`)

Single-file PowerShell WinForms application. Key sections in order:

1. **Schema config** — `$SchemaMap` maps XML namespaces to XSD filenames. `$EmbeddedSchemas` is populated by build.ps1 for EXE mode, otherwise schemas load from `schemas/` subfolder.
2. **Validation engine** — `Get-XmlNamespace` reads the first element's namespace. `Test-SepaXml` does full XSD validation via .NET `System.Xml.Schema.XmlSchemaSet`. Schema compilation is cached per-namespace in `$script:SchemaCache`.
3. **GUI** — WinForms controls with docked layout. Controls are added to the form in reverse dock-priority order (last added = docks first).
4. **Event handlers** — Drag & drop, file/folder dialogs, grid selection, export.

### Critical implementation constraints (PowerShell 5.1 / WinForms)

- **`XmlResolver = $null`** must be set on all `XmlReaderSettings` and `XmlSchemaSet` instances, otherwise validation hangs trying to resolve external resources over the network.
- **`-STA` flag** is required in the CMD launcher — WinForms needs Single-Thread Apartment mode.
- **No `Set-StrictMode`** — `StrictMode -Version Latest` breaks `.Count` on .NET collection objects in PS 5.1.
- **WinForms dock order** — controls added LAST to `$form.Controls` dock FIRST. Getting this wrong causes layout issues (panels overlapping, wrong sizing).
- **`[System.Windows.Forms.Application]::DoEvents()`** in validation loop keeps UI responsive.

### Build process (`windows/build.ps1`)

Reads XSD files from `xml_schema/`, GZip-compresses and Base64-encodes them, injects into SEPA-Validator.ps1 at the `# @@EMBEDDED_SCHEMAS@@` marker, then compiles via ps2exe to a standalone EXE.

## Data Directories (gitignored)

- `xml_schema/` — XSD schemas (not redistributable, download from iso20022.org or ebics.de)
- `to_check/` — XML files sorted into `inbox/`, `valid/`, `invalid/`, `duplicates/`, `archive/`
- `analysis/` — Generated CSV/Markdown validation reports
- `scripts/` — Bash CLI scripts for validation and renaming (tracked in git)

## Conventions

- Commit format: `type(scope): short summary` (e.g., `fix(validator): ...`, `feat(windows): ...`, `docs: ...`)
- Shell scripts use `set -euo pipefail`
- XML file naming: `YYYYMMDD_COMPANY_FORMAT.xml` with `_1`, `_2` on collisions
- Validation reports are kept for traceability; never delete prior timestamped reports
