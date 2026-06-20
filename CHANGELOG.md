# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/lkasdorf/SEPA-Validator/compare/v2.1.0...HEAD
[2.1.0]: https://github.com/lkasdorf/SEPA-Validator/compare/v2.0.0...v2.1.0
[2.0.0]: https://github.com/lkasdorf/SEPA-Validator/compare/v1.0.0...v2.0.0
[1.0.0]: https://github.com/lkasdorf/SEPA-Validator/releases/tag/v1.0.0
