# SEPA Validator v2.0.0 Enhancements — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an app icon, resizable panels, pretty-printed XML with matching error line numbers, click-to-center error highlighting, and ship a `v2.0.0-beta.1` draft release.

**Architecture:** XML is pretty-printed in the Rust backend with quick-xml's indenting writer; validation runs on the *formatted* text so libxml's error line/col match what the viewer shows (`read_formatted` returns the same formatted string). The Svelte UI gets draggable column gutters and a CodeMirror "active error line" decoration that scrolls to center.

**Tech Stack:** Rust, quick-xml (formatting), libxml (validation), Tauri v2, Svelte 5 + TypeScript, CodeMirror 6, Python/Pillow (icon source), `tauri icon`.

**Reference spec:** `docs/superpowers/specs/2026-06-18-app-enhancements-v2-design.md`

## Conventions
- All work in `app/` on branch `feature/tauri-rust-rewrite`. The gitignored `app/src-tauri/.cargo/config.toml` provides the libxml2/libclang build env.
- Verify non-interactively: `cargo test`, `cargo build`, `npm run check`, `npm run build`. Do NOT run `tauri dev` (opens a blocking window).
- Backend module declarations live in `app/src-tauri/src/lib.rs`.

## File Structure
- `app/src-tauri/src/formatting.rs` — NEW: `format_xml(path) -> Result<String, String>`.
- `app/src-tauri/src/validator.rs` — MODIFY: validate the formatted text; parse from string.
- `app/src-tauri/src/commands.rs` — MODIFY: add `read_formatted`.
- `app/src-tauri/src/lib.rs` — MODIFY: `mod formatting;`, register `read_formatted`.
- `app/src/lib/api.ts` — MODIFY: add `readFormatted`.
- `app/src/lib/CodeViewer.svelte` — MODIFY: use `readFormatted`; active-line decoration + center scroll.
- `app/src/App.svelte` + `app/src/styles.css`/`app.css` — MODIFY: draggable gutters.
- `app/src-tauri/icons/` (generated) + `app/src-tauri/icons/source.png` — NEW: icon.
- `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml` — MODIFY: version 2.0.0.

---

### Task 1: XML formatter (`format_xml`)

**Files:**
- Create: `app/src-tauri/src/formatting.rs`
- Modify: `app/src-tauri/src/lib.rs` (add `mod formatting;`)

- [ ] **Step 1: Write the module + tests**

Create `app/src-tauri/src/formatting.rs`:
```rust
//! Deterministic XML pretty-printer used for BOTH the code viewer and the
//! validation target, so error line/col numbers match the displayed text.

use std::path::Path;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

/// Pretty-print the XML at `path` with 2-space indentation.
/// Returns Err (with a message) if the file isn't readable or well-formed.
pub fn format_xml(path: &Path) -> Result<String, String> {
    let mut reader = Reader::from_file(path).map_err(|e| e.to_string())?;
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Vec::new(), b' ', 2);
    let mut buf = Vec::new();
    loop {
        let event = reader.read_event_into(&mut buf).map_err(|e| e.to_string())?;
        if matches!(event, Event::Eof) {
            break;
        }
        writer.write_event(event).map_err(|e| e.to_string())?;
        buf.clear();
    }
    String::from_utf8(writer.into_inner()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_xml(name: &str, contents: &str) -> std::path::PathBuf {
        let mut p = std::env::temp_dir();
        p.push(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(contents.as_bytes()).unwrap();
        p
    }

    #[test]
    fn single_line_becomes_multiline() {
        let p = temp_xml(
            "sepa_fmt_single.xml",
            r#"<?xml version="1.0"?><Document xmlns="urn:x"><A><B>1</B><C>2</C></A></Document>"#,
        );
        let out = format_xml(&p).unwrap();
        assert!(out.lines().count() > 3, "expected multiple lines, got:\n{out}");
        assert!(out.contains("<B>1</B>"));
        assert!(out.contains("<C>2</C>"));
    }

    #[test]
    fn malformed_xml_errors() {
        let p = temp_xml("sepa_fmt_bad.xml", "<a><b></a>");
        assert!(format_xml(&p).is_err());
    }
}
```
Add `mod formatting;` to `app/src-tauri/src/lib.rs`.

- [ ] **Step 2: Run tests**

Run: `cd app/src-tauri && cargo test formatting:: -- --nocapture`
Expected: 2 tests PASS. If the quick-xml 0.36 API differs (e.g. `config_mut().trim_text` vs `trim_text(true)` builder, or `write_event` wanting `&event`), adjust minimally to the installed API keeping behavior identical, and note the change.

- [ ] **Step 3: Commit**

```bash
git add app/src-tauri/src/formatting.rs app/src-tauri/src/lib.rs
git commit -m "feat(app): deterministic XML pretty-printer"
```

---

### Task 2: Validate the formatted text

**Files:**
- Modify: `app/src-tauri/src/validator.rs`

Currently `validate_file` parses the original file. Change it to validate the *formatted* string so error lines match the viewer. Keep all status/edge handling.

- [ ] **Step 1: Update validate_file**

In `app/src-tauri/src/validator.rs`, add the import near the top (with the other `use crate::...` lines):
```rust
use crate::formatting::format_xml;
```
Then replace the document-parse + validate section. Find this block:
```rust
        let doc = match Parser::default().parse_file(&path_str) {
            Ok(d) => d,
            Err(e) => return mk(ns.clone(), schema_name.to_string(),
                vec![Message { severity: Severity::Error, text: format!("XML parse error: {e:?}"), line: None, column: None }],
                Status::Error, 1, 0),
        };

        let messages = match validator.validate_document(&doc) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(to_message).collect(),
        };
```
and replace it with (parse the FORMATTED text so line numbers match what the viewer shows):
```rust
        let formatted = match format_xml(path) {
            Ok(s) => s,
            Err(e) => return mk(ns.clone(), schema_name.to_string(),
                vec![Message { severity: Severity::Error, text: format!("XML parse error: {e}"), line: None, column: None }],
                Status::Error, 1, 0),
        };

        let doc = match Parser::default().parse_string(&formatted) {
            Ok(d) => d,
            Err(e) => return mk(ns.clone(), schema_name.to_string(),
                vec![Message { severity: Severity::Error, text: format!("XML parse error: {e:?}"), line: None, column: None }],
                Status::Error, 1, 0),
        };

        let messages = match validator.validate_document(&doc) {
            Ok(()) => Vec::new(),
            Err(errors) => errors.iter().map(to_message).collect(),
        };
```
If `Parser::parse_string` has a different name in the `libxml` crate (e.g. `parse_string(&self, &str)` vs `parse_string(s: &str)`), adjust to the real API (it parses an in-memory string into a `Document`) and report the signature used.

- [ ] **Step 2: Add a consistency test**

Append inside the existing `#[cfg(test)] mod tests` block in `validator.rs`:
```rust
    #[test]
    fn invalid_fixture_lines_point_into_formatted_text() {
        let f = repo_root().join("to_check/invalid/20250121_NOFIRMA_PAIN00100109_1.xml");
        if !f.exists() { eprintln!("SKIP: fixture absent"); return; }
        let formatted = crate::formatting::format_xml(&f).unwrap();
        let line_count = formatted.lines().count() as u32;
        let mut v = super::Validator::new();
        let r = v.validate_file(&f);
        assert_eq!(r.status, Status::Invalid);
        for m in r.messages.iter() {
            if let Some(line) = m.line {
                assert!(line >= 1 && line <= line_count,
                    "error line {line} outside formatted text (1..={line_count})");
            }
        }
        assert!(r.messages.iter().any(|m| m.line.is_some()));
    }
```

- [ ] **Step 3: Run tests**

Run: `cd app/src-tauri && cargo test validator:: -- --nocapture`
Expected: all validator tests PASS, including the new one. `valid_fixture_is_ok` must still be `Ok` (formatting must not change the verdict). If `valid_fixture_is_ok` regresses, the formatter is altering content — investigate `trim_text`/round-trip fidelity, don't weaken the assertion.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/validator.rs
git commit -m "feat(app): validate pretty-printed XML so error lines match the viewer"
```

---

### Task 3: `read_formatted` command + frontend wiring

**Files:**
- Modify: `app/src-tauri/src/commands.rs`, `app/src-tauri/src/lib.rs`, `app/src/lib/api.ts`, `app/src/lib/CodeViewer.svelte`

- [ ] **Step 1: Add the command**

In `app/src-tauri/src/commands.rs`, add (and `use crate::formatting::format_xml;` at the top):
```rust
/// Return the pretty-printed XML for the viewer. Falls back to raw bytes if the
/// file isn't well-formed (so the user still sees the content).
#[tauri::command]
pub fn read_formatted(path: String) -> Result<String, String> {
    match crate::formatting::format_xml(std::path::Path::new(&path)) {
        Ok(s) => Ok(s),
        Err(_) => std::fs::read(&path)
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .map_err(|e| e.to_string()),
    }
}
```
In `app/src-tauri/src/lib.rs`, add `commands::read_formatted` to the `generate_handler!` list (so: `start_validation, read_file, write_text_file, read_formatted`).

- [ ] **Step 2: Frontend wrapper + viewer**

Append to `app/src/lib/api.ts`:
```ts
export function readFormatted(path: string): Promise<string> {
  return invoke<string>("read_formatted", { path });
}
```
In `app/src/lib/CodeViewer.svelte`, change the import `import { readFile } from "./api";` to `import { readFormatted } from "./api";` and replace the call `text = await readFile(path);` with `text = await readFormatted(path);`.

- [ ] **Step 3: Verify**

Run: `cd app/src-tauri && cargo build` (expect success), then `cd app && npm run check` (expect 0 errors).

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/commands.rs app/src-tauri/src/lib.rs app/src/lib/api.ts app/src/lib/CodeViewer.svelte
git commit -m "feat(app): read_formatted command; viewer shows pretty-printed XML"
```

---

### Task 4: Active-line highlight + scroll to center

**Files:**
- Modify: `app/src-tauri/.. none`; `app/src/lib/CodeViewer.svelte`

- [ ] **Step 1: Add an active-line decoration**

In `app/src/lib/CodeViewer.svelte`, after the existing `errorField` StateField definition, add a second field for the clicked/active line:
```ts
  const setActiveLine = StateEffect.define<number | null>();
  const activeLineField = StateField.define<DecorationSet>({
    create: () => Decoration.none,
    update(deco, tr) {
      deco = deco.map(tr.changes);
      for (const e of tr.effects) {
        if (e.is(setActiveLine)) {
          if (e.value == null || e.value < 1 || e.value > tr.state.doc.lines) {
            deco = Decoration.none;
          } else {
            const line = tr.state.doc.line(e.value);
            deco = Decoration.set([Decoration.line({ class: "cm-active-error-line" }).range(line.from)]);
          }
        }
      }
      return deco;
    },
    provide: (f) => EditorView.decorations.from(f),
  });
```

- [ ] **Step 2: Register the field + theme; update jumpTo**

In the `EditorState.create({ extensions: [...] })` array, add `activeLineField` (after `errorField`) and extend the `EditorView.theme({...})` to include the active style:
```ts
          EditorView.theme({
            ".cm-error-line": { backgroundColor: "rgba(244,71,71,0.18)" },
            ".cm-active-error-line": {
              backgroundColor: "rgba(244,71,71,0.38)",
              boxShadow: "inset 3px 0 0 #f44747",
              animation: "cm-flash 0.6s ease-out",
            },
            "@keyframes cm-flash": {
              from: { backgroundColor: "rgba(244,71,71,0.75)" },
              to: { backgroundColor: "rgba(244,71,71,0.38)" },
            },
          }),
```
Then update `jumpTo` to set the active line AND scroll to center:
```ts
  function jumpTo(line: number) {
    if (!view || line < 1 || line > view.state.doc.lines) return;
    const pos = view.state.doc.line(line).from;
    view.dispatch({
      effects: [setActiveLine.of(line), EditorView.scrollIntoView(pos, { y: "center" })],
    });
  }
```
Also, when the selected file changes, clear the active line: in `loadFor`, after the `setErrorLines` dispatch, the active line from a previous file should reset — add `setActiveLine.of(null)` to that dispatch's effects:
```ts
    view.dispatch({ effects: [setErrorLines.of(errorLines ?? []), setActiveLine.of(null)] });
```

- [ ] **Step 3: Verify**

Run: `cd app && npm run check` (expect 0 errors) and `npm run build` (expect success).

- [ ] **Step 4: Commit**

```bash
git add app/src/lib/CodeViewer.svelte
git commit -m "feat(app): emphasize and center the clicked error line"
```

---

### Task 5: Resizable panels (draggable gutters)

**Files:**
- Modify: `app/src/App.svelte`, and the global stylesheet `app/src/app.css`

- [ ] **Step 1: Replace App.svelte with gutters**

Replace `app/src/App.svelte` entirely:
```svelte
<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";

  let leftWidth = 260;
  let rightWidth = 360;

  function startDrag(which: "left" | "right", e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startLeft = leftWidth;
    const startRight = rightWidth;
    const onMove = (ev: MouseEvent) => {
      const dx = ev.clientX - startX;
      if (which === "left") {
        leftWidth = Math.min(480, Math.max(160, startLeft + dx));
      } else {
        rightWidth = Math.min(640, Math.max(240, startRight - dx));
      }
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }
</script>

<div class="app">
  <Toolbar />
  <main class="body" style="grid-template-columns: {leftWidth}px 6px 1fr 6px {rightWidth}px;">
    <aside class="files"><FileList /></aside>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("left", e)}></div>
    <section class="viewer"><CodeViewer /></section>
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("right", e)}></div>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
</div>
```

- [ ] **Step 2: Update the layout CSS**

In `app/src/app.css`, replace the `.body` rule and the `.files, .viewer, .log` border rule with:
```css
.body { display: grid; min-height: 0; }
.files, .viewer, .log { min-height: 0; overflow: hidden; background: var(--panel); }
.gutter { background: var(--border); cursor: col-resize; }
.gutter:hover { background: var(--accent); }
```
(The grid columns are now driven inline by `App.svelte`; the gutters replace the old fixed borders.)

- [ ] **Step 3: Verify**

Run: `cd app && npm run check` (expect 0 errors) and `npm run build` (expect success). If `npm run check` warns about a different a11y rule on the gutter, change the `svelte-ignore` comment to the exact rule it reports.

- [ ] **Step 4: Commit**

```bash
git add app/src/App.svelte app/src/app.css
git commit -m "feat(app): draggable gutters to resize the side panels"
```

---

### Task 6: App icon

**Files:**
- Create: `app/src-tauri/icons/source.png` (and the generated icon set under `app/src-tauri/icons/`)

- [ ] **Step 1: Generate the source PNG**

Ensure Pillow is available, then run this Python to draw a 1024×1024 icon (blue rounded square, white document with folded corner + text lines, green check badge). Save it to `app/src-tauri/icons/source.png`:
```python
# scripts/gen_icon.py  (run from repo root: python scripts/gen_icon.py)
from PIL import Image, ImageDraw
S = 1024
img = Image.new("RGBA", (S, S), (0, 0, 0, 0))
d = ImageDraw.Draw(img)
# Background: rounded square, app blue
d.rounded_rectangle([40, 40, S-40, S-40], radius=180, fill=(10, 132, 255, 255))
# Document
dx0, dy0, dx1, dy1 = 300, 230, 724, 770
fold = 120
d.polygon([(dx0, dy0), (dx1 - fold, dy0), (dx1, dy0 + fold), (dx1, dy1), (dx0, dy1)],
          fill=(255, 255, 255, 255))
d.polygon([(dx1 - fold, dy0), (dx1 - fold, dy0 + fold), (dx1, dy0 + fold)], fill=(205, 225, 250, 255))
# Text lines on the document
for i, y in enumerate(range(330, 620, 70)):
    w = 360 if i % 3 != 2 else 220
    d.rounded_rectangle([350, y, 350 + w, y + 26], radius=13, fill=(150, 175, 205, 255))
# Green check badge (bottom-right)
cx, cy, r = 690, 700, 120
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=(22, 163, 74, 255))
d.line([(cx - 52, cy + 4), (cx - 12, cy + 46), (cx + 58, cy - 42)], fill=(255, 255, 255, 255), width=26, joint="curve")
img.save("app/src-tauri/icons/source.png")
print("wrote app/src-tauri/icons/source.png")
```
Run: `pip install pillow` (if needed), then `python scripts/gen_icon.py`.

- [ ] **Step 2: Generate the icon set**

Run: `cd app && npx tauri icon src-tauri/icons/source.png`
Expected: writes `icon.ico`, `icon.png`, `32x32.png`, `128x128.png`, `Square*Logo.png`, etc. into `app/src-tauri/icons/`. Confirm `app/src-tauri/tauri.conf.json` `bundle.icon` references those files (it does by default).

- [ ] **Step 3: Verify the build picks up the icon**

Run: `cd app/src-tauri && cargo build` (expect success; Tauri embeds `icon.ico`). (Visual confirmation happens at release/dev time.)

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/icons scripts/gen_icon.py
git commit -m "feat(app): custom application icon"
```

---

### Task 7: Version bump, build, and v2.0.0-beta.1 draft release

**Files:**
- Modify: `app/src-tauri/tauri.conf.json`, `app/src-tauri/Cargo.toml`

- [ ] **Step 1: Bump version to 2.0.0**

In `app/src-tauri/tauri.conf.json` set `"version": "2.0.0"`. In `app/src-tauri/Cargo.toml` set `version = "2.0.0"` under `[package]`.

- [ ] **Step 2: Full verification**

Run: `cd app/src-tauri && cargo test` (all tests pass), then `cd app && npm run check` (0 errors) and `npm run build` (success).

- [ ] **Step 3: Build the installer**

Run: `cd app && npx tauri build --bundles nsis`
Expected: `app/src-tauri/target/release/bundle/nsis/SEPA Validator_2.0.0_x64-setup.exe` and `app/src-tauri/target/release/app.exe`.

- [ ] **Step 4: Stage release assets (clean names)**

```bash
REL="app/src-tauri/target/release"
mkdir -p "$REL/beta-assets"
cp "$REL/app.exe" "$REL/beta-assets/SEPA-Validator-2.0.0-beta.1-windows-x64-portable.exe"
cp "$REL/bundle/nsis/SEPA Validator_2.0.0_x64-setup.exe" "$REL/beta-assets/SEPA-Validator-2.0.0-beta.1-windows-x64-setup.exe"
```

- [ ] **Step 5: Commit the version bump, push, replace the draft**

```bash
git add app/src-tauri/tauri.conf.json app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock
git commit -m "chore(app): bump version to 2.0.0"
git push

# Remove the mis-versioned 0.1.0 draft, then create the 2.0.0 draft
gh release delete v0.1.0-beta.1 --yes --cleanup-tag 2>/dev/null || true
gh release create v2.0.0-beta.1 --draft --prerelease \
  --target feature/tauri-rust-rewrite \
  --title "SEPA Validator v2.0.0-beta.1 — Tauri/Rust rewrite" \
  --notes "Second beta of the Rust/Tauri rewrite. Draft / unsigned. New: custom icon, resizable panels, pretty-printed XML, click-an-error to jump to the centered line. Needs WebView2 (preinstalled on Win11). SmartScreen: More info -> Run anyway." \
  "app/src-tauri/target/release/beta-assets/SEPA-Validator-2.0.0-beta.1-windows-x64-setup.exe" \
  "app/src-tauri/target/release/beta-assets/SEPA-Validator-2.0.0-beta.1-windows-x64-portable.exe"
```
Expected: a new DRAFT pre-release `v2.0.0-beta.1` with both assets. Verify with `gh release view v2.0.0-beta.1 --json isDraft,assets`.

---

## Self-Review (by plan author)

**Spec coverage:**
- App icon → Task 6 ✓
- Resizable right (+left) panel → Task 5 ✓
- Show error line → already in LogPanel; made correct by Tasks 1–2 ✓
- Click error → center + emphasize → Task 4 ✓
- Pretty-print XML + matching line numbers → Tasks 1 (format), 2 (validate formatted), 3 (viewer reads formatted) ✓
- Version 2.0.0 + new draft release → Task 7 ✓
- Out of scope (runtime schema folder, macOS/Linux) → not included ✓

**Consistency:** `format_xml(path) -> Result<String,String>` defined in Task 1, used in Tasks 2 (validate) and 3 (read_formatted) — same signature. `readFormatted` (Task 3) consumed by CodeViewer (Task 3). `setActiveLine`/`activeLineField`/`jumpTo` all in Task 4. `leftWidth`/`rightWidth` only in App.svelte (Task 5).

**Empirical points to confirm during impl:** quick-xml 0.36 `config_mut().trim_text` / `write_event` arg form (Task 1); `libxml` `Parser::parse_string` signature (Task 2); `tauri icon` output filenames (Task 6).
