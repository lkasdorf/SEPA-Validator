# SEPA-Validator

Validate SEPA payment XML files against ISO 20022 XSD schemas.

![Windows](https://img.shields.io/badge/Windows-10%2F11-blue) ![Tauri](https://img.shields.io/badge/Tauri-Rust%20%2B%20Svelte-24C8DB) ![License](https://img.shields.io/badge/License-MIT-green) [![Latest release](https://img.shields.io/github/v/release/lkasdorf/SEPA-Validator)](https://github.com/lkasdorf/SEPA-Validator/releases/latest)

This repository contains three tools that share the same purpose:

- **SEPA Validator desktop app** (`app/`) — the modern native Windows app (Tauri + Rust + Svelte) with a live, clickable validation log. **This is the recommended tool.**
- **PowerShell GUI** (`windows/`) — the original WinForms tool. _Legacy._
- **CLI scripts** (`scripts/`) — bash validators for Linux/macOS/WSL.

All three validate the same way (ISO 20022 XSD validation); the desktop app and CLI use **libxml2**, the PowerShell tool uses .NET. Verdicts are equivalent.

---

## Desktop app (recommended)

A native Windows application that validates one or many files at once and lets you drill into exactly where each one fails.

### Features

- **Live, streaming log** — files appear and update as they are validated.
- **Click to the error** — click any error or warning to jump to its line in a syntax-highlighted XML viewer (pretty-printed, even if the source is all on one line).
- **Find & fold** — search within the open XML (`Ctrl+F`) and collapse/expand blocks; a lean mode keeps very large files (>10 MB) responsive.
- **Overview tab** — the creditor (`Cdtr`) block and a `PmtInf` statistics table (`NbOfTxs`, `CtrlSum`, `SvcLvl`, `LclInstrm`, `SeqTp`, dates) for pain.001 / pain.008.
- **Remittance tab** — one row per transaction with its origin (`InstrId` → `EndToEndId`) and remittance info (`Ustrd`), a warning for missing entries, and a **CSV export**.
- **Schemas… dialog** — import the XSD schemas (as `.xsd`, a folder, or a `.zip`), see which are present/missing, and open the schema folder. Schemas are **not bundled** (see below).
- **Export** results as TXT or CSV; **light/dark** theme; **drag & drop** files or folders; resizable panels.
- **Help/About menu** (☰) — about/version, keyboard shortcuts, licenses, a privacy note, "Copy Diagnostics", and a built-in **auto-updater** (Check for Updates).

### Install

1. Download the latest installer from the [**Releases**](https://github.com/lkasdorf/SEPA-Validator/releases/latest) page:
   - `SEPA-Validator-<version>-windows-x64-setup.exe` — installer, or
   - `SEPA-Validator-<version>-windows-x64-portable.exe` — single standalone executable.
2. Run it. It needs the Microsoft **WebView2** runtime (preinstalled on current Windows 10/11). The installer is unsigned, so SmartScreen may warn — choose **More info → Run anyway**.
3. From v2.1.0 onward the app updates itself: **☰ → Check for Updates**.

### First run — import the XSD schemas

The schemas are **not distributed** with the app (they are not redistributable). Open **Schemas…**, download them from the [official sources](#obtaining-xsd-schemas), and import them as `.xsd` files, a folder, or a `.zip`. The badge in the toolbar shows how many of the expected schemas are present.

### Build from source

```sh
cd app
npm install
npx tauri dev                 # run with hot reload
npx tauri build               # build installer -> app/src-tauri/target/release/bundle/
```

First-time native setup (vcpkg + libxml2, libclang, local `.cargo/config.toml`) is documented in [`app/README.md`](app/README.md).

---

## Supported SEPA formats

The desktop app and CLI recognise these namespaces (provide the matching XSD via the Schemas… dialog or `xml_schema/`):

| Format | Description |
|--------|-------------|
| pain.001.001.03 / .09 | Credit transfers (`.09` is current) |
| pain.002.001.10 | Payment status reports |
| pain.007.001.09 | Payment reversals (GBIC variant) |
| pain.008.001.02 / .08 | Direct debits (`.08` is current) |
| camt.054.001.08 | Bank-to-customer debit/credit notification |
| container.nnn.001.GBIC4 | DK/GBIC container |

## Obtaining XSD schemas

This tool requires XSD schema files for validation. Schemas are **not included** in this repository — download them from the official sources:

| Source | Schemas | URL |
|--------|---------|-----|
| ISO 20022 | pain.001, pain.002, pain.008, camt.054, … | [iso20022.org](https://www.iso20022.org/catalogue-of-iso-20022-messages) |
| Deutsche Kreditwirtschaft (DK) | GBIC variants for German SEPA | [die-dk.de](https://die-dk.de/themen/zahlungsverkehr/) |
| EBICS (Germany) | German SEPA data formats & schemas | [ebics.de](https://www.ebics.de/de/datenformate) |
| EPC | EPC SEPA scheme rulebooks | [europeanpaymentscouncil.eu](https://www.europeanpaymentscouncil.eu/document-library) |

For the **desktop app**, import the downloaded files via the **Schemas…** dialog. For the **CLI / PowerShell** tools, place the `.xsd` files in `xml_schema/` (or pass `--schema-dir`).

---

## CLI (Linux / macOS / WSL)

Requires `xmllint` (`sudo apt install libxml2-utils`, or `brew install libxml2` on macOS).

```bash
# Validate a single file, multiple files, or a whole folder
./scripts/validate.sh payment.xml
./scripts/validate.sh file1.xml file2.xml
./scripts/validate.sh /path/to/xml/files/

# Options
./scripts/validate.sh --schema-dir ./my-schemas payment.xml   # custom schema directory
./scripts/validate.sh --csv report.csv *.xml                  # export CSV
./scripts/validate.sh --export report.txt *.xml               # export text report
./scripts/validate.sh -q *.xml                                # quiet: errors + summary only
```

`scripts/validate_all.sh` batch-validates a tree and writes a CSV report; `scripts/rename_xml_by_*.sh` rename files by date/company or schema.

## PowerShell GUI (legacy)

The original single-file WinForms tool still lives under `windows/`.

```powershell
# Run as a script (place XSDs in windows/schemas/ first)
windows\SEPA-Validator.cmd

# Or build a standalone EXE with embedded schemas
Install-Module ps2exe -Scope CurrentUser
cd windows
powershell -ExecutionPolicy Bypass -File .\build.ps1    # -> windows/dist/SEPA-Validator.exe
```

It runs on PowerShell 5.1 (preinstalled on Windows 10/11) and needs no admin rights.

---

## Project structure

```
app/                    # Desktop app — Tauri + Rust + Svelte (recommended)
  src/                  #   Svelte/TypeScript frontend
  src-tauri/            #   Rust backend (libxml2 validation, schema import, updater)
windows/                # PowerShell/WinForms GUI (legacy)
  SEPA-Validator.ps1    #   Main application
  build.ps1             #   ps2exe build script
scripts/                # Bash CLI validators + renaming utilities
xml_schema/             # XSD schema files (not included — download from sources above)
```

## License

[MIT](LICENSE)
