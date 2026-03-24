# SEPA-Validator

A native Windows GUI tool for validating SEPA XML payment files against ISO 20022 XSD schemas.

![PowerShell](https://img.shields.io/badge/PowerShell-5.1+-blue) ![Windows](https://img.shields.io/badge/Windows-10%2F11-blue) ![License](https://img.shields.io/badge/License-MIT-green)

## Features

- **Drag & Drop** - drop XML files or entire folders onto the window
- **File & Folder Picker** - select files or directories via standard dialogs
- **Automatic Schema Detection** - identifies the XML namespace and selects the matching XSD
- **Detailed Error Reporting** - validation errors and warnings with line/column numbers
- **Color-Coded Results** - green (OK), red (errors), yellow (warnings)
- **Text Export** - save validation results as a readable summary report
- **No Installation Required** - runs on PowerShell 5.1 (preinstalled on Windows 10/11)
- **Standalone EXE** - can be compiled into a single executable with embedded schemas

## Supported SEPA Formats

| Format | Schema | Description |
|--------|--------|-------------|
| pain.001.001.03 | Credit Transfer v03 | Credit transfers |
| pain.001.001.09 | Credit Transfer v09 | Credit transfers (current) |
| pain.002.001.10 | Payment Status v10 | Status reports |
| pain.007.001.09 | Payment Reversal v09 | Payment reversals |
| pain.008.001.02 | Direct Debit v02 | Direct debits |
| pain.008.001.08 | Direct Debit v08 | Direct debits (current) |
| camt.054.001.08 | Bank-to-Customer Notification | Account statements |

## Quick Start

### Option A: Run as Script

1. Copy the `windows/` folder to your machine
2. Place your XSD schema files in a `schemas/` subfolder next to `SEPA-Validator.ps1`
3. Double-click `SEPA-Validator.cmd`

### Option B: Build Standalone EXE

1. Install the build tool: `Install-Module ps2exe -Scope CurrentUser`
2. Place your XSD schemas in `xml_schema/`
3. Run the build:

```powershell
cd windows
.\build.ps1
```

4. Find `SEPA-Validator.exe` in `windows/dist/` — all schemas are embedded, no extra files needed

## Usage

1. **Load files** - drag & drop, "Dateien waehlen..." (file picker) or "Ordner waehlen..." (folder picker)
2. **Review results** - click a file in the upper grid to see details below
3. **Export** - "Ergebnis exportieren..." saves a text report

## Adding Custom Schemas

1. Place the XSD file in the `schemas/` folder (or `xml_schema/` for EXE builds)
2. Add the namespace mapping in `SEPA-Validator.ps1`:

```powershell
$SchemaMap = [ordered]@{
    # ... existing entries ...
    'urn:your:namespace:here' = 'your_schema.xsd'
}
```

3. Rebuild the EXE if using the standalone version

## Requirements

- Windows 10 or 11
- PowerShell 5.1+ (preinstalled)
- No admin rights required

## Project Structure

```
windows/
  SEPA-Validator.ps1    # Main application
  SEPA-Validator.cmd    # Launcher (double-click)
  build.ps1             # EXE build script
  setup.cmd             # Schema copy helper
xml_schema/             # XSD schema files (not included - add your own)
```

## License

[MIT](LICENSE)
