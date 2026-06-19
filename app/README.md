# SEPA Validator — Desktop App (Tauri + Rust + Svelte)

A modern Windows desktop validator for SEPA payment XML files. It validates
files against ISO 20022 / GBIC XSD schemas using **libxml2**, and shows
a live, clickable, filterable validation log.

- **Backend:** Rust (`src-tauri/`), XSD validation via the `libxml` crate (libxml2),
  namespace detection via `quick-xml`. Results stream to the UI over a Tauri
  `ipc::Channel` so the log fills in live during a run.
- **Frontend:** Svelte 5 + TypeScript + Vite (`src/`), with a CodeMirror 6 XML
  viewer that jumps to and highlights the offending line when you click an error.

## Features

- Validate via file picker, folder picker, or drag & drop (folders are scanned recursively).
- Live-streaming results list with status icons (✓ / ✗ / ⚠).
- Click any error/warning to jump to its line in the XML viewer.
- Filter the log by severity (errors / warnings / all) and full-text search.
- System-aware light/dark theme with a manual toggle.
- Export results as TXT or CSV.

## Prerequisites (Windows)

This app links the native **libxml2** library, so first-time setup needs a few
build tools. After this one-time setup, builds are fast.

1. **Node.js** (18+) and **npm**.
2. **Rust** (stable, MSVC toolchain) — https://rustup.rs.
3. **Visual Studio Build Tools 2022** with the C++ workload (provides `link.exe`).
4. **vcpkg + libxml2** (static lib, dynamic-CRT triplet):
   ```sh
   git clone --depth 1 https://github.com/microsoft/vcpkg "%USERPROFILE%\vcpkg"
   "%USERPROFILE%\vcpkg\bootstrap-vcpkg.bat"
   "%USERPROFILE%\vcpkg\vcpkg.exe" install libxml2:x64-windows-static-md
   ```
   (vcpkg downloads its own CMake/Ninja — a system CMake is not required.)
5. **libclang** (the `libxml` crate runs bindgen). The lightest no-admin option
   is the PyPI wheel:
   ```sh
   pip install libclang -t "%USERPROFILE%\libclang_pkg"
   ```
   This puts `libclang.dll` in `%USERPROFILE%\libclang_pkg\clang\native`.
   (Alternatively install LLVM, which also provides `libclang.dll`.)
6. **XSD schemas:** the ISO 20022 / GBIC `.xsd` files are **not redistributed**
   (download from iso20022.org / ebics.de) and are **not embedded** in the binary.
   After first launch, use the **Schemas…** toolbar button to import the `.xsd` files
   or a folder containing them — the app copies them to `app_data_dir()/schemas/`
   and loads them at runtime.

### Local build config (`src-tauri/.cargo/config.toml`)

The build needs to know where vcpkg and libclang live. This is machine-specific,
so it is **gitignored** — create `app/src-tauri/.cargo/config.toml` yourself:

```toml
[env]
VCPKG_ROOT = "C:/Users/<you>/vcpkg"
VCPKGRS_TRIPLET = "x64-windows-static-md"
LIBCLANG_PATH = "C:/Users/<you>/libclang_pkg/clang/native"
```

(`bcrypt.lib`, which libxml2 ≥ 2.15 needs for `BCryptGenRandom`, is linked
automatically by `src-tauri/build.rs`.)

## Develop & build

```sh
cd app
npm install

# Run in dev (opens the app window with hot-reload)
npx tauri dev

# Type-check the frontend
npm run check

# Backend tests (validates against fixtures under ../to_check when present)
cd src-tauri && cargo test

# Production build — standalone exe (no installer)
cd app && npx tauri build --no-bundle
# -> app/src-tauri/target/release/app.exe

# Production build with installers (NSIS/MSI)
cd app && npx tauri build
```

## Architecture notes

- `src-tauri/src/model.rs` — `ValidationResult` / `Status` / `Message` (serde DTOs).
- `src-tauri/src/schema.rs` — namespace → XSD filename map; no embedded bytes.
- `src-tauri/src/validator.rs` — `detect_namespace` (quick-xml) + `Validator` with a
  per-run compiled-schema cache; maps libxml `StructuredError` to located messages.
- `src-tauri/src/scanner.rs` — recursively expands files/folders to `.xml` paths.
- `src-tauri/src/commands.rs` — `start_validation` (streams `ValidationEvent`s over a
  Channel from a worker thread, since libxml types are not `Send`), `read_file`,
  `write_text_file`, `schema_status`, `import_schemas`, `open_schema_dir`.
- `src/lib/` — `api.ts` (typed invoke wrappers), `stores.ts`, and the components
  (`Toolbar`, `FileList`, `CodeViewer`, `LogPanel`, `SummaryBar`, `SchemaDialog`).
