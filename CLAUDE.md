# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SEPA XML Validator: validates SEPA payment XML files against ISO 20022 XSD schemas, plus plausibility checks the schema can't express. Also serves as a local data curation workspace for triaging XML payment files.

The product is a **Tauri + Rust + Svelte** desktop app (`app/`, Windows-only) with a live, clickable, filterable validation log. Bash CLI scripts (`scripts/`) cover batch validation and renaming. The original PowerShell/WinForms tool was removed from master; it remains at tag `v1.0.0` (`windows/`).

## Key Commands

### Tauri/Rust App (on Windows): `app/`
```sh
cd app && npm install
npx tauri dev                      # run with hot-reload
npm run check && npm test          # svelte-check + vitest (frontend)
node scripts/third-party-notices.mjs   # regenerate app/THIRD-PARTY-NOTICES.txt after dependency changes (CI checks it)
cd src-tauri && cargo test         # backend tests; unit tests use the committed mini XSD in
                                   # tests/fixtures/, the 2 spike tests skip without private data
cd src-tauri && cargo fmt          # CI runs cargo fmt --check and clippy -D warnings
cd app && npx tauri build --no-bundle   # standalone exe -> src-tauri/target/release/app.exe
```
First-time native setup (vcpkg+libxml2, libclang, local `.cargo/config.toml`) is in `app/README.md`.

### CLI validation (Linux/macOS/WSL)
```bash
./scripts/validate.sh file.xml                           # Validate single file
./scripts/validate.sh path/to/folder/                    # Validate all XMLs in folder
./scripts/validate.sh --export report.txt --csv report.csv *.xml   # Reports (all errors per file)
./scripts/validate_all.sh to_check analysis              # Batch validate, write CSV report
./scripts/rename_xml_by_date_company_format.sh to_check analysis  # Rename by date/company/format
bash scripts/validate.test.sh                            # Tests for the scripts (needs xmllint + rg)
```

Prerequisites: `bash`, `xmllint` (always run with `--nonet`), `rg` (ripgrep, for validate_all.sh and the rename scripts; they stop with a clear message without it). WSL is not installed on this Windows machine, so the scripts and `scripts/validate.test.sh` are not run locally; their tests run only in CI (the ubuntu PII guard job).

## Architecture

### Tauri/Rust App (`app/`)

Tauri v2 backend (Rust, `src-tauri/`) + Svelte 5/TypeScript/Vite frontend (`src/`).

- **Backend modules** (`src-tauri/src/`):
  - `model`: serde DTOs `ValidationResult`/`Status`/`Message` (with an optional `hint`).
  - `schema`: namespace→XSD filename map plus `VARIANTS` (Swiss SPS, DK foreign AXZ, DK urgent euro CCU) for schemas that share the pain.001.001.09 namespace, picked via `resolve(ns, variant)`.
  - `formatting`: the meaning-preserving pretty-printer used for both the viewer and validation, so line numbers match. Leaf text stays verbatim; it rejects DOCTYPE, unclosed elements and non-UTF-8.
  - `validator`: `detect_namespace`, `detect_variant` (Swiss by debtor IBAN, else the DK subset from currency/ChrgBr/SvcLvl), and `Validator`.
    - If the guessed DK subset fails but another DK subset of the namespace accepts the file, that one is used (`dk_candidates`).
    - Strict parse via `xmlCtxtReadMemory` (no recover, NONET, BIG_LINES).
    - `xmlSchemaValidateDoc` called directly, because the crate's wrapper panics.
    - A per-run schema cache, loaded from `app_data_dir()/schemas/`.
  - `messages`: strips `{namespace}` prefixes and adds plain-language hints.
  - `plausibility`: warnings after XSD validation:
    - NbOfTxs/CtrlSum vs. the actual transactions
    - IBAN mod-97
    - past dates (DK `1999-01-01` = as soon as possible)
    - duplicate EndToEndId
  - `payments`: the Overview/Remittance summary.
  - `statements`: camt.052/053/054 (Rpt/Stmt/Ntfctn): plausibility (opening + booked entries = CLBD, TxsSummry counts/sums/net) and the statement Overview; entries become Transactions rows with booking date and CRDT/DBIT.
  - `statement_gen`: synthetic camt.053.001.08 statements from a pain.001/pain.008 ("as if the bank had booked it"): one statement per account and booking day, days chained (CLBD → next OPBD), EUR only, bank-side IDs marked `SYNTH-`. Problem files are refused with reasons, never cleaned up.
  - `scanner`: recursive `.xml` expansion.
  - `commands`: the IPC surface:
    - Validation: `start_validation`/`cancel_validation` with `RunState`; `run_batch` survives panics.
    - Reading: `read_formatted`, `read_payment_summary`.
    - Statements: `preview_statements`, `generate_statements` (writes `<stem>_camt053[_<account tag>]_YYYYMMDD.xml` next to the source (tag = last 4 IBAN chars, longer if two accounts share them; local drives only) via `check_export_path` + `create_new`, `_1`, `_2` on collisions).
    - Writing: `write_text_file` and `save_formatted` (guarded by `check_export_path`).
    - Editing: `validate_edited` (validates the viewer's text in memory and returns it re-indented, so lines match) and `save_edited` (writes `NAME_editN.xml` next to the original with `create_new`, then validates it). Frontend: `lib/drafts.ts` (per-file unsaved edits) and `lib/editing.ts` (Validate/Save copy/Discard).
    - Schemas: `schema_status`, `import_schemas` (ZIP: safe names, 10 MB cap), `open_schema_dir`.
    - Links: `open_url` (https allowlist).
- **Live streaming**: `start_validation` runs on a worker thread (libxml types are not `Send`) and streams `ValidationEvent`s (started/result/finished) over a `tauri::ipc::Channel`. The frontend (`lib/validation.ts`) ignores events from superseded runs. The viewer and summary loads use request tokens (`lib/latest.ts`).
- **Security**: strict CSP in `tauri.conf.json`. Style-src nonces are disabled so CodeMirror's inline styles work.
- **Native build deps** (one-time, documented in `app/README.md`):
  - vcpkg `libxml2[core,zlib]:x64-windows-static-md`. No iconv, to avoid statically linking LGPL libiconv.
  - `libclang` (PyPI wheel) for bindgen.
  - A **gitignored** `src-tauri/.cargo/config.toml` with `[env]` (`VCPKG_ROOT`, `VCPKGRS_TRIPLET`, `LIBCLANG_PATH`).
  - `build.rs` links `bcrypt` (libxml2 ≥ 2.15 needs `BCryptGenRandom`).
  - XSDs are not embedded; they are imported via the **Schemas…** dialog.

## Data Directories (gitignored)

- `xml_schema/`: XSD schemas (not redistributable; download from iso20022.org or ebics.de)
- `to_check/`: XML files sorted into `inbox/`, `valid/`, `invalid/`, `duplicates/`, `archive/`
- `analysis/`: generated CSV/Markdown validation reports

## CI

`.github/workflows/ci.yml` runs on every PR and push to master:
- **PII guard** (ubuntu): `scripts/pii-guard.sh <base>` fails on any `.xml`/`.xsd` outside `app/src-tauri/tests/fixtures/`, including files added and later deleted within the PR. Never commit payment files or ISO/SIX schemas; put synthetic test data in `tests/fixtures/`. The job also runs the Node script tests and `scripts/validate.test.sh`.
- **Build & test** (windows):
  - Frontend: `npm run check`, `npm test`, `npm run build`.
  - Rust: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test`.
  - Notices: the freshness check for `THIRD-PARTY-NOTICES.txt`.
  - libxml2 comes from vcpkg pinned to the same commit as the local setup and is cached as a vcpkg binary package.
- **Release** (`release.yml`, on tag `v*`):
  - `tauri build --no-bundle` + `tauri bundle --no-sign`, then a draft release (via `gh`) with the installer, `.sig`, `latest.json`, the portable exe and the notices. `scripts/release-assets.mjs` patches the NSIS bundle type into app.exe before signing and writes `latest.json`.
  - Authenticode signing is prepared for SignPath (two signing requests: binaries, then installer) and is on only when the repo variable `SIGNPATH_ORGANIZATION_ID` is set. It is currently off: the SignPath Foundation application was declined (2026-11, too little public visibility), so releases are unsigned. Policy: `CODE_SIGNING_POLICY.md`; setup and artifact configurations: `RELEASING.md`.
  - The version has a single source: `"version"` in `app/src-tauri/tauri.conf.json`. Bump it with `node scripts/bump-version.mjs X.Y.Z`.
  - The full procedure, the environment and signing-key handling are in `RELEASING.md`.
- **WinGet** (`winget.yml`, on release `released`): Komac (pinned, checksum checked) opens a PR at microsoft/winget-pkgs that updates `LeonKasdorf.SEPAValidator`. It needs the `WINGET_TOKEN` secret (classic token, `public_repo`) in the `winget` environment.
- `claude-review` (separate workflow) only posts a review comment; a green check does not always mean it reviewed.

## Conventions

- Commit format: `type(scope): short summary` (e.g., `fix(validator): ...`, `feat(app): ...`, `docs: ...`)
- Shell scripts use `set -euo pipefail` and LF line endings (`.gitattributes`)
- XML file naming: `YYYYMMDD_COMPANY_FORMAT.xml` with `_1`, `_2` on collisions
- Validation reports are kept for traceability; never delete prior timestamped reports
