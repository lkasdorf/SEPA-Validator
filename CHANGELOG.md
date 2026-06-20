# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/lkasdorf/SEPA-Validator/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/lkasdorf/SEPA-Validator/releases/tag/v1.0.0
