# SEPA Validator — Tauri/Rust Rewrite Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild the Windows SEPA XML validator as a modern Tauri desktop app with a live, clickable, filterable validation log.

**Architecture:** Tauri v2 app in `app/`. Rust backend validates XML against embedded ISO 20022 XSD schemas using libxml2 (via the `libxml` crate); namespace detection uses `quick-xml`. Validation runs on a worker thread and streams `ValidationEvent`s to the frontend over a `tauri::ipc::Channel` for a live log. Svelte + TypeScript frontend renders the file list, a CodeMirror 6 XML viewer with jump-to-line, and a filterable/searchable log.

**Tech Stack:** Rust, Tauri v2, `libxml` (libxml2 bindings, vcpkg on Windows), `quick-xml`, `serde`, Svelte 5 + TypeScript + Vite, CodeMirror 6, Tauri plugins `dialog` + `fs`.

**Reference spec:** `docs/superpowers/specs/2026-06-18-sepa-tauri-rewrite-design.md`

---

## Conventions for this plan

- All Rust paths are relative to repo root unless noted. The Tauri Rust crate lives in `app/src-tauri/`.
- Commit format: `type(scope): summary` (per CLAUDE.md). Work happens on branch `feature/tauri-rust-rewrite`.
- **Privacy:** files under `to_check/` contain real company names and IBANs. NEVER copy them into git-tracked test fixtures or commit them. Tests that need real SEPA files read them by path from the local (gitignored) `to_check/` tree and **skip gracefully if absent**. Synthetic fixtures (unknown-namespace XML, malformed XML) are written to temp dirs at test time.
- **Schemas:** `xml_schema/*.xsd` are gitignored and not redistributable. They are embedded into the binary at build time. The build fails loudly if a mapped schema file is missing.

---

## File Structure

```
app/
├─ package.json                 # frontend deps + scripts
├─ vite.config.ts
├─ tsconfig.json
├─ index.html
├─ src/                         # Svelte frontend
│  ├─ main.ts                   # mounts App
│  ├─ App.svelte                # layout: toolbar / filelist / viewer+log / summary
│  ├─ lib/
│  │  ├─ api.ts                 # typed wrappers over Tauri invoke + Channel
│  │  ├─ types.ts               # ValidationResult / Message / Status mirrors of Rust
│  │  ├─ stores.ts              # results, selection, filter, theme stores
│  │  ├─ Toolbar.svelte
│  │  ├─ FileList.svelte
│  │  ├─ CodeViewer.svelte      # CodeMirror 6 XML view + jump-to-line + markers
│  │  ├─ LogPanel.svelte        # messages of selected file + filter + search
│  │  └─ SummaryBar.svelte
│  └─ styles.css
└─ src-tauri/
   ├─ Cargo.toml
   ├─ build.rs                  # embeds XSDs from ../../xml_schema, generates schema_data.rs
   ├─ tauri.conf.json
   └─ src/
      ├─ main.rs                # Tauri builder, command registration
      ├─ commands.rs           # start_validation (Channel), read_file
      ├─ model.rs              # ValidationResult, Status, Severity, Message
      ├─ schema.rs             # SCHEMA_MAP, embedded bytes, lookup
      ├─ validator.rs          # Validator (schema cache), validate_file, detect_namespace
      └─ scanner.rs            # expand_paths
```

---

## Phase 0 — Spike (de-risk libxml2 + schemas)

### Task 1: Scaffold the Tauri + Svelte/TS project

**Files:**
- Create: everything under `app/` (generated)

- [ ] **Step 1: Generate the project**

Run from repo root:
```bash
npm create tauri-app@latest app -- --template svelte-ts --manager npm --yes
```
This creates `app/` with `src/` (Svelte-TS) and `src-tauri/` (Rust + Tauri v2).

- [ ] **Step 2: Verify it builds and runs (dev)**

Run:
```bash
cd app && npm install && npm run tauri dev
```
Expected: a window titled with the default template opens. Close it.

- [ ] **Step 3: Pin app metadata**

Edit `app/src-tauri/tauri.conf.json`: set `productName` to `SEPA Validator`, `identifier` to `dev.sepa.validator`, `app.windows[0].title` to `SEPA XML Validator`, `app.windows[0].width` to `1100`, `height` to `720`, `minWidth` to `820`, `minHeight` to `560`.

- [ ] **Step 4: Commit**

```bash
git add app .gitignore
git commit -m "feat(app): scaffold Tauri + Svelte-TS project"
```

---

### Task 2: libxml2 spike — prove schema compilation + validation on Windows

This is the highest-risk step. Its job: confirm libxml2 links via vcpkg, the SEPA XSDs compile, and we get error messages with line numbers. We also pin the exact `libxml` API (`StructuredError` fields).

**Files:**
- Modify: `app/src-tauri/Cargo.toml`
- Create: `app/src-tauri/tests/spike.rs`

- [ ] **Step 1: Install libxml2 via vcpkg (one-time, Windows)**

Run (in a shell with git + cmake available):
```bash
git clone https://github.com/microsoft/vcpkg "$HOME/vcpkg" || true
"$HOME/vcpkg/bootstrap-vcpkg.sh" 2>/dev/null || powershell -File "$HOME/vcpkg/bootstrap-vcpkg.bat"
"$HOME/vcpkg/vcpkg" install libxml2:x64-windows-static-md
```
Set the env var so the `libxml` crate's build finds it (PowerShell, current session):
```powershell
$env:VCPKG_ROOT = "$HOME\vcpkg"
$env:LIBXML2 = "$HOME\vcpkg\installed\x64-windows-static-md\lib\libxml2.lib"
```
Note: `libxml` builds against the system/vcpkg libxml2. If `LIBXML2` pointing at the `.lib` does not work, fall back to `x64-windows` (DLL) triplet and ensure the DLL is on PATH.

- [ ] **Step 2: Add dependencies**

Edit `app/src-tauri/Cargo.toml`, add under `[dependencies]`:
```toml
libxml = "0.3"
quick-xml = "0.36"
```

- [ ] **Step 3: Write the spike test**

Create `app/src-tauri/tests/spike.rs`:
```rust
//! Spike: prove libxml2 compiles a SEPA schema and validates documents.
//! Confirms the `libxml` API and that the XSDs are self-contained.
use std::path::PathBuf;

use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};

fn repo_root() -> PathBuf {
    // app/src-tauri -> repo root
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
}

#[test]
fn schema_compiles_and_validates() {
    let xsd = repo_root().join("xml_schema/pain.008.001.02.xsd");
    let valid = repo_root()
        .join("to_check/valid/20250410_ENRW_ENERGIEVERSORGUNG_ROTTWEIL_GMBH_CO_KG_PAIN00800102.xml");

    if !xsd.exists() || !valid.exists() {
        eprintln!("SKIP: local xml_schema/ or to_check/ fixtures not present");
        return;
    }

    // Compile schema from file.
    let mut sp = SchemaParserContext::from_file(xsd.to_str().unwrap());
    let mut validator = SchemaValidationContext::from_parser(&mut sp)
        .expect("schema must compile (self-contained, no external imports)");

    // Parse + validate a known-good document.
    let doc = Parser::default()
        .parse_file(valid.to_str().unwrap())
        .expect("valid.xml must parse");

    match validator.validate_document(&doc) {
        Ok(()) => { /* expected */ }
        Err(errors) => {
            for e in &errors {
                // CONFIRM these field accesses against `cargo doc -p libxml` and adjust
                // model extraction in Task 6 to match the real StructuredError shape.
                eprintln!("err: msg={:?} line={:?} level={:?}", e.message, e.line, e.level);
            }
            panic!("known-good file reported {} schema errors", errors.len());
        }
    }
}
```

- [ ] **Step 4: Run the spike**

Run:
```bash
cd app/src-tauri && cargo test --test spike -- --nocapture
```
Expected: PASS. If the local fixtures are missing it prints `SKIP` and passes — in that case place at least one XSD + matching valid XML locally and re-run before continuing.

- [ ] **Step 5: Reconcile schema filenames + pin the StructuredError API**

While the spike output is fresh, record two facts in a scratch note (used by Tasks 4 and 6):
1. The exact field/method names on `StructuredError` (e.g. `message`, `line`, `level`, and whether a column is exposed via `int2`).
2. Which physical files in `xml_schema/` back each mapped namespace. Known gap: the map references `pain.007.001.09.xsd` but only `pain.007.001.09_GBIC_5.xsd` is present — Task 4's map must use the present filename.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/Cargo.toml app/src-tauri/tests/spike.rs
git commit -m "feat(app): libxml2 schema-validation spike"
```

---

## Phase 1 — Rust backend (TDD)

### Task 3: Data model

**Files:**
- Create: `app/src-tauri/src/model.rs`
- Modify: `app/src-tauri/src/main.rs` (add `mod model;`)
- Test: inline `#[cfg(test)]` in `model.rs`

- [ ] **Step 1: Write the failing test**

Create `app/src-tauri/src/model.rs`:
```rust
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub severity: Severity,
    pub text: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Invalid,
    Warnings,
    NoSchema,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationResult {
    pub file: String,
    pub path: String,
    pub namespace: String,
    pub schema: String,
    pub status: Status,
    pub errors: u32,
    pub warnings: u32,
    pub messages: Vec<Message>,
}

impl ValidationResult {
    /// Build a result from collected messages, deriving status + counts.
    pub fn from_messages(
        file: String,
        path: String,
        namespace: String,
        schema: String,
        messages: Vec<Message>,
    ) -> Self {
        let errors = messages.iter().filter(|m| m.severity == Severity::Error).count() as u32;
        let warnings = messages.iter().filter(|m| m.severity == Severity::Warning).count() as u32;
        let status = if errors > 0 {
            Status::Invalid
        } else if warnings > 0 {
            Status::Warnings
        } else {
            Status::Ok
        };
        Self { file, path, namespace, schema, status, errors, warnings, messages }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_is_ok_with_no_messages() {
        let r = ValidationResult::from_messages(
            "f.xml".into(), "/f.xml".into(), "ns".into(), "s.xsd".into(), vec![],
        );
        assert_eq!(r.status, Status::Ok);
        assert_eq!(r.errors, 0);
        assert_eq!(r.warnings, 0);
    }

    #[test]
    fn status_is_invalid_with_an_error() {
        let msgs = vec![Message { severity: Severity::Error, text: "boom".into(), line: Some(4), column: None }];
        let r = ValidationResult::from_messages(
            "f.xml".into(), "/f.xml".into(), "ns".into(), "s.xsd".into(), msgs,
        );
        assert_eq!(r.status, Status::Invalid);
        assert_eq!(r.errors, 1);
    }

    #[test]
    fn status_is_warnings_when_only_warnings() {
        let msgs = vec![Message { severity: Severity::Warning, text: "hmm".into(), line: None, column: None }];
        let r = ValidationResult::from_messages(
            "f.xml".into(), "/f.xml".into(), "ns".into(), "s.xsd".into(), msgs,
        );
        assert_eq!(r.status, Status::Warnings);
        assert_eq!(r.warnings, 1);
    }

    #[test]
    fn serializes_status_as_snake_case_string() {
        let r = ValidationResult::from_messages(
            "f.xml".into(), "/f.xml".into(), "ns".into(), "s.xsd".into(), vec![],
        );
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"status\":\"ok\""));
        assert!(json.contains("\"namespace\":\"ns\""));
    }
}
```
Add to `app/src-tauri/src/main.rs` near the top (after the existing `#![cfg_attr(...)]` line): `mod model;`. Add `serde_json` to `[dev-dependencies]` in `Cargo.toml` (`serde_json = "1"`); `serde` with `derive` is already a Tauri dependency but add `serde = { version = "1", features = ["derive"] }` explicitly under `[dependencies]` if not present.

- [ ] **Step 2: Run tests to verify they pass**

Run:
```bash
cd app/src-tauri && cargo test model:: -- --nocapture
```
Expected: 4 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/model.rs app/src-tauri/src/main.rs app/src-tauri/Cargo.toml
git commit -m "feat(app): validation data model"
```

---

### Task 4: Schema map + embedding

**Files:**
- Create: `app/src-tauri/build.rs`
- Create: `app/src-tauri/src/schema.rs`
- Modify: `app/src-tauri/src/main.rs` (add `mod schema;`)
- Modify: `app/src-tauri/Cargo.toml` (`build = "build.rs"`)

- [ ] **Step 1: Write build.rs to embed XSDs**

Create `app/src-tauri/build.rs`:
```rust
use std::path::Path;

// Namespace -> physical XSD filename in <repo>/xml_schema.
// Mirror of the PowerShell $SchemaMap, reconciled to the files actually present
// (pain.007 uses the _GBIC_5 variant — confirmed in the Task 2 spike).
const SCHEMA_FILES: &[&str] = &[
    "pain.001.001.03.xsd",
    "pain.001.001.09.xsd",
    "pain.002.001.10.xsd",
    "pain.007.001.09_GBIC_5.xsd",
    "pain.008.001.02.xsd",
    "pain.008.001.08.xsd",
    "camt.054.001.08.xsd",
    "container.nnn.001.GBIC4.xsd",
];

fn main() {
    let schema_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("xml_schema");
    println!("cargo:rerun-if-changed={}", schema_dir.display());
    for f in SCHEMA_FILES {
        let p = schema_dir.join(f);
        if !p.exists() {
            panic!(
                "Required schema file missing: {}\nPlace the ISO 20022 / GBIC XSDs in xml_schema/.",
                p.display()
            );
        }
        println!("cargo:rerun-if-changed={}", p.display());
    }
    // build.rs only validates presence; schema.rs uses include_bytes! with literal paths.
    tauri_build::build();
}
```

- [ ] **Step 2: Write the failing test + schema.rs**

Create `app/src-tauri/src/schema.rs`:
```rust
//! Embedded ISO 20022 / GBIC XSD schemas and namespace lookup.

macro_rules! xsd {
    ($f:literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../xml_schema/", $f))
    };
}

/// Ordered namespace -> (display filename, embedded bytes).
pub const SCHEMAS: &[(&str, &str, &[u8])] = &[
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.03", "pain.001.001.03.xsd", xsd!("pain.001.001.03.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:pain.001.001.09", "pain.001.001.09.xsd", xsd!("pain.001.001.09.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:pain.002.001.10", "pain.002.001.10.xsd", xsd!("pain.002.001.10.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:pain.007.001.09", "pain.007.001.09.xsd", xsd!("pain.007.001.09_GBIC_5.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02", "pain.008.001.02.xsd", xsd!("pain.008.001.02.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:pain.008.001.08", "pain.008.001.08.xsd", xsd!("pain.008.001.08.xsd")),
    ("urn:iso:std:iso:20022:tech:xsd:camt.054.001.08", "camt.054.001.08.xsd", xsd!("camt.054.001.08.xsd")),
    ("urn:conxml:xsd:container.nnn.001.GBIC4", "container.nnn.001.GBIC4.xsd", xsd!("container.nnn.001.GBIC4.xsd")),
];

/// Returns (display_filename, xsd_bytes) for a namespace, if known.
pub fn lookup(namespace: &str) -> Option<(&'static str, &'static [u8])> {
    SCHEMAS.iter().find(|(ns, _, _)| *ns == namespace).map(|(_, name, bytes)| (*name, *bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_has_non_empty_bytes() {
        for (ns, name, bytes) in SCHEMAS {
            assert!(!bytes.is_empty(), "{ns} -> {name} embedded empty");
            assert!(name.ends_with(".xsd"));
        }
    }

    #[test]
    fn lookup_known_namespace() {
        let (name, bytes) = lookup("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02").unwrap();
        assert_eq!(name, "pain.008.001.02.xsd");
        assert!(bytes.len() > 100);
    }

    #[test]
    fn lookup_unknown_namespace_is_none() {
        assert!(lookup("urn:made:up").is_none());
    }
}
```
Add `build = "build.rs"` under `[package]` in `Cargo.toml` (the scaffold already sets this — verify). Add `mod schema;` to `main.rs`.

- [ ] **Step 3: Run tests**

Run:
```bash
cd app/src-tauri && cargo test schema:: -- --nocapture
```
Expected: 3 tests PASS. If `build.rs` panics about a missing file, reconcile the filename in `xsd!(...)` against `xml_schema/`.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/build.rs app/src-tauri/src/schema.rs app/src-tauri/src/main.rs app/src-tauri/Cargo.toml
git commit -m "feat(app): embed XSD schemas with namespace lookup"
```

---

### Task 5: Namespace detection

**Files:**
- Create: `app/src-tauri/src/validator.rs` (partial — `detect_namespace` only)
- Modify: `app/src-tauri/src/main.rs` (add `mod validator;`)

- [ ] **Step 1: Write the failing test + detect_namespace**

Create `app/src-tauri/src/validator.rs`:
```rust
use std::path::Path;

use quick_xml::events::Event;
use quick_xml::name::ResolveResult;
use quick_xml::reader::NsReader;

/// Returns the namespace URI bound to the first (root) element, or None.
pub fn detect_namespace(path: &Path) -> Option<String> {
    let mut reader = NsReader::from_file(path).ok()?;
    let mut buf = Vec::new();
    loop {
        match reader.read_resolved_event_into(&mut buf) {
            Ok((ResolveResult::Bound(ns), Event::Start(_) | Event::Empty(_))) => {
                return Some(String::from_utf8_lossy(ns.as_ref()).into_owned());
            }
            Ok((_, Event::Start(_) | Event::Empty(_))) => return None, // element but no namespace
            Ok((_, Event::Eof)) => return None,
            Ok(_) => buf.clear(),
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_xml(contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        // unique-ish per contents length + a fixed prefix; tests run single-threaded enough
        p.push(format!("sepa_ns_test_{}.xml", contents.len()));
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn detects_default_namespace_on_root() {
        let p = temp_xml(r#"<?xml version="1.0"?><Document xmlns="urn:iso:std:iso:20022:tech:xsd:pain.008.001.02"><X/></Document>"#);
        assert_eq!(
            detect_namespace(&p).as_deref(),
            Some("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02")
        );
    }

    #[test]
    fn returns_none_for_no_namespace() {
        let p = temp_xml(r#"<?xml version="1.0"?><root><child/></root>"#);
        assert_eq!(detect_namespace(&p), None);
    }

    #[test]
    fn returns_none_for_garbage() {
        let p = temp_xml("not xml at all <<<");
        assert_eq!(detect_namespace(&p), None);
    }
}
```
Add `mod validator;` to `main.rs`.

- [ ] **Step 2: Run tests**

Run:
```bash
cd app/src-tauri && cargo test validator::tests::detects -- --nocapture && cargo test validator::tests::returns -- --nocapture
```
Expected: 3 tests PASS. (If `read_resolved_event_into` signature differs in the installed quick-xml, adjust per `cargo doc -p quick-xml`; the namespace-resolving reader API is `NsReader`.)

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/validator.rs app/src-tauri/src/main.rs
git commit -m "feat(app): root-element namespace detection"
```

---

### Task 6: Validator with schema cache

**Files:**
- Modify: `app/src-tauri/src/validator.rs` (add `Validator`, `validate_file`)
- Test: inline + a guarded integration-style test using local fixtures

- [ ] **Step 1: Add Validator + validate_file**

Append to `app/src-tauri/src/validator.rs` (above the `#[cfg(test)]` module):
```rust
use std::collections::HashMap;

use libxml::parser::Parser;
use libxml::schemas::{SchemaParserContext, SchemaValidationContext};

use crate::model::{Message, Severity, Status, ValidationResult};
use crate::schema;

/// Holds a per-run cache of compiled schemas. Not Send (wraps libxml2 pointers):
/// construct and use it on a single worker thread.
pub struct Validator {
    cache: HashMap<&'static str, SchemaValidationContext>,
}

impl Validator {
    pub fn new() -> Self {
        Self { cache: HashMap::new() }
    }

    pub fn validate_file(&mut self, path: &Path) -> ValidationResult {
        let file = path.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let path_str = path.display().to_string();

        let mk = |ns: String, schema_name: String, msgs: Vec<Message>, status: Status, e: u32, w: u32| {
            ValidationResult { file: file.clone(), path: path_str.clone(), namespace: ns, schema: schema_name, status, errors: e, warnings: w, messages: msgs }
        };

        if !path.exists() {
            return mk(String::new(), String::new(),
                vec![Message { severity: Severity::Error, text: "File not found.".into(), line: None, column: None }],
                Status::Error, 1, 0);
        }

        let ns = match detect_namespace(path) {
            Some(ns) => ns,
            None => return mk(String::new(), String::new(),
                vec![Message { severity: Severity::Error, text: "No XML namespace detected. File may not be valid XML.".into(), line: None, column: None }],
                Status::Error, 1, 0),
        };

        let (schema_name, _bytes) = match schema::lookup(&ns) {
            Some(v) => v,
            None => return mk(ns.clone(), String::new(),
                vec![Message { severity: Severity::Warning, text: format!("No matching schema for namespace: {ns}"), line: None, column: None }],
                Status::NoSchema, 0, 1),
        };

        // Ensure compiled schema is cached.
        if !self.cache.contains_key(schema_name) {
            match self.compile(&ns) {
                Ok(ctx) => { self.cache.insert(schema_name, ctx); }
                Err(text) => return mk(ns.clone(), schema_name.to_string(),
                    vec![Message { severity: Severity::Error, text, line: None, column: None }],
                    Status::Error, 1, 0),
            }
        }
        let validator = self.cache.get_mut(schema_name).unwrap();

        // Parse the document.
        let doc = match Parser::default().parse_file(&path_str) {
            Ok(d) => d,
            Err(e) => return mk(ns.clone(), schema_name.to_string(),
                vec![Message { severity: Severity::Error, text: format!("XML parse error: {e}"), line: None, column: None }],
                Status::Error, 1, 0),
        };

        let messages = match validator.validate_document(&doc) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(to_message).collect(),
        };

        ValidationResult::from_messages(file, path_str, ns, schema_name.to_string(), messages)
    }

    fn compile(&self, namespace: &str) -> Result<SchemaValidationContext, String> {
        let (_name, bytes) = schema::lookup(namespace).ok_or("schema not found")?;
        // from_buffer keeps schemas embedded (no temp files).
        let mut parser = SchemaParserContext::from_buffer(bytes);
        SchemaValidationContext::from_parser(&mut parser)
            .map_err(|errs| format!("Failed to load schema: {} error(s)", errs.len()))
    }
}

/// Map a libxml StructuredError to our Message.
/// NOTE: field access (`message`, `line`, `level`) confirmed by the Task 2 spike;
/// adjust here if the spike found different names. Column = int2 when > 0.
fn to_message(e: &libxml::error::StructuredError) -> Message {
    use libxml::error::XmlErrorLevel;
    let severity = match e.level {
        XmlErrorLevel::Warning => Severity::Warning,
        _ => Severity::Error,
    };
    let text = e.message.clone().unwrap_or_else(|| "validation error".into()).trim().to_string();
    let line = e.line.filter(|l| *l > 0).map(|l| l as u32);
    Message { severity, text, line, column: None }
}
```
If the spike found a column field, set `column` from it; otherwise leave `None` (the model already treats it as optional).

- [ ] **Step 2: Add guarded fixture tests**

Append inside the existing `#[cfg(test)] mod tests` block in `validator.rs`:
```rust
    use crate::model::Status;

    fn repo_root() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..")
    }

    #[test]
    fn unknown_namespace_yields_no_schema() {
        let p = temp_xml(r#"<?xml version="1.0"?><Doc xmlns="urn:made:up"><X/></Doc>"#);
        let mut v = super::Validator::new();
        let r = v.validate_file(&p);
        assert_eq!(r.status, Status::NoSchema);
        assert_eq!(r.namespace, "urn:made:up");
    }

    #[test]
    fn valid_fixture_is_ok() {
        let f = repo_root().join("to_check/valid/20250410_ENRW_ENERGIEVERSORGUNG_ROTTWEIL_GMBH_CO_KG_PAIN00800102.xml");
        if !f.exists() { eprintln!("SKIP: fixture absent"); return; }
        let mut v = super::Validator::new();
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Ok, "messages: {:?}", r.messages);
    }

    #[test]
    fn invalid_fixture_reports_errors() {
        let f = repo_root().join("to_check/invalid/20250121_NOFIRMA_PAIN00100109_1.xml");
        if !f.exists() { eprintln!("SKIP: fixture absent"); return; }
        let mut v = super::Validator::new();
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Invalid);
        assert!(r.errors >= 1);
        assert!(r.messages.iter().any(|m| m.line.is_some()), "expect at least one located error");
    }
```

- [ ] **Step 3: Run tests**

Run:
```bash
cd app/src-tauri && cargo test validator:: -- --nocapture
```
Expected: namespace tests PASS; `unknown_namespace_yields_no_schema` PASS; the two fixture tests PASS (or print SKIP if local fixtures absent). If a fixture test fails on status, inspect printed messages — a genuine schema/file mismatch must be reconciled, not asserted away.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/validator.rs
git commit -m "feat(app): XSD validator with per-run schema cache"
```

---

### Task 7: Path scanner

**Files:**
- Create: `app/src-tauri/src/scanner.rs`
- Modify: `app/src-tauri/src/main.rs` (add `mod scanner;`)
- Modify: `app/src-tauri/Cargo.toml` (add `walkdir = "2"`)

- [ ] **Step 1: Write the failing test + scanner**

Create `app/src-tauri/src/scanner.rs`:
```rust
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

/// Expand a set of input paths (files and/or directories) into a deduplicated,
/// sorted list of `.xml` files. Directories are walked recursively.
/// Skips NTFS `:Zone.Identifier` alternate-stream artifacts.
pub fn expand_paths<I, P>(inputs: I) -> Vec<PathBuf>
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    let mut out: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let input = input.as_ref();
        if input.is_dir() {
            for entry in WalkDir::new(input).into_iter().filter_map(Result::ok) {
                if entry.file_type().is_file() && is_xml(entry.path()) {
                    out.push(entry.into_path());
                }
            }
        } else if input.is_file() && is_xml(input) {
            out.push(input.to_path_buf());
        }
    }
    out.sort();
    out.dedup();
    out
}

fn is_xml(p: &Path) -> bool {
    let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
    name.to_ascii_lowercase().ends_with(".xml") && !name.contains(":Zone.Identifier")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn expands_dir_and_filters_non_xml() {
        let dir = std::env::temp_dir().join("sepa_scan_test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.xml"), "<a/>").unwrap();
        fs::write(dir.join("b.txt"), "nope").unwrap();
        fs::write(dir.join("sub/c.XML"), "<c/>").unwrap();

        let got = expand_paths([&dir]);
        let names: Vec<_> = got.iter().filter_map(|p| p.file_name()?.to_str()).map(|s| s.to_string()).collect();
        assert!(names.contains(&"a.xml".to_string()));
        assert!(names.contains(&"c.XML".to_string()));
        assert!(!names.iter().any(|n| n.ends_with(".txt")));
    }

    #[test]
    fn passes_through_single_file() {
        let f = std::env::temp_dir().join("sepa_scan_single.xml");
        fs::write(&f, "<a/>").unwrap();
        let got = expand_paths([&f]);
        assert_eq!(got.len(), 1);
    }
}
```
Add `mod scanner;` to `main.rs` and `walkdir = "2"` to `Cargo.toml`.

- [ ] **Step 2: Run tests**

Run:
```bash
cd app/src-tauri && cargo test scanner:: -- --nocapture
```
Expected: 2 tests PASS.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/scanner.rs app/src-tauri/src/main.rs app/src-tauri/Cargo.toml
git commit -m "feat(app): recursive XML path scanner"
```

---

## Phase 2 — Tauri commands + event streaming

### Task 8: start_validation (Channel) + read_file commands

**Files:**
- Create: `app/src-tauri/src/commands.rs`
- Modify: `app/src-tauri/src/main.rs` (register commands, add `mod commands;`)

- [ ] **Step 1: Write the commands**

Create `app/src-tauri/src/commands.rs`:
```rust
use std::path::PathBuf;

use serde::Serialize;
use tauri::ipc::Channel;

use crate::model::ValidationResult;
use crate::scanner;
use crate::validator::Validator;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "event", content = "data")]
pub enum ValidationEvent {
    Started { total: usize },
    Result { index: usize, result: ValidationResult },
    Finished { total: usize },
}

/// Expand inputs, then validate each file on a worker thread, streaming
/// results to the frontend in order via the channel.
#[tauri::command]
pub fn start_validation(paths: Vec<String>, on_event: Channel<ValidationEvent>) {
    let files: Vec<PathBuf> = scanner::expand_paths(paths.iter().map(PathBuf::from));
    let total = files.len();

    // libxml types are not Send: build the Validator inside the thread.
    std::thread::spawn(move || {
        let _ = on_event.send(ValidationEvent::Started { total });
        let mut validator = Validator::new();
        for (index, file) in files.iter().enumerate() {
            let result = validator.validate_file(file);
            let _ = on_event.send(ValidationEvent::Result { index, result });
        }
        let _ = on_event.send(ValidationEvent::Finished { total });
    });
}

/// Read a file's text for the code viewer (lossy UTF-8).
#[tauri::command]
pub fn read_file(path: String) -> Result<String, String> {
    std::fs::read(&path)
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register modules + commands in main.rs**

In `app/src-tauri/src/main.rs`, ensure these module decls exist near the top:
```rust
mod model;
mod schema;
mod validator;
mod scanner;
mod commands;
```
And register the handler in the builder (inside the generated `run`/`main`):
```rust
tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .plugin(tauri_plugin_fs::init())
    .invoke_handler(tauri::generate_handler![
        commands::start_validation,
        commands::read_file
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
```

- [ ] **Step 3: Add the dialog + fs plugins**

Run:
```bash
cd app && npm run tauri add dialog && npm run tauri add fs
```
This adds the Rust crates `tauri-plugin-dialog`/`tauri-plugin-fs`, the JS packages, and capability entries.

- [ ] **Step 4: Verify it compiles**

Run:
```bash
cd app/src-tauri && cargo build
```
Expected: builds with no errors.

- [ ] **Step 5: Commit**

```bash
git add app/src-tauri/src app/src-tauri/Cargo.toml app/src-tauri/capabilities app/package.json app/package-lock.json
git commit -m "feat(app): streaming validation + read_file commands"
```

---

## Phase 3 — Svelte frontend

### Task 9: Frontend types + API wrappers

**Files:**
- Create: `app/src/lib/types.ts`
- Create: `app/src/lib/api.ts`

- [ ] **Step 1: Types mirroring the Rust model**

Create `app/src/lib/types.ts`:
```ts
export type Severity = "error" | "warning";
export type StatusKind = "ok" | "invalid" | "warnings" | "no_schema" | "error";

export interface Message {
  severity: Severity;
  text: string;
  line: number | null;
  column: number | null;
}

export interface ValidationResult {
  file: string;
  path: string;
  namespace: string;
  schema: string;
  status: StatusKind;
  errors: number;
  warnings: number;
  messages: Message[];
}

export type ValidationEvent =
  | { event: "started"; data: { total: number } }
  | { event: "result"; data: { index: number; result: ValidationResult } }
  | { event: "finished"; data: { total: number } };

/** Human label like the old tool: "INVALID (2 errors, 1 warning)". */
export function statusLabel(r: ValidationResult): string {
  switch (r.status) {
    case "ok": return "OK";
    case "warnings": return `WARNINGS (${r.warnings})`;
    case "no_schema": return "NO SCHEMA";
    case "error": return "ERROR";
    case "invalid": return `INVALID (${r.errors} errors, ${r.warnings} warnings)`;
  }
}
```

- [ ] **Step 2: API wrapper**

Create `app/src/lib/api.ts`:
```ts
import { invoke, Channel } from "@tauri-apps/api/core";
import type { ValidationEvent, ValidationResult } from "./types";

/** Start validation; `onEvent` is called for each streamed event in order. */
export async function startValidation(
  paths: string[],
  onEvent: (ev: ValidationEvent) => void
): Promise<void> {
  const channel = new Channel<ValidationEvent>();
  channel.onmessage = onEvent;
  await invoke("start_validation", { paths, onEvent: channel });
}

export function readFile(path: string): Promise<string> {
  return invoke<string>("read_file", { path });
}

export type { ValidationResult };
```

- [ ] **Step 3: Verify type-check**

Run:
```bash
cd app && npx tsc --noEmit
```
Expected: no type errors. (Install `@tauri-apps/api` if the scaffold didn't: `npm i @tauri-apps/api`.)

- [ ] **Step 4: Commit**

```bash
git add app/src/lib/types.ts app/src/lib/api.ts
git commit -m "feat(app): frontend types and Tauri API wrappers"
```

---

### Task 10: Stores + App layout skeleton

**Files:**
- Create: `app/src/lib/stores.ts`
- Modify: `app/src/App.svelte`
- Modify: `app/src/styles.css`

- [ ] **Step 1: Stores**

Create `app/src/lib/stores.ts`:
```ts
import { writable, derived } from "svelte/store";
import type { ValidationResult } from "./types";

export const results = writable<ValidationResult[]>([]);
export const selectedIndex = writable<number>(-1);
export const progress = writable<{ done: number; total: number; running: boolean }>({
  done: 0, total: 0, running: false,
});
export type LogFilter = "all" | "errors" | "warnings";
export const logFilter = writable<LogFilter>("all");
export const search = writable<string>("");
export const theme = writable<"system" | "light" | "dark">("system");

export const selectedResult = derived(
  [results, selectedIndex],
  ([$results, $i]) => ($i >= 0 && $i < $results.length ? $results[$i] : null)
);

export const summary = derived(results, ($r) => ({
  total: $r.length,
  ok: $r.filter((x) => x.status === "ok").length,
  invalid: $r.filter((x) => x.status === "invalid" || x.status === "error").length,
  warnings: $r.filter((x) => x.status === "warnings").length,
  noSchema: $r.filter((x) => x.status === "no_schema").length,
}));
```

- [ ] **Step 2: App layout**

Replace `app/src/App.svelte`:
```svelte
<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";
</script>

<div class="app">
  <Toolbar />
  <main class="body">
    <aside class="files"><FileList /></aside>
    <section class="viewer"><CodeViewer /></section>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
</div>
```

- [ ] **Step 3: Base layout CSS**

Replace `app/src/styles.css` with a grid layout and CSS variables for theming:
```css
:root {
  --bg: #f4f5f7; --fg: #1d1f23; --panel: #ffffff; --border: #e1e3e8;
  --accent: #0a84ff; --ok: #157f3b; --err: #c4271c; --warn: #9d5d00;
  --code-bg: #1e1e1e; --code-fg: #dcdcdc;
}
:root[data-theme="dark"] {
  --bg: #1b1d22; --fg: #e6e7ea; --panel: #24262c; --border: #34373f;
  --accent: #0a84ff; --ok: #4ec9b0; --err: #f44747; --warn: #ffc832;
}
* { box-sizing: border-box; }
html, body, .app { height: 100%; margin: 0; }
body { font: 14px "Segoe UI", system-ui, sans-serif; background: var(--bg); color: var(--fg); }
.app { display: grid; grid-template-rows: auto 1fr auto; height: 100vh; }
.body { display: grid; grid-template-columns: 260px 1fr 360px; min-height: 0; }
.files, .viewer, .log { min-height: 0; overflow: hidden; border-right: 1px solid var(--border); background: var(--panel); }
.log { border-right: none; }
```

- [ ] **Step 4: Create placeholder components so it compiles**

Create five files, each a minimal placeholder to be filled in later tasks:
`app/src/lib/Toolbar.svelte`, `FileList.svelte`, `CodeViewer.svelte`, `LogPanel.svelte`, `SummaryBar.svelte`, each containing:
```svelte
<div class="placeholder">component</div>
```

- [ ] **Step 5: Verify the app renders the shell**

Run:
```bash
cd app && npm run tauri dev
```
Expected: a three-column shell renders. Close it.

- [ ] **Step 6: Commit**

```bash
git add app/src
git commit -m "feat(app): app shell layout and stores"
```

---

### Task 11: FileList component

**Files:**
- Modify: `app/src/lib/FileList.svelte`

- [ ] **Step 1: Implement FileList**

Replace `app/src/lib/FileList.svelte`:
```svelte
<script lang="ts">
  import { results, selectedIndex } from "./stores";
  import { statusLabel } from "./types";
  import type { ValidationResult } from "./types";

  function icon(r: ValidationResult): string {
    switch (r.status) {
      case "ok": return "✓";
      case "invalid": case "error": return "✗";
      default: return "⚠";
    }
  }
  function cls(r: ValidationResult): string {
    if (r.status === "ok") return "ok";
    if (r.status === "invalid" || r.status === "error") return "err";
    return "warn";
  }
</script>

<ul class="filelist">
  {#each $results as r, i}
    <li class:selected={i === $selectedIndex} on:click={() => selectedIndex.set(i)}
        title={statusLabel(r)}>
      <span class="icon {cls(r)}">{icon(r)}</span>
      <span class="name">{r.file}</span>
    </li>
  {/each}
  {#if $results.length === 0}
    <li class="empty">No files yet — add files or drag &amp; drop.</li>
  {/if}
</ul>

<style>
  .filelist { list-style: none; margin: 0; padding: 0; overflow-y: auto; height: 100%; }
  li { display: flex; gap: 8px; align-items: center; padding: 7px 10px; cursor: pointer; border-bottom: 1px solid var(--border); }
  li.selected { background: color-mix(in srgb, var(--accent) 15%, transparent); }
  .icon { width: 1em; font-weight: 700; }
  .icon.ok { color: var(--ok); } .icon.err { color: var(--err); } .icon.warn { color: var(--warn); }
  .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .empty { color: #888; cursor: default; }
</style>
```

- [ ] **Step 2: Verify (after Task 15 wires data; for now type-check)**

Run:
```bash
cd app && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/lib/FileList.svelte
git commit -m "feat(app): file list with status icons"
```

---

### Task 12: CodeViewer with CodeMirror 6

**Files:**
- Modify: `app/src/lib/CodeViewer.svelte`
- Modify: `app/package.json` (CodeMirror deps)

- [ ] **Step 1: Install CodeMirror**

Run:
```bash
cd app && npm i @codemirror/state @codemirror/view @codemirror/language @codemirror/lang-xml @codemirror/theme-one-dark
```

- [ ] **Step 2: Implement CodeViewer**

Replace `app/src/lib/CodeViewer.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState, StateEffect, StateField, RangeSetBuilder } from "@codemirror/state";
  import { EditorView, lineNumbers, Decoration, type DecorationSet, gutter, GutterMarker } from "@codemirror/view";
  import { xml } from "@codemirror/lang-xml";
  import { oneDark } from "@codemirror/theme-one-dark";
  import { selectedResult } from "./stores";
  import { readFile } from "./api";

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  let currentPath = "";

  // Decoration to highlight error lines.
  const setErrorLines = StateEffect.define<number[]>();
  const errorField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setErrorLines)) {
          const b = new RangeSetBuilder<Decoration>();
          const doc = tr.state.doc;
          for (const ln of e.value) {
            if (ln >= 1 && ln <= doc.lines) {
              const line = doc.line(ln);
              b.add(line.from, line.from, Decoration.line({ class: "cm-error-line" }));
            }
          }
          deco = b.finish();
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: "",
        extensions: [lineNumbers(), xml(), oneDark, errorField, EditorView.editable.of(false),
          EditorView.theme({ ".cm-error-line": { backgroundColor: "rgba(244,71,71,0.18)" } })],
      }),
    });
    return () => view?.destroy();
  });

  // Load file content when selection changes.
  $: void loadFor($selectedResult?.path, $selectedResult?.messages.map((m) => m.line ?? 0).filter((l) => l > 0));

  async function loadFor(path: string | undefined, errorLines: number[] | undefined) {
    if (!view || !path) return;
    if (path !== currentPath) {
      currentPath = path;
      let text = "";
      try { text = await readFile(path); } catch { text = "(could not read file)"; }
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
    view.dispatch({ effects: setErrorLines.of(errorLines ?? []) });
  }

  /** Public: scroll to and flash a 1-based line (called from LogPanel via event). */
  export function jumpTo(line: number) {
    if (!view || line < 1 || line > view.state.doc.lines) return;
    const pos = view.state.doc.line(line).from;
    view.dispatch({ selection: { anchor: pos }, effects: EditorView.scrollIntoView(pos, { y: "center" }) });
  }
</script>

<div class="codehost" bind:this={host}></div>

<style>
  .codehost { height: 100%; }
  :global(.codehost .cm-editor) { height: 100%; }
</style>
```

- [ ] **Step 3: Wire jump-to-line via a shared store callback**

So `LogPanel` can call `jumpTo`, expose it through a store. Add to `app/src/lib/stores.ts`:
```ts
export const jumpToLine = writable<(line: number) => void>(() => {});
```
In `CodeViewer.svelte`, after `onMount` sets up `view`, register it: add `import { jumpToLine } from "./stores";` and inside `onMount` (after creating `view`) `jumpToLine.set(jumpTo);`.

- [ ] **Step 4: Verify type-check + dev render**

Run:
```bash
cd app && npx tsc --noEmit && npm run tauri dev
```
Expected: compiles; an empty dark code pane renders. Close it.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/CodeViewer.svelte app/src/lib/stores.ts app/package.json app/package-lock.json
git commit -m "feat(app): CodeMirror XML viewer with error-line highlight"
```

---

### Task 13: LogPanel (filter + search + click-to-jump)

**Files:**
- Modify: `app/src/lib/LogPanel.svelte`

- [ ] **Step 1: Implement LogPanel**

Replace `app/src/lib/LogPanel.svelte`:
```svelte
<script lang="ts">
  import { selectedResult, logFilter, search, jumpToLine } from "./stores";
  import { statusLabel } from "./types";
  import type { Message } from "./types";

  $: msgs = filterMsgs($selectedResult?.messages ?? [], $logFilter, $search);

  function filterMsgs(all: Message[], filter: string, q: string): Message[] {
    const term = q.trim().toLowerCase();
    return all.filter((m) => {
      if (filter === "errors" && m.severity !== "error") return false;
      if (filter === "warnings" && m.severity !== "warning") return false;
      if (term && !m.text.toLowerCase().includes(term)) return false;
      return true;
    });
  }
  function click(m: Message) {
    if (m.line) $jumpToLine(m.line);
  }
</script>

<div class="logpanel">
  <header>
    {#if $selectedResult}
      <div class="status {$selectedResult.status}">{statusLabel($selectedResult)}</div>
      <div class="meta">{$selectedResult.schema || "—"}</div>
    {:else}
      <div class="meta">Select a file to see its log.</div>
    {/if}
    <div class="controls">
      <input placeholder="Search…" bind:value={$search} />
      <div class="filters">
        <button class:active={$logFilter === "errors"} on:click={() => logFilter.set("errors")}>Errors</button>
        <button class:active={$logFilter === "warnings"} on:click={() => logFilter.set("warnings")}>Warnings</button>
        <button class:active={$logFilter === "all"} on:click={() => logFilter.set("all")}>All</button>
      </div>
    </div>
  </header>

  <ul>
    {#each msgs as m, i}
      <li class={m.severity} class:clickable={!!m.line} on:click={() => click(m)}>
        <span class="badge">{m.severity === "error" ? "ERROR" : "WARN"}</span>
        <span class="text">{m.text}</span>
        {#if m.line}<span class="loc">L{m.line}{m.column ? `:${m.column}` : ""}</span>{/if}
      </li>
    {/each}
    {#if $selectedResult && msgs.length === 0}
      <li class="none">No messages match.</li>
    {/if}
  </ul>
</div>

<style>
  .logpanel { display: grid; grid-template-rows: auto 1fr; height: 100%; }
  header { padding: 8px 10px; border-bottom: 1px solid var(--border); display: grid; gap: 6px; }
  .status { font-weight: 700; }
  .status.ok { color: var(--ok); } .status.invalid, .status.error { color: var(--err); }
  .status.warnings, .status.no_schema { color: var(--warn); }
  .meta { color: #888; font-size: 12px; }
  .controls { display: flex; gap: 8px; align-items: center; }
  .controls input { flex: 1; padding: 4px 8px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; }
  .filters button { border: 1px solid var(--border); background: var(--panel); color: var(--fg); padding: 3px 8px; border-radius: 6px; cursor: pointer; }
  .filters button.active { background: var(--accent); color: #fff; border-color: var(--accent); }
  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  li { display: flex; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); align-items: baseline; }
  li.clickable { cursor: pointer; }
  li.clickable:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .badge { font-size: 11px; font-weight: 700; padding: 1px 5px; border-radius: 4px; }
  li.error .badge { background: var(--err); color: #fff; }
  li.warning .badge { background: var(--warn); color: #fff; }
  .loc { margin-left: auto; color: #888; font-variant-numeric: tabular-nums; }
  .none { color: #888; }
</style>
```

- [ ] **Step 2: Verify type-check**

Run:
```bash
cd app && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/lib/LogPanel.svelte
git commit -m "feat(app): filterable, clickable validation log"
```

---

### Task 14: SummaryBar + progress

**Files:**
- Modify: `app/src/lib/SummaryBar.svelte`

- [ ] **Step 1: Implement SummaryBar**

Replace `app/src/lib/SummaryBar.svelte`:
```svelte
<script lang="ts">
  import { summary, progress } from "./stores";
</script>

<footer class="summary">
  {#if $progress.running}
    <div class="bar"><div class="fill" style="width:{$progress.total ? ($progress.done / $progress.total) * 100 : 0}%"></div></div>
    <span>Validating {$progress.done}/{$progress.total}…</span>
  {:else}
    <span>{$summary.total} files</span>
    <span class="ok">OK {$summary.ok}</span>
    <span class="err">Invalid {$summary.invalid}</span>
    <span class="warn">Warnings {$summary.warnings}</span>
    <span class="warn">No schema {$summary.noSchema}</span>
  {/if}
</footer>

<style>
  .summary { display: flex; gap: 14px; align-items: center; padding: 6px 12px; border-top: 1px solid var(--border); background: var(--panel); font-size: 13px; }
  .ok { color: var(--ok); } .err { color: var(--err); } .warn { color: var(--warn); }
  .bar { flex: 0 0 220px; height: 8px; background: var(--bg); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 120ms; }
</style>
```

- [ ] **Step 2: Verify type-check**

Run:
```bash
cd app && npx tsc --noEmit
```
Expected: no errors.

- [ ] **Step 3: Commit**

```bash
git add app/src/lib/SummaryBar.svelte
git commit -m "feat(app): summary bar with live progress"
```

---

### Task 15: Toolbar — pickers, drag & drop, run validation

**Files:**
- Modify: `app/src/lib/Toolbar.svelte`

- [ ] **Step 1: Implement Toolbar wiring the backend**

Replace `app/src/lib/Toolbar.svelte`:
```svelte
<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { startValidation } from "./api";
  import { results, selectedIndex, progress, theme } from "./stores";
  import type { ValidationEvent } from "./types";

  async function run(paths: string[]) {
    if (paths.length === 0) return;
    results.set([]);
    selectedIndex.set(-1);
    progress.set({ done: 0, total: 0, running: true });
    await startValidation(paths, (ev: ValidationEvent) => {
      if (ev.event === "started") {
        progress.set({ done: 0, total: ev.data.total, running: true });
      } else if (ev.event === "result") {
        results.update((r) => { r[ev.data.index] = ev.data.result; return r; });
        progress.update((p) => ({ ...p, done: p.done + 1 }));
        if (ev.data.index === 0) selectedIndex.set(0);
      } else if (ev.event === "finished") {
        progress.update((p) => ({ ...p, running: false }));
      }
    });
  }

  async function pickFiles() {
    const sel = await open({ multiple: true, filters: [{ name: "XML", extensions: ["xml"] }] });
    if (sel) run(Array.isArray(sel) ? sel : [sel]);
  }
  async function pickFolder() {
    const sel = await open({ directory: true });
    if (sel) run([sel as string]);
  }
  function toggleTheme() {
    theme.update((t) => (t === "dark" ? "light" : "dark"));
  }

  onMount(() => {
    const un = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") run(event.payload.paths);
    });
    return () => { un.then((f) => f()); };
  });
</script>

<header class="toolbar">
  <strong class="brand">SEPA XML Validator</strong>
  <button on:click={pickFiles}>Select Files…</button>
  <button on:click={pickFolder}>Select Folder…</button>
  <span class="hint">or drag &amp; drop files here</span>
  <button class="theme" on:click={toggleTheme} title="Toggle theme">◐</button>
</header>

<style>
  .toolbar { display: flex; gap: 10px; align-items: center; padding: 8px 12px; background: var(--accent); color: #fff; }
  .brand { margin-right: 8px; }
  .toolbar button { background: rgba(255,255,255,0.15); color: #fff; border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; }
  .toolbar button:hover { background: rgba(255,255,255,0.28); }
  .hint { opacity: 0.85; font-size: 12px; }
  .theme { margin-left: auto; }
</style>
```

- [ ] **Step 2: Add drag-drop capability**

Ensure `app/src-tauri/capabilities/default.json` includes `"core:webview:allow-internal-toggle"` is not needed, but drag-drop is on by default; confirm `app.windows[0]` has `"dragDropEnabled": true` (default) in `tauri.conf.json`. The dialog/fs permissions were added in Task 8.

- [ ] **Step 3: Verify end-to-end (manual)**

Run:
```bash
cd app && npm run tauri dev
```
Manual checks:
1. Drag a folder of XML onto the window → files stream into the list live; progress bar advances.
2. Select an invalid file → log shows red errors with line numbers; code pane shows the XML.
3. Click an error with a line → code pane scrolls to and highlights that line.
4. Filter/search narrow the log.
Close it.

- [ ] **Step 4: Commit**

```bash
git add app/src/lib/Toolbar.svelte app/src-tauri/tauri.conf.json
git commit -m "feat(app): toolbar with pickers, drag-drop, live run"
```

---

### Task 16: Theme application

**Files:**
- Modify: `app/src/main.ts`

- [ ] **Step 1: Apply theme to documentElement**

In `app/src/main.ts`, after mounting the app, add reactive theme application:
```ts
import { theme } from "./lib/stores";

const mq = window.matchMedia("(prefers-color-scheme: dark)");
function apply(t: "system" | "light" | "dark") {
  const dark = t === "dark" || (t === "system" && mq.matches);
  document.documentElement.setAttribute("data-theme", dark ? "dark" : "light");
}
theme.subscribe(apply);
mq.addEventListener("change", () => {
  let cur: "system" | "light" | "dark" = "system";
  theme.subscribe((t) => (cur = t))();
  apply(cur);
});
```

- [ ] **Step 2: Verify**

Run:
```bash
cd app && npm run tauri dev
```
Expected: app follows OS theme; the toolbar ◐ button toggles light/dark. Close it.

- [ ] **Step 3: Commit**

```bash
git add app/src/main.ts
git commit -m "feat(app): system-aware theme with manual toggle"
```

---

### Task 17: Export TXT + CSV

**Files:**
- Create: `app/src/lib/export.ts`
- Modify: `app/src/lib/Toolbar.svelte` (add Export button)

- [ ] **Step 1: Export helpers**

Create `app/src/lib/export.ts`:
```ts
import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "@tauri-apps/plugin-fs";
import type { ValidationResult } from "./types";
import { statusLabel } from "./types";

function stamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

export async function exportTxt(results: ValidationResult[]): Promise<void> {
  const path = await save({ defaultPath: `SEPA_Validation_${stamp()}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
  if (!path) return;
  const ok = results.filter((r) => r.status === "ok").length;
  let out = `SEPA XML Validation - ${new Date().toISOString()}\n${results.length} files | OK: ${ok} | Failed: ${results.length - ok}\n${"=".repeat(80)}\n`;
  for (const r of results) {
    out += `\nFile: ${r.path}\nNamespace: ${r.namespace}\nSchema: ${r.schema}\nStatus: ${statusLabel(r)}\n`;
    r.messages.forEach((m, i) => {
      const loc = m.line ? ` (Line ${m.line}${m.column ? `, Col ${m.column}` : ""})` : "";
      out += `[${i + 1}] ${m.severity.toUpperCase()}: ${m.text}${loc}\n`;
    });
    out += `${"-".repeat(80)}\n`;
  }
  await writeTextFile(path, out);
}

export async function exportCsv(results: ValidationResult[]): Promise<void> {
  const path = await save({ defaultPath: `SEPA_Validation_${stamp()}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
  let out = "file;namespace;schema;status;errors;warnings\n";
  for (const r of results) {
    out += [esc(r.file), esc(r.namespace), esc(r.schema), esc(statusLabel(r)), r.errors, r.warnings].join(";") + "\n";
  }
  await writeTextFile(path, out);
}
```

- [ ] **Step 2: Add Export button + menu to Toolbar**

In `app/src/lib/Toolbar.svelte` `<script>`, add:
```ts
  import { exportTxt, exportCsv } from "./export";
  import { get } from "svelte/store";
  function doExportTxt() { exportTxt(get(results)); }
  function doExportCsv() { exportCsv(get(results)); }
```
And in the markup, after the folder button, add:
```svelte
  <button on:click={doExportTxt} disabled={$results.length === 0}>Export TXT</button>
  <button on:click={doExportCsv} disabled={$results.length === 0}>Export CSV</button>
```

- [ ] **Step 3: Verify fs capability allows writes**

Confirm `app/src-tauri/capabilities/default.json` contains `"fs:allow-write-text-file"` and dialog `save`. If `writeTextFile` is denied at runtime, add `"fs:default"` + `"fs:allow-write-text-file"` to the capability permissions and restart.

- [ ] **Step 4: Verify (manual)**

Run `npm run tauri dev`, validate some files, click Export TXT and Export CSV, confirm files are written and contents look right.

- [ ] **Step 5: Commit**

```bash
git add app/src/lib/export.ts app/src/lib/Toolbar.svelte app/src-tauri/capabilities
git commit -m "feat(app): TXT and CSV export of results"
```

---

## Phase 4 — Wrap-up

### Task 18: Build, docs, regression pass

**Files:**
- Create: `app/README.md`
- Modify: `CLAUDE.md` (add the Rust/Tauri app section)

- [ ] **Step 1: Full Rust test pass**

Run:
```bash
cd app/src-tauri && cargo test
```
Expected: all unit tests PASS; fixture-dependent tests PASS or SKIP. No failures.

- [ ] **Step 2: Production build**

Run:
```bash
cd app && npm run tauri build
```
Expected: produces an installer/exe under `app/src-tauri/target/release/`. Launch it once and validate a folder to confirm the bundled build works (schemas embedded).

- [ ] **Step 3: Write app README**

Create `app/README.md` documenting: prerequisites (Node, Rust, vcpkg + libxml2 with the exact triplet from Task 2), `npm run tauri dev`, `npm run tauri build`, and the note that `xml_schema/` must be present at build time (embedded, not redistributed).

- [ ] **Step 4: Update CLAUDE.md**

Add a "Rust/Tauri App (`app/`)" subsection under Architecture summarizing: Tauri v2 + Svelte-TS, libxml2 validation engine, streaming Channel events, embedded schemas via `build.rs`, and the dev/build commands.

- [ ] **Step 5: Commit + push branch**

```bash
git add app/README.md CLAUDE.md
git commit -m "docs(app): build instructions and CLAUDE.md update"
git push -u origin feature/tauri-rust-rewrite
```

- [ ] **Step 6: Finish the branch**

Use the superpowers:finishing-a-development-branch skill to decide on merge/PR.

---

## Self-Review (completed by plan author)

**Spec coverage:**
- Tauri + Svelte/TS → Tasks 1, 9–17 ✓
- libxml2 XSD engine, linked, embedded schemas → Tasks 2, 4, 6 ✓
- Live-stream log → Task 8 (Channel) + Task 15 (live updates) ✓
- Clickable → XML line → Task 12 (jumpTo) + Task 13 (click) ✓
- Filter & search → Task 13 ✓
- Color + grouped + summary → Tasks 11, 13, 14 ✓
- System theme + toggle → Task 16 ✓
- TXT + CSV export → Task 17 ✓
- Status semantics (OK/INVALID/WARNINGS/NO SCHEMA/ERROR) → Task 3 + Task 6 ✓
- Tests against valid/invalid fixtures → Task 6 ✓
- Out of scope (rename/sort, cross-platform) → not included ✓

**Type consistency:** `ValidationResult`/`Message`/`Status`/`Severity` defined in Task 3 (Rust) and mirrored in Task 9 (TS); `ValidationEvent` shape in Task 8 (Rust, `tag="event"`, `content="data"`, camelCase) matches Task 9 (TS union) and Task 15 consumer. `jumpToLine` store defined in Task 12 and consumed in Task 13. `startValidation`/`readFile` defined in Task 9, used in Tasks 12/15.

**Known empirical points (de-risked by the spike, Task 2):** exact `libxml` `StructuredError` field names (Task 6 `to_message`), vcpkg triplet for libxml2, and physical XSD filenames behind each namespace (Task 4).
