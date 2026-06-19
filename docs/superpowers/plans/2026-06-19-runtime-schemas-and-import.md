# Runtime Schemas + Import (Teil 1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop embedding the ISO 20022 / GBIC XSDs in the binary; load them at runtime from a per-user schema directory, and add a Schemas… dialog to view status and import `.xsd` files/folders.

**Architecture:** `schema.rs` keeps only the namespace→filename map (no `include_bytes!`); the `Validator` reads/compiles each XSD from a schema directory (`app_data_dir()/schemas/`) via libxml `SchemaParserContext::from_file`. New Tauri commands report schema status, import `.xsd` files, and open the folder. A Svelte `SchemaDialog` drives import; the toolbar shows an "N/M" badge.

**Tech Stack:** Rust + Tauri v2 (`tauri::Manager` path API), `libxml`; Svelte 5 + TypeScript + Vite, `@tauri-apps/plugin-dialog`.

## Global Constraints

- XSDs are NOT embedded. The 8 known schemas (namespace, filename) stay listed in `schema.rs`; bytes/`include_bytes!` are removed. `lookup(ns) -> Option<&'static str>` returns the filename.
- Schema directory = `app.path().app_data_dir()?.join("schemas")`, created with `create_dir_all`. Requires `use tauri::Manager;`.
- The `Validator` loads schemas from this directory; a known namespace whose file is absent yields the existing `Status::NoSchema` with a clear "not imported" message (no new status). Unknown namespace → `NoSchema` as before.
- Compile via `SchemaParserContext::from_file(path_str)` (proven by `tests/spike.rs`), not `from_buffer`.
- Import accepts `.xsd` files (multi-select) and a folder (top-level `.xsd` only). No ZIP. Extension match is case-insensitive. Copies into the schema dir, overwriting same-named files.
- "Open folder" is a Rust command using `std::process::Command::new("explorer")` (Windows-only). No new plugin. App-defined commands need no capability entry.
- Serde DTOs use `#[serde(rename_all = "camelCase")]`; TS types mirror exactly.
- Backend verification `cargo test` (schema-compiling validator tests skip when the local `xml_schema/` dir is absent); frontend `npm run check` (0 errors/0 warnings).
- Commit format: `type(scope): summary`.

---

### Task 1: Backend de-embed + validation wiring

Remove embedding from `schema.rs`/`build.rs`, make the `Validator` load from a schema directory, resolve that directory in `start_validation`, and update the validator tests. End state: `cargo test` green; the app validates using the runtime schema dir (which is empty until the user imports — that is expected and handled by Tasks 2-3).

**Files:**
- Modify (rewrite): `app/src-tauri/src/schema.rs`
- Modify (rewrite): `app/src-tauri/build.rs`
- Modify: `app/src-tauri/src/validator.rs`
- Modify: `app/src-tauri/src/commands.rs` (add `schema_dir` helper; update `start_validation`)

**Interfaces:**
- Produces: `schema::lookup(&str) -> Option<&'static str>`; `schema::known_schemas() -> &'static [(&'static str, &'static str)]`; `Validator::new(schema_dir: PathBuf)`; `commands::schema_dir(app: &AppHandle) -> Result<PathBuf, String>` (consumed by Task 2).

- [ ] **Step 1: Rewrite `schema.rs` (drop embedding)**

Overwrite `app/src-tauri/src/schema.rs` with:

```rust
//! ISO 20022 / GBIC schema namespace → XSD filename map.
//! Schemas are NOT embedded; they are loaded at runtime from the app's schema
//! directory and imported by the user (legal redistribution constraint).

/// Ordered namespace -> expected XSD filename (looked up in the schema dir).
pub const SCHEMAS: &[(&str, &str)] = &[
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.03", "pain.001.001.03.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.09", "pain.001.001.09.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.002.001.10", "pain.002.001.10.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.007.001.09", "pain.007.001.09_GBIC_5.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02", "pain.008.001.02.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.08", "pain.008.001.08.xsd"),
    ("urn:iso:std:iso:20022:tech:xsd:camt.054.001.08", "camt.054.001.08.xsd"),
    ("urn:conxml:xsd:container.nnn.001.GBIC4", "container.nnn.001.GBIC4.xsd"),
];

/// Returns the XSD filename for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<&'static str> {
    SCHEMAS.iter().find(|(ns, _)| *ns == namespace).map(|(_, name)| *name)
}

/// All known (namespace, filename) pairs, for the schema-status UI.
pub fn known_schemas() -> &'static [(&'static str, &'static str)] {
    SCHEMAS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_known_namespace_returns_filename() {
        assert_eq!(
            lookup("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"),
            Some("pain.008.001.02.xsd")
        );
    }

    #[test]
    fn lookup_unknown_namespace_is_none() {
        assert!(lookup("urn:made:up").is_none());
    }

    #[test]
    fn known_schemas_lists_all_with_xsd_filenames() {
        let all = known_schemas();
        assert_eq!(all.len(), 8);
        for (ns, name) in all {
            assert!(!ns.is_empty());
            assert!(name.ends_with(".xsd"));
        }
    }
}
```

- [ ] **Step 2: Rewrite `build.rs` (drop the XSD-existence check)**

Overwrite `app/src-tauri/build.rs` with:

```rust
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    // libxml2 (>= 2.15) needs BCryptGenRandom from bcrypt.lib on Windows.
    println!("cargo:rustc-link-lib=dylib=bcrypt");
    tauri_build::build()
}
```

- [ ] **Step 3: `validator.rs` — load schemas from a directory**

In `app/src-tauri/src/validator.rs`:

(a) Change the import `use std::path::Path;` to:

```rust
use std::path::{Path, PathBuf};
```

(b) Replace the struct + `Default` + `new` block:

```rust
pub struct Validator {
    cache: HashMap<&'static str, SchemaValidationContext>,
}

impl Default for Validator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }
```

with (the `Default` impl is removed):

```rust
pub struct Validator {
    schema_dir: PathBuf,
    cache: HashMap<&'static str, SchemaValidationContext>,
}

impl Validator {
    /// `schema_dir` holds the imported XSD files looked up by filename.
    pub fn new(schema_dir: PathBuf) -> Self {
        Self {
            schema_dir,
            cache: HashMap::new(),
        }
    }
```

(c) Replace the `schema::lookup` block (the `let (schema_name, _bytes) = match schema::lookup(&ns) { ... };` through the line just before `if !self.cache.contains_key(schema_name) {`) with:

```rust
        let schema_name = match schema::lookup(&ns) {
            Some(name) => name,
            None => {
                return mk(
                    ns.clone(),
                    String::new(),
                    vec![Message {
                        severity: Severity::Warning,
                        text: format!("No matching schema for namespace: {ns}"),
                        line: None,
                        column: None,
                    }],
                    Status::NoSchema,
                    0,
                    1,
                )
            }
        };

        if !self.schema_dir.join(schema_name).exists() {
            return mk(
                ns.clone(),
                schema_name.to_string(),
                vec![Message {
                    severity: Severity::Warning,
                    text: format!("Schema '{schema_name}' not imported. Open Schemas… to import it."),
                    line: None,
                    column: None,
                }],
                Status::NoSchema,
                0,
                1,
            );
        }
```

(d) Replace `compile`:

```rust
    fn compile(&self, namespace: &str) -> Result<SchemaValidationContext, String> {
        let (_name, bytes) = schema::lookup(namespace).ok_or("schema not found")?;
        let mut parser = SchemaParserContext::from_buffer(bytes);
        SchemaValidationContext::from_parser(&mut parser)
            .map_err(|errs| format!("Failed to load schema: {} error(s)", errs.len()))
    }
```

with:

```rust
    fn compile(&self, namespace: &str) -> Result<SchemaValidationContext, String> {
        let filename = schema::lookup(namespace).ok_or("schema not found")?;
        let path = self.schema_dir.join(filename);
        let path_str = path.to_str().ok_or("schema path is not valid UTF-8")?;
        let mut parser = SchemaParserContext::from_file(path_str);
        SchemaValidationContext::from_parser(&mut parser)
            .map_err(|errs| format!("Failed to load schema: {} error(s)", errs.len()))
    }
```

- [ ] **Step 4: Update the validator tests for the new constructor**

In `validator.rs`, after the existing `fn repo_root() -> std::path::PathBuf { ... }` helper in the test module, add:

```rust
    /// Local XSD directory (never committed); tests that compile schemas skip if absent.
    fn test_schema_dir() -> std::path::PathBuf {
        repo_root().join("xml_schema")
    }
```

Change `unknown_namespace_yields_no_schema` — replace `let mut v = super::Validator::new();` with:

```rust
        let mut v = super::Validator::new(test_schema_dir());
```

In each of `valid_fixture_is_ok`, `invalid_fixture_reports_errors`, and `invalid_fixture_lines_point_into_formatted_text`, replace the existing skip guard:

```rust
        if !f.exists() {
            eprintln!("SKIP: fixture absent");
            return;
        }
```

with (note: `invalid_fixture_lines_point_into_formatted_text` has the same `if !f.exists()` guard — apply there too):

```rust
        if !f.exists() || !test_schema_dir().exists() {
            eprintln!("SKIP: fixture or schema dir absent");
            return;
        }
```

and replace each `let mut v = super::Validator::new();` in those three tests with:

```rust
        let mut v = super::Validator::new(test_schema_dir());
```

- [ ] **Step 5: `commands.rs` — schema dir helper + wire `start_validation`**

In `app/src-tauri/src/commands.rs`, change the `use tauri::ipc::Channel;` import to:

```rust
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager};
```

Add this helper (e.g. directly above `start_validation`):

```rust
/// The per-user directory that holds imported XSD schema files (created if missing).
pub fn schema_dir(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("schemas");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
```

Change `start_validation`'s signature and `Validator::new()` call. Replace:

```rust
#[tauri::command]
pub fn start_validation(paths: Vec<String>, on_event: Channel<ValidationEvent>) {
    let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
    let total = files.len();

    // libxml types are not Send: build the Validator inside the thread.
    std::thread::spawn(move || {
        let _ = on_event.send(ValidationEvent::Started { total });
        let mut validator = Validator::new();
```

with:

```rust
#[tauri::command]
pub fn start_validation(app: AppHandle, paths: Vec<String>, on_event: Channel<ValidationEvent>) {
    let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
    let total = files.len();
    let dir = schema_dir(&app).unwrap_or_default();

    // libxml types are not Send: build the Validator inside the thread.
    std::thread::spawn(move || {
        let _ = on_event.send(ValidationEvent::Started { total });
        let mut validator = Validator::new(dir);
```

- [ ] **Step 6: Build + test**

Run: `cd app/src-tauri && cargo test`
Expected: compiles; all tests pass. The schema-compiling validator tests (`valid_fixture_is_ok`, the two invalid-fixture tests) pass if the local `xml_schema/` dir and fixtures exist, otherwise print `SKIP` and pass. No warnings.

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/schema.rs app/src-tauri/build.rs app/src-tauri/src/validator.rs app/src-tauri/src/commands.rs
git commit -m "feat(app): load XSD schemas at runtime from app data dir (de-embed)"
```

---

### Task 2: Schema status / import / open-folder commands

Add the backend commands the dialog needs, plus a pure, testable copy helper.

**Files:**
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `schema_dir(app)` and `schema::known_schemas()` (Task 1).
- Produces (serde camelCase):
  - `SchemaInfo { namespace: String, filename: String, present: bool }`
  - `ImportResult { imported: u32, skipped: Vec<String> }`
  - commands `schema_status(app) -> Result<Vec<SchemaInfo>, String>`, `import_schemas(app, paths: Vec<String>) -> Result<ImportResult, String>`, `open_schema_dir(app) -> Result<(), String>`.
  - `copy_xsds(paths: &[String], dest: &Path) -> ImportResult` (pure helper).

- [ ] **Step 1: Add DTOs, the copy helper, the commands, and helper tests**

In `app/src-tauri/src/commands.rs`, add `use std::path::Path;` to the imports if not already present (it imports `std::path::PathBuf`; add `Path`):

```rust
use std::path::{Path, PathBuf};
```

(Replace the existing `use std::path::PathBuf;` line with the line above.)

Append at the end of the file (before any trailing `#[cfg(test)]` module; if none exists, this includes one):

```rust
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchemaInfo {
    pub namespace: String,
    pub filename: String,
    pub present: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: u32,
    pub skipped: Vec<String>,
}

/// Return the known schemas with a present/absent flag for the schema dir.
#[tauri::command]
pub fn schema_status(app: AppHandle) -> Result<Vec<SchemaInfo>, String> {
    let dir = schema_dir(&app)?;
    Ok(crate::schema::known_schemas()
        .iter()
        .map(|(ns, filename)| SchemaInfo {
            namespace: (*ns).to_string(),
            filename: (*filename).to_string(),
            present: dir.join(filename).exists(),
        })
        .collect())
}

fn is_xsd(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("xsd"))
        .unwrap_or(false)
}

fn copy_one(src: &Path, dest: &Path, imported: &mut u32, skipped: &mut Vec<String>) {
    match src.file_name() {
        Some(name) if std::fs::copy(src, dest.join(name)).is_ok() => *imported += 1,
        _ => skipped.push(src.display().to_string()),
    }
}

/// Copy `.xsd` files from the given paths (files or directories) into `dest`.
/// Pure (no AppHandle) for testability.
pub fn copy_xsds(paths: &[String], dest: &Path) -> ImportResult {
    let mut imported = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    for p in paths {
        let path = Path::new(p);
        if path.is_dir() {
            match std::fs::read_dir(path) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        let ep = entry.path();
                        if ep.is_file() && is_xsd(&ep) {
                            copy_one(&ep, dest, &mut imported, &mut skipped);
                        }
                    }
                }
                Err(_) => skipped.push(p.clone()),
            }
        } else if path.is_file() && is_xsd(path) {
            copy_one(path, dest, &mut imported, &mut skipped);
        } else {
            skipped.push(p.clone());
        }
    }
    ImportResult { imported, skipped }
}

/// Copy selected `.xsd` files/folders into the schema dir.
#[tauri::command]
pub fn import_schemas(app: AppHandle, paths: Vec<String>) -> Result<ImportResult, String> {
    let dir = schema_dir(&app)?;
    Ok(copy_xsds(&paths, &dir))
}

/// Open the schema dir in the OS file explorer (Windows).
#[tauri::command]
pub fn open_schema_dir(app: AppHandle) -> Result<(), String> {
    let dir = schema_dir(&app)?;
    std::process::Command::new("explorer")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fresh_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_file(p: &Path, content: &str) {
        let mut f = std::fs::File::create(p).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn copies_xsd_file_and_skips_non_xsd() {
        let src = fresh_dir("sepa_imp_src1");
        let dest = fresh_dir("sepa_imp_dest1");
        write_file(&src.join("pain.001.001.03.xsd"), "<xsd/>");
        write_file(&src.join("notes.txt"), "x");
        let xsd = src.join("pain.001.001.03.xsd").display().to_string();
        let txt = src.join("notes.txt").display().to_string();
        let r = copy_xsds(&[xsd, txt.clone()], &dest);
        assert_eq!(r.imported, 1);
        assert_eq!(r.skipped, vec![txt]);
        assert!(dest.join("pain.001.001.03.xsd").exists());
    }

    #[test]
    fn copies_all_xsd_from_directory_case_insensitive() {
        let src = fresh_dir("sepa_imp_src2");
        let dest = fresh_dir("sepa_imp_dest2");
        write_file(&src.join("a.xsd"), "<a/>");
        write_file(&src.join("b.XSD"), "<b/>");
        write_file(&src.join("c.txt"), "c");
        let r = copy_xsds(&[src.display().to_string()], &dest);
        assert_eq!(r.imported, 2);
        assert!(dest.join("a.xsd").exists());
        assert!(dest.join("b.XSD").exists());
        assert!(!dest.join("c.txt").exists());
    }
}
```

- [ ] **Step 2: Register the new commands**

In `app/src-tauri/src/lib.rs`, the handler list is:

```rust
        .invoke_handler(tauri::generate_handler![
            commands::start_validation,
            commands::read_file,
            commands::write_text_file,
            commands::read_formatted,
            commands::read_payment_summary
        ])
```

Replace it with (note the comma after `read_payment_summary`):

```rust
        .invoke_handler(tauri::generate_handler![
            commands::start_validation,
            commands::read_file,
            commands::write_text_file,
            commands::read_formatted,
            commands::read_payment_summary,
            commands::schema_status,
            commands::import_schemas,
            commands::open_schema_dir
        ])
```

- [ ] **Step 3: Build + test**

Run: `cd app/src-tauri && cargo test`
Expected: all tests pass including the two new `copy_xsds` tests; no warnings.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs
git commit -m "feat(app): schema status, import and open-folder commands"
```

---

### Task 3: Schemas dialog (frontend) + docs

Add the TS types, api wrappers, the open-state store, the `SchemaDialog`, the toolbar entry point with a status badge, and update the docs.

**Files:**
- Modify: `app/src/lib/types.ts`
- Modify: `app/src/lib/api.ts`
- Modify: `app/src/lib/stores.ts`
- Create: `app/src/lib/SchemaDialog.svelte`
- Modify: `app/src/lib/Toolbar.svelte`
- Modify: `app/src/App.svelte`
- Modify: `CLAUDE.md`
- Modify: `app/README.md`

**Interfaces:**
- Consumes: commands `schema_status`, `import_schemas`, `open_schema_dir` (Task 2); `@tauri-apps/plugin-dialog` `open`.
- Produces: nothing for later tasks (final task).

- [ ] **Step 1: Add TS types**

In `app/src/lib/types.ts`, append:

```ts
export interface SchemaInfo {
  namespace: string;
  filename: string;
  present: boolean;
}

export interface ImportResult {
  imported: number;
  skipped: string[];
}
```

- [ ] **Step 2: Add api wrappers**

In `app/src/lib/api.ts`, change the type import line to also import the new types:

```ts
import type { ValidationEvent, ValidationResult, PaymentSummary, SchemaInfo, ImportResult } from "./types";
```

Then add at the end of the file:

```ts
export function schemaStatus(): Promise<SchemaInfo[]> {
  return invoke<SchemaInfo[]>("schema_status");
}

export function importSchemas(paths: string[]): Promise<ImportResult> {
  return invoke<ImportResult>("import_schemas", { paths });
}

export function openSchemaDir(): Promise<void> {
  return invoke("open_schema_dir");
}
```

- [ ] **Step 3: Add the open-state store**

In `app/src/lib/stores.ts`, append:

```ts
export const schemaDialogOpen = writable<boolean>(false);
```

- [ ] **Step 4: Create SchemaDialog.svelte**

Create `app/src/lib/SchemaDialog.svelte`:

```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { schemaStatus, importSchemas, openSchemaDir } from "./api";
  import { schemaDialogOpen } from "./stores";
  import type { SchemaInfo } from "./types";

  let rows: SchemaInfo[] = [];
  let busy = false;
  let note = "";

  onMount(refresh);

  async function refresh() {
    try {
      rows = await schemaStatus();
    } catch {
      note = "Status konnte nicht geladen werden.";
    }
  }

  async function importFiles() {
    const sel = await openDialog({ multiple: true, filters: [{ name: "XSD", extensions: ["xsd"] }] });
    if (!sel) return;
    await runImport(Array.isArray(sel) ? sel : [sel]);
  }

  async function importFolder() {
    const sel = await openDialog({ directory: true });
    if (!sel) return;
    await runImport([sel as string]);
  }

  async function runImport(paths: string[]) {
    busy = true;
    note = "";
    try {
      const r = await importSchemas(paths);
      note = `${r.imported} XSD-Datei(en) importiert${r.skipped.length ? `, ${r.skipped.length} übersprungen` : ""}.`;
      await refresh();
    } catch {
      note = "Import fehlgeschlagen.";
    }
    busy = false;
  }

  function close() {
    schemaDialogOpen.set(false);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Schemas">
    <header>
      <strong>Schemas</strong>
      <button class="x" on:click={close} aria-label="Schließen">✕</button>
    </header>
    <p class="hint">Die XSDs werden nicht mitgeliefert. Importiere die ISO-20022/GBIC-Schemas, um zu validieren.</p>
    <div class="tablewrap">
      <table>
        <thead><tr><th>Namespace</th><th>Datei</th><th>Status</th></tr></thead>
        <tbody>
          {#each rows as r}
            <tr>
              <td class="ns">{r.namespace}</td>
              <td>{r.filename}</td>
              <td class={r.present ? "ok" : "missing"}>{r.present ? "✓ vorhanden" : "✗ fehlt"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if note}<p class="note">{note}</p>{/if}
    <footer>
      <button on:click={importFiles} disabled={busy}>XSD-Dateien…</button>
      <button on:click={importFolder} disabled={busy}>Ordner…</button>
      <button on:click={openSchemaDir}>Ordner öffnen</button>
      <button class="close" on:click={close}>Schließen</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .dialog {
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: min(720px, 92vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    padding: 14px 16px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .hint { opacity: 0.8; font-size: 12px; margin: 6px 0 10px; }
  .tablewrap { overflow: auto; min-height: 0; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); }
  td.ns { word-break: break-all; }
  td.ok { color: var(--ok); }
  td.missing { color: var(--err); }
  .note { font-size: 12px; margin: 8px 0 0; }
  footer { display: flex; gap: 8px; margin-top: 12px; }
  footer button { background: var(--accent); color: #fff; border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 12px; }
  footer button:hover:not(:disabled) { filter: brightness(1.1); }
  footer button:disabled { opacity: 0.45; cursor: default; }
  footer button.close { margin-left: auto; background: transparent; color: var(--fg); border: 1px solid var(--border); }
</style>
```

- [ ] **Step 5: Toolbar entry point + status badge**

In `app/src/lib/Toolbar.svelte`, add to the imports (after the existing imports in the `<script>`):

```ts
  import { schemaDialogOpen } from "./stores";
  import { schemaStatus } from "./api";
```

Then add the badge state + refresh, after the existing `function doExportCsv()` line:

```ts
  let schemaPresent = 0;
  let schemaTotal = 0;
  async function refreshSchemaBadge() {
    try {
      const s = await schemaStatus();
      schemaPresent = s.filter((x) => x.present).length;
      schemaTotal = s.length;
    } catch {
      schemaPresent = 0;
      schemaTotal = 0;
    }
  }
  $: if (!$schemaDialogOpen) refreshSchemaBadge();
```

In the toolbar markup, add a button after the `Export CSV` button:

```svelte
  <button on:click={() => schemaDialogOpen.set(true)}>Schemas… {schemaTotal ? `(${schemaPresent}/${schemaTotal})` : ""}</button>
```

- [ ] **Step 6: Mount the dialog in App.svelte**

In `app/src/App.svelte`, add to the component imports (after `import SummaryBar from "./lib/SummaryBar.svelte";`):

```ts
  import SchemaDialog from "./lib/SchemaDialog.svelte";
```

Add to the store import — change:

```ts
  import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./lib/stores";
```

to:

```ts
  import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer, schemaDialogOpen } from "./lib/stores";
```

Then, just before the closing `</div>` of `<div class="app">` (after `<SummaryBar />`), add:

```svelte
  {#if $schemaDialogOpen}<SchemaDialog />{/if}
```

- [ ] **Step 7: Update the docs**

In `CLAUDE.md`, replace:

```
- **Backend modules** (`src-tauri/src/`): `model` (serde DTOs `ValidationResult`/`Status`/`Message`), `schema` (namespace→XSD map, schemas embedded via `include_bytes!`), `validator` (`detect_namespace` via quick-xml + `Validator` with a per-run compiled-schema cache, mapping libxml `StructuredError` to located messages), `scanner` (recursive `.xml` expansion), `commands` (`start_validation`, `read_file`, `write_text_file`).
```

with:

```
- **Backend modules** (`src-tauri/src/`): `model` (serde DTOs `ValidationResult`/`Status`/`Message`), `schema` (namespace→XSD filename map; schemas are NOT embedded — loaded at runtime from the per-user schema dir), `validator` (`detect_namespace` via quick-xml + `Validator::new(schema_dir)` with a per-run compiled-schema cache, mapping libxml `StructuredError` to located messages; a known namespace whose XSD is not imported yields `NoSchema`), `scanner` (recursive `.xml` expansion), `commands` (`start_validation`, `read_file`, `write_text_file`, `read_formatted`, `read_payment_summary`, `schema_status`, `import_schemas`, `open_schema_dir`).
```

Also in `CLAUDE.md`, replace:

```
- **Native build deps** (one-time, documented in `app/README.md`): vcpkg `libxml2:x64-windows-static-md`, `libclang` (PyPI wheel) for bindgen, and a **gitignored** `src-tauri/.cargo/config.toml` with `[env]` (`VCPKG_ROOT`, `VCPKGRS_TRIPLET`, `LIBCLANG_PATH`). `build.rs` links `bcrypt` (libxml2 ≥ 2.15 needs `BCryptGenRandom`) and verifies the mapped XSDs exist in `xml_schema/`.
```

with:

```
- **Native build deps** (one-time, documented in `app/README.md`): vcpkg `libxml2:x64-windows-static-md`, `libclang` (PyPI wheel) for bindgen, and a **gitignored** `src-tauri/.cargo/config.toml` with `[env]` (`VCPKG_ROOT`, `VCPKGRS_TRIPLET`, `LIBCLANG_PATH`). `build.rs` links `bcrypt` (libxml2 ≥ 2.15 needs `BCryptGenRandom`). XSDs are no longer embedded; the app loads them at runtime from `app_data_dir()/schemas/`, imported via the **Schemas…** dialog.
```

In `app/README.md`: find any sentence stating that the XSDs are embedded or that they must be present in `xml_schema/` for the build to succeed, and reword it to: "XSDs are not embedded. The app loads them at runtime from the per-user schema directory (`app_data_dir()/schemas/`); import the ISO 20022 / GBIC XSDs via the **Schemas…** dialog." (Use Grep for `include_bytes`, `xml_schema`, or `embed` in `app/README.md` to locate the spots; if none exist, add the sentence under the build-prerequisites section.)

- [ ] **Step 8: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/types.ts app/src/lib/api.ts app/src/lib/stores.ts app/src/lib/SchemaDialog.svelte app/src/lib/Toolbar.svelte app/src/App.svelte CLAUDE.md app/README.md
git commit -m "feat(app): Schemas dialog (status + import) and docs"
```

---

## Self-Review

**Spec coverage:**
- De-embed (no `include_bytes!`; namespace→filename map) → Task 1 schema.rs.
- build.rs drops the XSD-existence check → Task 1 build.rs.
- Runtime load from `app_data_dir()/schemas/` via `from_file` → Task 1 validator.rs `compile` + commands.rs `schema_dir`.
- Missing known schema → `NoSchema` with "not imported" message → Task 1 validator.rs.
- `start_validation` resolves the schema dir (AppHandle) → Task 1 commands.rs.
- `schema_status` / `import_schemas` (.xsd files + folder, case-insensitive) / `open_schema_dir` → Task 2.
- Schemas… dialog (status table, import files/folder, open folder) + toolbar badge → Task 3.
- DTO camelCase ↔ TS → Task 2 serde + Task 3 types.
- Tests skip without local XSDs; `copy_xsds` tested → Task 1 (validator tests) + Task 2 (`copy_xsds` tests).
- Docs updated → Task 3.
- YAGNI exclusions (ZIP, auto-download, configurable path, content validation, cross-platform open) → not implemented. ✓

**Placeholder scan:** No TBD/TODO. The README step is a concrete grep-and-reword instruction (its exact current text is unknown to this plan), not a deferred placeholder. Every code step has complete code. ✓

**Type consistency:** Rust `schema::lookup -> Option<&'static str>` and `known_schemas` used by Task 2 `schema_status`. `Validator::new(PathBuf)` defined in Task 1 and called in Task 1 (`start_validation`, tests). `SchemaInfo`/`ImportResult` serde fields (`namespace/filename/present`, `imported/skipped`) match the TS interfaces (Task 3) and api wrappers. Command names `schema_status`/`import_schemas`/`open_schema_dir` match between `commands.rs`, `lib.rs` registration, and `api.ts` invoke strings. Store `schemaDialogOpen` matches across `stores.ts`, `Toolbar.svelte`, `App.svelte`, `SchemaDialog.svelte`. ✓
