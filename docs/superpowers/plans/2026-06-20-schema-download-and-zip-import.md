# Schema Download Helper + ZIP Import (Teil 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Herunterladen…" button that opens the official schema source in the browser, and let the schema import accept ZIP bundles (extracting the contained `.xsd`).

**Architecture:** The backend gains a pure-Rust `zip` dependency, an `extract_zip_xsds` helper used inside `copy_xsds`, and an `open_url` command. The Schemas dialog broadens its file picker to `.xsd`+`.zip` and adds a download button. No scraping — the download button is a browser opener.

**Tech Stack:** Rust + Tauri v2, `zip` (pure-Rust deflate via miniz_oxide); Svelte 5 + TypeScript, `@tauri-apps/plugin-dialog`.

## Global Constraints

- ZIP import: a selected `.zip` → its `.xsd` entries (flattened to basename — prevents zip-slip) written into the schema dir; `.xsd` files and folders unchanged. Extension checks case-insensitive.
- `open_url(url)` opens the URL in the default browser (Windows, `explorer <url>`), registered in `lib.rs`. Download button opens `https://www.ebics.de/de/datenformate`.
- File picker accepts `extensions: ["xsd", "zip"]` (no separate ZIP button).
- `zip` dependency must be pure-Rust (no new C build deps): `zip = { version = "2", default-features = false, features = ["deflate"] }`.
- No auto-download/scraping; no recursive ZIP discovery in folders; no XSD content validation (YAGNI). Windows-only `open_url` (like `open_schema_dir`).
- Backend `cargo test`; frontend `npm run check` (0/0).
- Commit format: `type(scope): summary`.

---

### Task 1: Backend — ZIP extraction + open_url command

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Modify: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/lib.rs`

**Interfaces:**
- Produces: `extract_zip_xsds(zip_path: &Path, dest: &Path) -> (u32, Vec<String>)`; `copy_xsds` now also handles `.zip`; command `open_url(url: String) -> Result<(), String>`.

- [ ] **Step 1: Add the `zip` dependency (pure-Rust)**

In `app/src-tauri/Cargo.toml`, under `[dependencies]`, after the `tauri-plugin-dialog = "2"` line, add:

```toml
zip = { version = "2", default-features = false, features = ["deflate"] }
```

- [ ] **Step 2: Add `is_zip` + `extract_zip_xsds` helpers**

In `app/src-tauri/src/commands.rs`, directly after the existing `fn is_xsd(...) { ... }` helper, add:

```rust
fn is_zip(p: &Path) -> bool {
    p.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}

/// Extract every `.xsd` entry of a zip into `dest`, flattened to its basename
/// (so archive subfolders and any `../` are neutralized). Returns
/// (imported_count, skipped). On open/read failure the zip path is added to skipped.
fn extract_zip_xsds(zip_path: &Path, dest: &Path) -> (u32, Vec<String>) {
    let mut imported = 0u32;
    let mut skipped: Vec<String> = Vec::new();
    let file = match std::fs::File::open(zip_path) {
        Ok(f) => f,
        Err(_) => {
            skipped.push(zip_path.display().to_string());
            return (imported, skipped);
        }
    };
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => {
            skipped.push(zip_path.display().to_string());
            return (imported, skipped);
        }
    };
    for i in 0..archive.len() {
        let mut entry = match archive.by_index(i) {
            Ok(e) => e,
            Err(_) => continue,
        };
        if entry.is_dir() {
            continue;
        }
        let name = entry.name().to_string();
        let base = name
            .rsplit(|c| c == '/' || c == '\\')
            .next()
            .unwrap_or("")
            .to_string();
        if base.is_empty() || !base.to_lowercase().ends_with(".xsd") {
            continue;
        }
        match std::fs::File::create(dest.join(&base)) {
            Ok(mut out) if std::io::copy(&mut entry, &mut out).is_ok() => imported += 1,
            _ => skipped.push(format!("{}!{}", zip_path.display(), base)),
        }
    }
    (imported, skipped)
}
```

- [ ] **Step 3: Handle `.zip` in `copy_xsds`**

In `copy_xsds`, replace this block:

```rust
        } else if path.is_file() && is_xsd(path) {
            copy_one(path, dest, &mut imported, &mut skipped);
        } else {
            skipped.push(p.clone());
        }
```

with:

```rust
        } else if path.is_file() && is_zip(path) {
            let (imp, mut skp) = extract_zip_xsds(path, dest);
            imported += imp;
            skipped.append(&mut skp);
        } else if path.is_file() && is_xsd(path) {
            copy_one(path, dest, &mut imported, &mut skipped);
        } else {
            skipped.push(p.clone());
        }
```

- [ ] **Step 4: Add the `open_url` command**

In `app/src-tauri/src/commands.rs`, directly after the existing `open_schema_dir` command, add:

```rust
/// Open a URL in the default browser (Windows).
#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    std::process::Command::new("explorer")
        .arg(&url)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
```

- [ ] **Step 5: Register `open_url`**

In `app/src-tauri/src/lib.rs`, change the handler list tail:

```rust
            commands::schema_status,
            commands::import_schemas,
            commands::open_schema_dir
        ])
```

to (add the comma after `open_schema_dir`):

```rust
            commands::schema_status,
            commands::import_schemas,
            commands::open_schema_dir,
            commands::open_url
        ])
```

- [ ] **Step 6: Add a ZIP-extraction test**

In `app/src-tauri/src/commands.rs`, inside the existing `#[cfg(test)] mod tests { ... }`, add a test after `copies_all_xsd_from_directory_case_insensitive`:

```rust
    #[test]
    fn extracts_xsd_from_zip_and_ignores_non_xsd() {
        use std::io::Write as _;
        let dest = fresh_dir("sepa_imp_destzip");
        let zip_path = std::env::temp_dir().join("sepa_imp_test.zip");
        let _ = std::fs::remove_file(&zip_path);
        {
            let f = std::fs::File::create(&zip_path).unwrap();
            let mut zw = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            // A nested .xsd (must be flattened to its basename) and a non-.xsd.
            zw.start_file("schemas/pain.001.001.03.xsd", opts).unwrap();
            zw.write_all(b"<xsd/>").unwrap();
            zw.start_file("readme.txt", opts).unwrap();
            zw.write_all(b"hi").unwrap();
            zw.finish().unwrap();
        }
        let r = copy_xsds(&[zip_path.display().to_string()], &dest);
        assert_eq!(r.imported, 1);
        assert!(dest.join("pain.001.001.03.xsd").exists());
        assert!(!dest.join("readme.txt").exists());
    }
```

- [ ] **Step 7: Build + test**

Run: `cd app/src-tauri && cargo test`
Expected: compiles (zip pulled as a pure-Rust dep), all tests pass including the new zip test, no warnings.

- [ ] **Step 8: Commit**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs
git commit -m "feat(app): ZIP schema import + open-url command"
```

---

### Task 2: Frontend — download button + ZIP picker

**Files:**
- Modify: `app/src/lib/api.ts`
- Modify: `app/src/lib/SchemaDialog.svelte`

**Interfaces:**
- Consumes: the `open_url` command (Task 1); the existing `import_schemas` (now ZIP-capable).
- Produces: nothing for later tasks (final task).

- [ ] **Step 1: Add the `openUrl` api wrapper**

In `app/src/lib/api.ts`, append at the end of the file:

```ts
export function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}
```

- [ ] **Step 2: Import `openUrl` and add a download handler in SchemaDialog**

In `app/src/lib/SchemaDialog.svelte`, change the api import:

```ts
  import { schemaStatus, importSchemas, openSchemaDir } from "./api";
```

to:

```ts
  import { schemaStatus, importSchemas, openSchemaDir, openUrl } from "./api";
```

Then directly after the existing `openFolder` function, add:

```ts
  const DOWNLOAD_URL = "https://www.ebics.de/de/datenformate";
  async function download() {
    try {
      await openUrl(DOWNLOAD_URL);
    } catch {
      note = "Download-Seite konnte nicht geöffnet werden.";
    }
  }
```

- [ ] **Step 3: Accept ZIP in the file picker + update the note wording**

In `importFiles`, replace:

```ts
    const sel = await openDialog({ multiple: true, filters: [{ name: "XSD", extensions: ["xsd"] }] });
```

with:

```ts
    const sel = await openDialog({ multiple: true, filters: [{ name: "XSD/ZIP", extensions: ["xsd", "zip"] }] });
```

In `runImport`, replace:

```ts
      note = `${r.imported} XSD-Datei(en) importiert${r.skipped.length ? `, ${r.skipped.length} übersprungen` : ""}.`;
```

with:

```ts
      note = `${r.imported} Schema-Datei(en) importiert${r.skipped.length ? `, ${r.skipped.length} übersprungen` : ""}.`;
```

- [ ] **Step 4: Update the hint and add the buttons**

In `app/src/lib/SchemaDialog.svelte`, replace the hint paragraph:

```svelte
    <p class="hint">Die XSDs werden nicht mitgeliefert. Importiere die ISO-20022/GBIC-Schemas, um zu validieren.</p>
```

with:

```svelte
    <p class="hint">Die XSDs werden nicht mitgeliefert. Lade sie von der offiziellen Quelle (ebics.de für DK/GBIC, iso20022.org für die ISO-Schemas) und importiere sie hier als ZIP oder XSD.</p>
```

And replace the footer:

```svelte
    <footer>
      <button on:click={importFiles} disabled={busy}>XSD-Dateien…</button>
      <button on:click={importFolder} disabled={busy}>Ordner…</button>
      <button on:click={openFolder}>Ordner öffnen</button>
      <button class="close" on:click={close}>Schließen</button>
    </footer>
```

with:

```svelte
    <footer>
      <button on:click={download}>Herunterladen…</button>
      <button on:click={importFiles} disabled={busy}>XSD/ZIP-Dateien…</button>
      <button on:click={importFolder} disabled={busy}>Ordner…</button>
      <button on:click={openFolder}>Ordner öffnen</button>
      <button class="close" on:click={close}>Schließen</button>
    </footer>
```

- [ ] **Step 5: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add app/src/lib/api.ts app/src/lib/SchemaDialog.svelte
git commit -m "feat(app): Schemas dialog download button + ZIP import picker"
```

---

## Self-Review

**Spec coverage:**
- ZIP import (selected `.zip` → contained `.xsd`, flattened basename) → Task 1 Steps 2-3 + test Step 6.
- `open_url` command (Windows browser) + registration → Task 1 Steps 4-5.
- Download button opens ebics.de → Task 2 Steps 2, 4.
- File picker accepts xsd+zip → Task 2 Step 3.
- Hint mentions both sources → Task 2 Step 4.
- Pure-Rust zip (no new C deps) → Task 1 Step 1 (`default-features = false, features = ["deflate"]`).
- zip-slip neutralized via basename flattening → Task 1 Step 2.
- YAGNI exclusions (auto-download, recursive zip-in-folder, content validation, multi-source UI) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every step shows exact code. ✓

**Type consistency:** `open_url` command name matches `lib.rs` registration and `api.ts` `invoke("open_url", { url })`. `extract_zip_xsds` returns `(u32, Vec<String>)`, aggregated into `imported`/`skipped` in `copy_xsds`. `openUrl(url)` signature matches the call `openUrl(DOWNLOAD_URL)`. The zip writer test uses `SimpleFileOptions`/`CompressionMethod::Stored` (the `deflate` feature still covers reading real deflated archives). ✓
