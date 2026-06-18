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

### Changed
- XSD validation engine moved from .NET (`System.Xml.Schema`) to **libxml2** (Rust `libxml` crate). Valid/invalid verdicts are equivalent; error message wording differs.

## [1.0.0] - 2026-03-24

### Added
- Initial release: native Windows GUI SEPA XML Validator (PowerShell/WinForms) with full XSD validation, drag & drop, file/folder selection, batch validation, and TXT export.
- Bash CLI scripts for validation, batch validation, and renaming XML files by date/company/schema.

[Unreleased]: https://github.com/lkasdorf/SEPA-Validator/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/lkasdorf/SEPA-Validator/releases/tag/v1.0.0
