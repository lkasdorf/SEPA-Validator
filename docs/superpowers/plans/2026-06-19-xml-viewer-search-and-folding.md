# XML Viewer Search & Folding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add in-document search (Ctrl+F + a Search button) and code folding (gutter arrows + Collapse all / Expand all buttons) to the read-only XML viewer in the Tauri/Svelte app.

**Architecture:** Both features are CodeMirror 6 extensions wired into the existing `CodeViewer.svelte` editor — no backend, no Tauri command, no custom algorithm. A small button bar in `App.svelte`'s `.viewer` section drives the editor through three Svelte stores, mirroring the existing `jumpToLine` store pattern.

**Tech Stack:** Svelte 5 + TypeScript + Vite, CodeMirror 6 (`@codemirror/state`, `@codemirror/view`, `@codemirror/language`, `@codemirror/lang-xml`, `@codemirror/search`).

## Global Constraints

- CodeMirror packages stay on the 6.x line (installed: `@codemirror/state ^6.6.0`, `@codemirror/view ^6.43.1`, `@codemirror/language ^6.12.3`, `@codemirror/lang-xml ^6.1.0`). New dependency `@codemirror/search` must be a 6.x release.
- UI language is English; button labels are `Search`, `Collapse all`, `Expand all`. No localization.
- Viewer is read-only: keep `EditorView.editable.of(false)` and add `EditorState.readOnly.of(true)`.
- No frontend test runner exists in this project. Per-task verification is `cd app && npm run check` (svelte-check + tsc, expect 0 errors) plus the listed manual checks. Do **not** add a JS test framework (out of scope).
- Existing behavior must keep working: error-line highlight (`cm-error-line`), active-line flash (`cm-active-error-line`), click-to-line (`jumpTo`).
- Commit format: `type(scope): summary` (e.g. `feat(app): ...`).

---

### Task 1: Wire in-document search into the editor

Adds `@codemirror/search` and enables the Ctrl+F find panel (find-only, no replace) inside `CodeViewer.svelte`.

**Files:**
- Modify: `app/package.json` (via `npm install` — adds `@codemirror/search` to dependencies)
- Modify: `app/src/lib/CodeViewer.svelte` (imports + editor extensions)

**Interfaces:**
- Consumes: existing `EditorView` instance `view` and the `extensions` array in `onMount`.
- Produces: nothing for other tasks yet (Task 3 will call `openSearchPanel(view)`); this task just makes the search extension active.

- [ ] **Step 1: Install the search package**

Run:
```
cd app && npm install @codemirror/search
```
Expected: `package.json` gains `"@codemirror/search": "^6.x"` under `dependencies`, install succeeds.

- [ ] **Step 2: Add the search imports**

In `app/src/lib/CodeViewer.svelte`, add `keymap` to the existing `@codemirror/view` import and add a new `@codemirror/search` import. The `@codemirror/view` line changes from:

```ts
import { EditorView, lineNumbers, Decoration, type DecorationSet } from "@codemirror/view";
```

to:

```ts
import { EditorView, lineNumbers, keymap, Decoration, type DecorationSet } from "@codemirror/view";
```

Then add, directly below the `@codemirror/theme-one-dark` import:

```ts
import { search, searchKeymap, highlightSelectionMatches, openSearchPanel } from "@codemirror/search";
```

- [ ] **Step 3: Add the search extensions and read-only facet**

In `onMount`, change the `extensions` array. Current array:

```ts
extensions: [lineNumbers(), xml(), oneDark, errorField, activeLineField, EditorView.editable.of(false),
  EditorView.theme({
```

Replace the leading part (up to and including `EditorView.editable.of(false),`) with:

```ts
extensions: [lineNumbers(), xml(), oneDark,
  highlightSelectionMatches(),
  search({ top: true }),
  keymap.of(searchKeymap),
  errorField, activeLineField,
  EditorView.editable.of(false),
  EditorState.readOnly.of(true),
  EditorView.theme({
```

(`EditorState` is already imported from `@codemirror/state`; the trailing `EditorView.theme({ ... })` block and closing `]` stay unchanged.)

- [ ] **Step 4: Type-check**

Run:
```
cd app && npm run check
```
Expected: completes with 0 errors and 0 warnings.

- [ ] **Step 5: Manual verification**

Run `cd app && npx tauri dev`, load/validate an XML file so the viewer shows content, then:
- Press `Ctrl+F` → a find panel appears at the **top** of the editor.
- The panel shows find + match-case/regexp/next/previous, **no Replace fields** (because of `readOnly`).
- Type a token present in the XML → matches highlight; `Enter` / `Shift+Enter` move to next/previous; `Esc` closes the panel.

- [ ] **Step 6: Commit**

```
git add app/package.json app/package-lock.json app/src/lib/CodeViewer.svelte
git commit -m "feat(app): in-document search in XML viewer (Ctrl+F)"
```

---

### Task 2: Wire code folding into the editor

Adds clickable fold arrows in the gutter and folding key bindings, using `@codemirror/language` (already installed). XML elements are foldable because `@codemirror/lang-xml` registers `foldNodeProp` for element nodes.

**Files:**
- Modify: `app/src/lib/CodeViewer.svelte` (imports + editor extensions)

**Interfaces:**
- Consumes: the same `extensions` array in `onMount`.
- Produces: nothing for other tasks yet (Task 3 will call `foldAll(view)` / `unfoldAll(view)`); this task makes the fold gutter + keymap active.

- [ ] **Step 1: Add the folding imports**

In `app/src/lib/CodeViewer.svelte`, add directly below the `@codemirror/search` import from Task 1:

```ts
import { codeFolding, foldGutter, foldKeymap, foldAll, unfoldAll } from "@codemirror/language";
```

- [ ] **Step 2: Add the folding extensions**

Update the `extensions` array (as left by Task 1) to add `foldGutter()` next to `lineNumbers()`, add `codeFolding()`, and merge `foldKeymap` into the keymap. The array head becomes:

```ts
extensions: [lineNumbers(), foldGutter(), xml(), oneDark,
  codeFolding(),
  highlightSelectionMatches(),
  search({ top: true }),
  keymap.of([...searchKeymap, ...foldKeymap]),
  errorField, activeLineField,
  EditorView.editable.of(false),
  EditorState.readOnly.of(true),
  EditorView.theme({
```

(Only two lines change versus Task 1: `lineNumbers(), foldGutter(),` and the merged `keymap.of([...searchKeymap, ...foldKeymap])`; plus the new `codeFolding()` line.)

- [ ] **Step 3: Type-check**

Run:
```
cd app && npm run check
```
Expected: 0 errors, 0 warnings.

- [ ] **Step 4: Manual verification**

In `npx tauri dev` with an XML loaded:
- A fold gutter (arrow column) appears to the right of the line numbers.
- Hovering a line that starts an XML element shows a `▾` arrow; clicking it folds that element to a single line; the arrow becomes `▸`; clicking again unfolds.
- Error-line highlight, active-line flash, and click-to-line from the log still work (regression check).

- [ ] **Step 5: Commit**

```
git add app/src/lib/CodeViewer.svelte
git commit -m "feat(app): code folding in XML viewer (gutter arrows)"
```

---

### Task 3: Viewer button bar (Search / Collapse all / Expand all)

Adds the visible bar above the editor. Three new stores (matching the existing `jumpToLine` pattern) expose the editor commands; `CodeViewer` populates them on mount; `App.svelte` renders the bar and calls them. Buttons are disabled when no document is loaded.

**Files:**
- Modify: `app/src/lib/stores.ts` (three new stores)
- Modify: `app/src/lib/CodeViewer.svelte` (populate stores in `onMount`)
- Modify: `app/src/App.svelte` (bar markup + store imports)
- Modify: `app/src/app.css` (`.viewer` flex column + `.viewer-bar` styles)

**Interfaces:**
- Consumes: `openSearchPanel`, `foldAll`, `unfoldAll` (imported into `CodeViewer.svelte` in Tasks 1–2); the editor `view`; the existing `selectedResult` store.
- Produces: stores `openViewerSearch`, `foldAllInViewer`, `unfoldAllInViewer`, each `Writable<() => void>`, called from `App.svelte` as `$openViewerSearch()` etc.

- [ ] **Step 1: Add the three stores**

In `app/src/lib/stores.ts`, append after the existing `jumpToLine` line (`export const jumpToLine = writable<(line: number) => void>(() => {});`):

```ts
export const openViewerSearch = writable<() => void>(() => {});
export const foldAllInViewer = writable<() => void>(() => {});
export const unfoldAllInViewer = writable<() => void>(() => {});
```

- [ ] **Step 2: Populate the stores from CodeViewer**

In `app/src/lib/CodeViewer.svelte`, extend the stores import line. Change:

```ts
import { selectedResult, jumpToLine } from "./stores";
```

to:

```ts
import { selectedResult, jumpToLine, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./stores";
```

Then in `onMount`, directly after the existing `jumpToLine.set(jumpTo);` line, add:

```ts
openViewerSearch.set(() => { if (view) openSearchPanel(view); });
foldAllInViewer.set(() => { if (view) foldAll(view); });
unfoldAllInViewer.set(() => { if (view) unfoldAll(view); });
```

- [ ] **Step 3: Add the bar to App.svelte**

In `app/src/App.svelte`, add to the `<script>` block (after the existing component imports):

```ts
import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./lib/stores";
```

Then replace the viewer section. Change:

```svelte
    <section class="viewer"><CodeViewer /></section>
```

to:

```svelte
    <section class="viewer">
      <div class="viewer-bar">
        <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
        <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
        <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
      </div>
      <CodeViewer />
    </section>
```

- [ ] **Step 4: Lay out the bar in CSS**

In `app/src/app.css`, find the line:

```css
.gutter { background: var(--border); cursor: col-resize; }
```

and insert directly above it:

```css
.viewer { display: flex; flex-direction: column; }
.viewer-bar { display: flex; gap: 8px; padding: 6px 8px; border-bottom: 1px solid var(--border); flex: 0 0 auto; }
.viewer-bar button { background: var(--accent); color: #fff; border: none; padding: 4px 10px; border-radius: 6px; cursor: pointer; font-size: 12px; }
.viewer-bar button:hover:not(:disabled) { filter: brightness(1.1); }
.viewer-bar button:disabled { opacity: 0.45; cursor: default; }
```

- [ ] **Step 5: Make the editor fill the remaining height**

In `app/src/lib/CodeViewer.svelte`, the scoped style currently is:

```svelte
<style>
  .codehost { height: 100%; }
  :global(.codehost .cm-editor) { height: 100%; }
</style>
```

Change the `.codehost` rule so it grows inside the new flex column:

```svelte
<style>
  .codehost { flex: 1 1 auto; min-height: 0; }
  :global(.codehost .cm-editor) { height: 100%; }
</style>
```

- [ ] **Step 6: Type-check**

Run:
```
cd app && npm run check
```
Expected: 0 errors, 0 warnings.

- [ ] **Step 7: Manual verification**

In `npx tauri dev`:
- Before loading any file: the three buttons are visible but **disabled** (greyed).
- After loading/validating an XML and selecting a result: buttons enable.
- `Search` → opens the same find panel as `Ctrl+F`.
- `Collapse all` → all foldable XML blocks collapse; `Expand all` → all expand.
- The editor still fills the viewer area below the bar (no clipping, scrollbar works).

- [ ] **Step 8: Commit**

```
git add app/src/lib/stores.ts app/src/lib/CodeViewer.svelte app/src/App.svelte app/src/app.css
git commit -m "feat(app): viewer bar with Search, Collapse all, Expand all"
```

---

## Self-Review

**Spec coverage:**
- Viewer button bar (Search / Collapse all / Expand all), disabled when no doc → Task 3.
- Search: `@codemirror/search`, `search({ top: true })`, `highlightSelectionMatches()`, `searchKeymap`, Ctrl+F + button, `EditorState.readOnly.of(true)` hides replace → Tasks 1 & 3.
- Folding: `codeFolding()`, `foldGutter()`, `foldKeymap`, `lang-xml` foldNodeProp, `foldAll`/`unfoldAll` buttons → Tasks 2 & 3.
- Store-based wiring like `jumpToLine` → Task 3 stores.
- Coexistence with error/active-line/jump features → regression checks in Tasks 2 & 3 manual steps.
- Verification `npm run check` green, `cargo test` unaffected (no backend change) → every task.
- YAGNI exclusions (cross-file search, replace, custom search bar, fold persistence) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every code step shows exact code. ✓

**Type consistency:** Store names `openViewerSearch` / `foldAllInViewer` / `unfoldAllInViewer` are identical in stores.ts (Task 3 Step 1), CodeViewer import + setters (Step 2), and App.svelte usage (Step 3). CodeMirror command names `openSearchPanel`, `foldAll`, `unfoldAll`, `searchKeymap`, `foldKeymap` match their import sources. The new store type `Writable<() => void>` matches the `() => void` setters. ✓
