# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **Plausibility checks** beyond the XSD, shown as warnings with a line to jump to and a hint:
  - `NbOfTxs` and `CtrlSum`, in `GrpHdr` and in every `PmtInf`, are checked against the actual transactions (amounts compared exactly as decimals).
  - IBAN check digits (mod 97).
  - Requested execution and collection dates in the past (the DK value `1999-01-01`, "as soon as possible", is accepted). Many affected payment blocks produce one warning.
  - EndToEndIds used more than once (`NOTPROVIDED` excepted).
  A schema-valid file with such findings is shown as **Warnings** instead of OK.
- **Readable error messages with hints.** Namespace prefixes such as `{urn:iso:std:iso:20022:tech:xsd:pain.001.001.09}` are removed. Common mistakes get a plain-language hint below the message: a missing or unexpected element, a code that isn't in the allowed list, a decimal comma, leading or trailing spaces, an empty value, or a value that is too long. Hints are included in the TXT export.
- **Empty state** with a drop zone, Select Files/Folder buttons and a warning when schemas are missing; a drop overlay shows while files are dragged over the window.
- **Save formatted…** in the XML tab saves the indented XML shown in the viewer as a new file, next to the original as `<name>_formatted.xml` by default. Only whitespace between tags changes; all values stay byte-identical, and the original file is never overwritten.
- **Cancel** button while a validation runs. The UI stops at once, and the backend stops after the file it is currently validating.
- **Third-party licenses.** About → Licenses can show the full license texts of every bundled component (Rust crates, the npm packages in the frontend, libxml2 and zlib). The same `THIRD-PARTY-NOTICES.txt` is attached to each release.

### Changed
- The legacy PowerShell/WinForms tool (`windows/`) was removed from master. It remains available at tag `v1.0.0`.
- libxml2 is now built without iconv support, so the app no longer links LGPL-licensed libiconv. SEPA files are UTF-8, so validation is unaffected.

### Fixed
- **Files that are not well-formed XML are no longer reported OK.** Truncated files, a bare `&`, control characters, a second root element or text after `</Document>` now fail with a located error. Before, libxml's recovery mode silently repaired them.
- **Pretty-printing no longer changes the verdict.** Leading or trailing spaces in values (e.g. ` DE89…` in `<IBAN>`, `COBADEFFXXX ` in `<BIC>`) and empty fields like `<Nm></Nm>` are now validated exactly as they appear in the file. Before, they were trimmed or padded and passed.
- **A single bad file no longer stops the run.** DOCTYPE declarations are rejected, undefined entities are reported, and an internal error marks just that file as an error. The run always finishes; before, the UI could hang at "Validating…".
- Error line numbers above 65,535 are reported correctly (they were capped at 65535).
- Parse errors now show their line and column.
- **CLI (`scripts/validate.sh`):**
  - The TXT export lists every error of a file, not just the first.
  - The CSV export is valid RFC 4180: quotes are doubled, and all errors of a file go into one cell.
  - `--schema-dir`, `--export` and `--csv` without a value give a clear error instead of a bash crash.
  - DK/GBIC container files are recognised.
- **The portable exe no longer installs a second copy when updating.** It used to run the NSIS installer like the installed app. It now detects that it is portable (no uninstaller next to it) and offers the new portable file to download instead.
- `validate_all.sh` and the rename scripts stop with a clear message when `rg` (ripgrep) is missing. Before, they silently reported every file as NO_SCHEMA or dated it 00000000.
- **Starting a new validation while one is running no longer mixes results.** Each run has an id, late events from an older run are ignored, and the backend stops the older run.
- **The XML viewer and Overview/Remittance tabs no longer show the previous file.** Before, a slower load could overwrite a newer selection. The viewer is dimmed while a file loads, and clicking an error doesn't jump while the text still belongs to another file.
- **Re-validating the same file reloads its content.** Before, the viewer and Overview kept showing the old text and summary.
- Error-line highlighting no longer breaks when errors are reported out of line order.
- Dropping a large folder no longer blocks the window while the folder is scanned.
- **Log panel no longer overflows** at the default window size. Long messages wrap, the filter buttons stay visible, and the window no longer scrolls. At the minimum window size the XML viewer keeps at least 280 px.
- **Readable status colors and badges.** The dark-mode WARN and ERROR badges, green text in light mode and red text on dark panels now meet the WCAG AA contrast ratio (4.5:1); a test guards the theme colors.
- A valid file's log says so instead of "No matches", and status labels use the singular where needed ("1 error", "1 warning").

### Security
- **Content Security Policy enabled.** Scripts load only from the app itself, so injected inline scripts and event handlers are blocked.
- **Links are allow-listed.** The app opens only `https://` links to github.com, ebics.de and six-group.com. Local paths, network shares, `file:` and other protocol handlers are refused.
- **Exports are path-checked.** The app writes only `.txt`/`.csv` files into an existing folder on a local drive. The unused raw file-read command was removed.
- The CLI scripts call `xmllint --nonet`, so validation never fetches anything from the network.
- **ZIP schema import hardened.** Entries with names Windows would resolve outside the schema folder (such as `C:evil.xsd`) are skipped, schema files are capped at 10 MB, and a failed or oversized import leaves no partial file behind.

## [2.2.0] - 2026-09-19

### Added
- **Swiss Payment Standards (SIX)**: validates Swiss credit transfers (`pain.001.001.09.ch.03`), Swiss direct debits (`pain.008.001.02.ch.03`), and Swiss SEPA direct debits (`pain.008.001.02.chsdd.02`). The Swiss pain.001 shares the ISO namespace, so the app checks each file: the Swiss schema is used when the first debtor IBAN is `CH`/`LI` or `xsi:schemaLocation` names a `.ch.` schema.
- Schemas… dialog: **Download CH…** opens the SIX schema page (the existing button is now **Download DE…**).

## [2.1.0] - 2026-06-21

### Added
- **Help / About menu** (the ☰ button in the toolbar): an **About** dialog (version, description, MIT license, GitHub links), plus **Keyboard Shortcuts**, **Licenses** (open-source components and a note that the XSD schemas are not bundled), and a **Privacy** statement (all validation runs locally).
- Menu actions: **Check for Updates…**, **Documentation**, **Changelog**, **Report an Issue**, and **Copy Diagnostics** (version, WebView2 version, schema status — for bug reports).
- **Built-in auto-updater**: the app can check GitHub for a newer released version and download, verify (signed), install, and restart in-app. Stable channel — pre-releases are not offered automatically.

### Changed
- **Visual redesign of the desktop app** toward a calm, instrument-like look:
  - The toolbar is now a quiet neutral surface instead of a solid blue bar; the accent colour is a deliberate "ledger ink" blue (`#1f53c2`, lighter in dark mode), reserved for the single primary action, the selected file/tab, and the progress bar.
  - One shared button system (primary / ghost / segmented control) replaces the three divergent button styles that existed across the toolbar, viewer bar, log filters, and dialogs.
  - Structured payment data is now set in a monospace, tabular-figure "ledger" treatment: IBAN, BIC, creditor identifier, the `PmtInf` statistics table, transaction origins, schema namespaces/filenames, and log line locators all line up for scanning and copying.
  - A spacing scale, a shared muted-text colour, and design tokens replace ad-hoc paddings and hard-coded greys.
- **The entire UI is now in English** (tabs, dialogs, status, empty states) — the former German labels are gone. The payment tabs are now **Overview** and **Remittance** (previously *Übersicht* / *Verwendungszweck*), and the remittance CSV export uses English headers (`#;Origin;Remittance info`).

### Accessibility
- Visible keyboard-focus rings on all interactive controls (buttons, list rows, tabs, filters).
- `prefers-reduced-motion` is now respected (animations and transitions are reduced).

## [2.0.0] - 2026-06-20

### Added
- New native Windows desktop app (`app/`) built with **Tauri + Rust + Svelte**, replacing the PowerShell/WinForms tool.
- Live, streaming validation log — files appear and update as they are validated.
- Click an error or warning to jump to its line in a syntax-highlighted XML viewer.
- Filter the log by severity (errors / warnings / all) and full-text search.
- System-aware light/dark theme with a manual toggle.
- Export validation results as TXT or CSV.
- Drag & drop files or folders (folders scanned recursively).
- Custom application icon.
- Resizable side panels (draggable gutters between the file list, viewer, and log).
- Pretty-printed XML in the viewer — readable even when the source is all on one line.
- Clicking an error scrolls its line to the center of the viewer and highlights it with a brief flash.
- Search within the open XML — `Ctrl+F` or a **Search** button opens a find panel that highlights matches and steps through them (next/previous).
- Collapse XML blocks in the viewer — fold arrows in the gutter fold individual elements, plus **Collapse all** / **Expand all** buttons.
- Per-file payment overview in an **Übersicht** tab: a creditor (`Cdtr`) block (name, IBAN, BIC, creditor identifier) and a `PmtInf` statistics table (block count, `NbOfTxs`, `CtrlSum`, `SvcLvl/Cd`, `LclInstrm`, `SeqTp`, execution/collection date) for pain.001 and pain.008.
- **Verwendungszweck** tab: one entry per transaction showing its origin (`InstrId`, falling back to `EndToEndId`) and remittance info (`Ustrd`), with a warning banner + red marker for empty/missing purposes, and a **CSV export** of the table.
- **Schemas… dialog** to manage XSD schemas: shows which are present/missing, imports `.xsd` files, folders, or `.zip` bundles into a per-user schema folder, opens that folder, and links to the official download source.
- Lean viewer mode for large XML files (over 10 MB): syntax highlighting and folding are disabled to keep scrolling, searching, and tab-switching responsive.

### Changed
- XSD validation engine moved from .NET (`System.Xml.Schema`) to **libxml2** (Rust `libxml` crate). Valid/invalid verdicts are equivalent; error message wording differs.
- XSD schemas are **no longer embedded** in the binary — they are loaded at runtime from a per-user folder and imported via the Schemas… dialog (so the app ships without the non-redistributable schemas).

### Fixed
- Switching from the Übersicht tab back to the XML tab no longer leaves the XML viewer empty (the viewer is kept mounted instead of being recreated).

## [1.0.0] - 2026-03-24

### Added
- Initial release: native Windows GUI SEPA XML Validator (PowerShell/WinForms) with full XSD validation, drag & drop, file/folder selection, batch validation, and TXT export.
- Bash CLI scripts for validation, batch validation, and renaming XML files by date/company/schema.

[Unreleased]: https://github.com/lkasdorf/SEPA-Validator/compare/v2.2.0...HEAD
[2.2.0]: https://github.com/lkasdorf/SEPA-Validator/compare/v2.1.0...v2.2.0
[2.1.0]: https://github.com/lkasdorf/SEPA-Validator/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/lkasdorf/SEPA-Validator/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/lkasdorf/SEPA-Validator/releases/tag/v1.0.0
