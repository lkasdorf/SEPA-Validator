# SEPA-Validator

Validate SEPA payment and bank statement XML files against ISO 20022 XSD schemas, and catch the mistakes the schema can't see.

![Windows](https://img.shields.io/badge/Windows-10%2F11-blue) ![Tauri](https://img.shields.io/badge/Tauri-Rust%20%2B%20Svelte-24C8DB) ![License](https://img.shields.io/badge/License-MIT-green) [![Latest release](https://img.shields.io/github/v/release/lkasdorf/SEPA-Validator)](https://github.com/lkasdorf/SEPA-Validator/releases/latest) [![CI](https://github.com/lkasdorf/SEPA-Validator/actions/workflows/ci.yml/badge.svg)](https://github.com/lkasdorf/SEPA-Validator/actions/workflows/ci.yml)

![SEPA Validator showing a camt.053 statement in the Transactions tab, with GVC names and ISO bank transaction codes](docs/screenshot.png)

This repository contains:

- **SEPA Validator desktop app** (`app/`): a native Windows app (Tauri + Rust + Svelte) with a live, clickable validation log. **This is the recommended tool.**
- **CLI scripts** (`scripts/`): bash validators for Linux, macOS and WSL.

Both validate with **libxml2**. Everything runs locally; no file leaves your machine.

The original PowerShell/WinForms tool was retired in favour of the desktop app. It is still available at tag [`v1.0.0`](https://github.com/lkasdorf/SEPA-Validator/tree/v1.0.0/windows).

---

## Desktop app (recommended)

A native Windows application that validates one or many files at once and lets you drill into exactly where each one fails.

### Features

- **Strict XSD validation.** Files that aren't well-formed fail with a located error. Values are checked exactly as written, so a stray space in an IBAN or an empty `<Nm></Nm>` is caught.
- **The right schema per file.** German credit transfers come in three DK subsets of pain.001.001.09 with the same namespace: SEPA, urgent euro (CCU) and foreign payments (AXZ). The app picks the subset from the content, falls back to another DK subset if that one accepts the file, and recognizes Swiss files by the debtor IBAN. The log names the XSD it used.
- **Plausibility checks** the schema can't express, reported as warnings:
  - `NbOfTxs` and `CtrlSum` checked against the actual transactions.
  - IBAN check digits (mod 97).
  - Execution or collection dates in the past.
  - Duplicate EndToEndIds.
  - Statements (camt): opening balance plus booked entries must equal the closing balance, and the transaction summary must match the entries.
- **Readable messages with hints.** For example: "Use a dot as the decimal separator, e.g. 1250.00." or "Required element <InitgPty> is missing in <GrpHdr>."
- **Live log, click to the error.** Results stream in as files are validated. Click any error to jump to its line in a syntax-highlighted, pretty-printed XML viewer. Search with `Ctrl+F`, fold blocks, and **Cancel** a long run.
- **Overview, Remittance and Transactions tabs.**
  - The **Overview** shows the creditor or debtor and the `PmtInf` statistics for pain.001 and pain.008. For statements it shows the account, period, sequence number, balances and credit/debit totals.
  - **Transactions** has one row per transaction or statement entry, with search, sorting, exact sums per currency and CSV export for Excel.
  - Statement entries add booking date, **GVC** with its name, and the ISO bank transaction code. A click opens all details of the entry: parties, references, return reason, FX amounts, charges.
  - An **↗** button next to each IBAN checks it online on request.
- **Edit in the viewer.** Change the XML directly, **Validate** the edited text, and **Save copy** writes `NAME_edit1.xml`. The original is never overwritten.
- **Generate statement…** turns a pain.001 or pain.008 file into synthetic camt.053 statements, as if the bank had booked it. This is useful for testing statement imports.
- **Save formatted…** writes the indented XML as a new file. Values stay byte-identical, and the original is never touched.
- **Schemas… dialog.** Import the XSDs as `.xsd` files, a folder or a `.zip`, and see which are present. It also imports the DK GVC mapping table (`.xlsx`) for the GVC names. Neither is **bundled** (see below).
- **Export and convenience.** TXT/CSV export, light/dark theme, drag & drop, a Help/About menu with third-party licenses, and a built-in **auto-updater**.

### Install

1. Download the latest release from the [**Releases**](https://github.com/lkasdorf/SEPA-Validator/releases/latest) page:
   - `SEPA-Validator-<version>-windows-x64-setup.exe` (installer), or
   - `SEPA-Validator-<version>-windows-x64-portable.exe` (single standalone executable, no installation).
2. Run it. It needs the Microsoft **WebView2** runtime, which is preinstalled on current Windows 10/11. Code signing through SignPath Foundation is being set up (see the [Code signing policy](CODE_SIGNING_POLICY.md)). Until it is in place, the executables are unsigned, so SmartScreen may warn: choose **More info → Run anyway**.
3. To update, use **☰ → Check for Updates**. The installed app updates itself. The portable version offers the new portable file to download, so it never installs a second copy.

### First run: import the XSD schemas

The schemas are **not distributed** with the app because they are not redistributable.
1. Open **Schemas…**.
2. Download the schemas from the [official sources](#obtaining-xsd-schemas).
3. Import them as `.xsd` files, a folder or a `.zip`.
4. Optional: also import the DK table *Anhang 1 zu Anlage 3 – Datenformatstandards* (`.xlsx`, from ebics.de) to see the names of GVCs in statements.

The badge in the toolbar shows how many of the expected schemas are present.

### Build from source

```sh
cd app
npm install
npx tauri dev                 # run with hot reload
npx tauri build               # build the installer -> app/src-tauri/target/release/bundle/
```

First-time native setup is documented in [`app/README.md`](app/README.md): vcpkg + libxml2, libclang and a local `.cargo/config.toml`. How releases are built and signed is described in [`RELEASING.md`](RELEASING.md).

---

## Supported SEPA formats

Provide the matching XSD via the Schemas… dialog (desktop app) or `xml_schema/` (CLI).

| Format | Description |
|--------|-------------|
| pain.001.001.03 / .09 | Credit transfers (`.09` is current; see the DK subsets below) |
| pain.002.001.10 | Payment status reports |
| pain.007.001.09 | Payment reversals (the app uses the GBIC variant) |
| pain.008.001.02 / .08 | Direct debits (`.08` is current) |
| camt.052.001.08 | Bank-to-customer account report (intraday) |
| camt.053.001.08 | Bank-to-customer statement |
| camt.054.001.08 | Bank-to-customer debit/credit notification |
| container.nnn.001.GBIC4 / GBIC5 | DK/GBIC container |

### DK subsets of pain.001.001.09 (desktop app)

The Deutsche Kreditwirtschaft publishes three schemas for pain.001.001.09 that share one namespace. The app picks one per file:

| Schema | Used for | Picked when the file has |
|--------|----------|--------------------------|
| `pain.001.001.09_AXZ_GBIC_5.xsd` | Foreign payments and cheques (AXZ) | a non-EUR amount, `EqvtAmt`, `ChrgBr` DEBT/CRED, `PmtMtd` CHK, or a service level other than SEPA/URGP |
| `pain.001.001.09_CCU_GBIC_5.xsd` | Urgent euro transfers (CCU) | service level URGP or `ChrgBr` SHAR |
| `pain.001.001.09.xsd` (DK SEPA, GBIC_5) | SEPA and instant credit transfers | anything else |

If the picked schema rejects the file but another DK subset accepts it, that one is used. Swiss files (below) are always checked against the Swiss schema.

### Swiss Payment Standards (desktop app only)

| Format | Description |
|--------|-------------|
| pain.001.001.09.ch.03 | Swiss credit transfer (SPS 2025/2026) |
| pain.008.001.02.ch.03 | Swiss direct debit (CH-DD / LSV+) |
| pain.008.001.02.chsdd.02 | SEPA direct debit, Swiss variant |

The Swiss pain.008 variants have their own namespaces. The Swiss pain.001 shares the ISO namespace with the standard pain.001.001.09, so the app picks the Swiss schema per file. It does so when:
- the first debtor account (`DbtrAcct` IBAN) is Swiss or Liechtenstein (`CH…`/`LI…`), or
- `xsi:schemaLocation` names a `.ch.` schema.

Creditor accounts don't count: a German payer sending money to a Swiss account is still checked against the standard schema. The result shows which schema was used.

## Obtaining XSD schemas

The XSD schema files are **not included** in this repository. Download them from the official sources:

| Source | Schemas | URL |
|--------|---------|-----|
| ISO 20022 | pain.001, pain.002, pain.008, camt.052/053/054, … | [iso20022.org](https://www.iso20022.org/catalogue-of-iso-20022-messages) |
| Deutsche Kreditwirtschaft (DK) | GBIC variants for German SEPA | [die-dk.de](https://die-dk.de/themen/zahlungsverkehr/) |
| EBICS (Germany) | German SEPA data formats & schemas (incl. AXZ, CCU, camt) and the GVC mapping table | [ebics.de](https://www.ebics.de/de/datenformate) |
| SIX (Switzerland) | Swiss Payment Standards (`.ch.` schemas) | [six-group.com](https://www.six-group.com/en/products-services/banking-services/payment-standardization/standards/iso-20022.html) |
| EPC | EPC SEPA scheme rulebooks | [europeanpaymentscouncil.eu](https://www.europeanpaymentscouncil.eu/document-library) |

For the **desktop app**, import the downloaded files via the **Schemas…** dialog. For the **CLI**, place the `.xsd` files in `xml_schema/`, or pass `--schema-dir`.

---

## CLI (Linux / macOS / WSL)

Requires `xmllint`: `sudo apt install libxml2-utils`, or `brew install libxml2` on macOS. Validation never touches the network (`xmllint --nonet`).

```bash
# Validate a single file, multiple files, or a whole folder
./scripts/validate.sh payment.xml
./scripts/validate.sh file1.xml file2.xml
./scripts/validate.sh /path/to/xml/files/

# Options
./scripts/validate.sh --schema-dir ./my-schemas payment.xml   # custom schema directory
./scripts/validate.sh --export report.txt *.xml               # text report with every error per file
./scripts/validate.sh --csv report.csv *.xml                  # CSV (RFC 4180), all errors of a file in one cell
./scripts/validate.sh -q *.xml                                # quiet: errors + summary only
```

The batch and data-curation helpers additionally need `rg` ([ripgrep](https://github.com/BurntSushi/ripgrep)):

- `scripts/validate_all.sh` batch-validates a folder tree and writes a CSV report.
- `scripts/rename_xml_by_date_company_format.sh` renames files to `YYYYMMDD_COMPANY_FORMAT.xml`.
- `scripts/rename_xml_by_schema.sh` renames files to `YYYYMMDD_BIC_FORMAT.xml`.

---

## Project structure

```
app/                    # Desktop app: Tauri + Rust + Svelte
  src/                  #   Svelte/TypeScript frontend
  src-tauri/            #   Rust backend (libxml2 validation, plausibility checks, schema import, updater)
scripts/                # Bash CLI validators, renaming helpers, release and notices tooling
docs/                   # Screenshot and design notes
xml_schema/             # Your XSD schema files (not included; download from the sources above)
```

## Code signing policy

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/). What is signed, the team roles and the privacy policy are in [`CODE_SIGNING_POLICY.md`](CODE_SIGNING_POLICY.md).

## License

[MIT](LICENSE). Third-party components and their licenses are listed in [`app/THIRD-PARTY-NOTICES.txt`](app/THIRD-PARTY-NOTICES.txt).
