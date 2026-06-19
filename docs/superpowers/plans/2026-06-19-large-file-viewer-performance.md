# Lean Viewer Mode for Large Files Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the XML viewer from being sluggish on large (~60 MB) files by disabling the heavy CodeMirror extensions (syntax highlighting, folding, selection-match highlighting) above a size threshold.

**Architecture:** The four heavy extensions move into a CodeMirror `Compartment`; on file load, `CodeViewer` reconfigures it to empty when the formatted text exceeds 10 MB and sets a `viewerLarge` store. `App` hides the Collapse/Expand buttons and shows a notice in large mode. Line numbers, error/active-line highlighting, click-to-line, and search stay in both modes. No backend change.

**Tech Stack:** Svelte 5 + TypeScript + Vite, CodeMirror 6 (`Compartment` from `@codemirror/state`).

## Global Constraints

- Threshold: `HEAVY_LIMIT = 10 * 1024 * 1024`; "large" when the loaded formatted text length (`text.length`) exceeds it.
- Heavy extensions placed in a `Compartment` and disabled in large mode: `xml()`, `codeFolding()`, `foldGutter()`, `highlightSelectionMatches()`.
- Kept in both modes: `lineNumbers()`, the error-line + active-line decorations, click-to-line (`jumpTo`), `search({ top: true })` + `searchKeymap`, `EditorView.editable.of(false)`, `EditorState.readOnly.of(true)`, the theme.
- New store `viewerLarge` (`writable<boolean>`, default false), set by `CodeViewer` on load.
- In `App.svelte`'s XML tab: **Search** always shown; **Collapse all** / **Expand all** only when `!$viewerLarge`; a notice "Große Datei: Syntax-Highlighting & Falten deaktiviert (Performance)" when `$viewerLarge`.
- No backend / validation change. No JS test runner by design; verification is `cd app && npm run check` (0 errors/0 warnings).
- Commit format: `type(scope): summary`.

---

### Task 1: Lean viewer mode (compartment + store + UI)

**Files:**
- Modify: `app/src/lib/stores.ts`
- Modify: `app/src/lib/CodeViewer.svelte`
- Modify: `app/src/App.svelte`
- Modify: `app/src/app.css`

**Interfaces:**
- Produces: store `viewerLarge: Writable<boolean>`.
- Consumes: existing `readFormatted`, `selectedResult`, the existing viewer stores; CodeMirror `Compartment`.

- [ ] **Step 1: Add the `viewerLarge` store**

In `app/src/lib/stores.ts`, append:

```ts
export const viewerLarge = writable<boolean>(false);
```

- [ ] **Step 2: Import `Compartment` and `viewerLarge` in CodeViewer**

In `app/src/lib/CodeViewer.svelte`, change the `@codemirror/state` import:

```ts
  import { EditorState, StateEffect, StateField, RangeSetBuilder } from "@codemirror/state";
```

to:

```ts
  import { EditorState, StateEffect, StateField, RangeSetBuilder, Compartment } from "@codemirror/state";
```

And change the stores import:

```ts
  import { selectedResult, jumpToLine, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./stores";
```

to:

```ts
  import { selectedResult, jumpToLine, openViewerSearch, foldAllInViewer, unfoldAllInViewer, viewerLarge } from "./stores";
```

- [ ] **Step 3: Define the threshold, compartment, and heavy extension set**

In `app/src/lib/CodeViewer.svelte`, find:

```ts
  let view: EditorView | null = null;
  let currentPath = "";
```

and insert directly after it:

```ts
  const HEAVY_LIMIT = 10 * 1024 * 1024;
  const heavyComp = new Compartment();
  const HEAVY = [xml(), foldGutter(), codeFolding(), highlightSelectionMatches()];
```

- [ ] **Step 4: Put the heavy extensions in the compartment**

In the `EditorState.create` extensions array, replace:

```ts
        extensions: [lineNumbers(), foldGutter(), xml(), oneDark,
          codeFolding(),
          highlightSelectionMatches(),
          search({ top: true }),
```

with:

```ts
        extensions: [lineNumbers(), heavyComp.of(HEAVY), oneDark,
          search({ top: true }),
```

(The rest of the array — `keymap.of([...searchKeymap, ...foldKeymap])`, `errorField`, `activeLineField`, `EditorView.editable.of(false)`, `EditorState.readOnly.of(true)`, `EditorView.theme({ ... })` — is unchanged.)

- [ ] **Step 5: Decide large/small on load and reconfigure**

In `loadFor`, replace:

```ts
    if (path !== currentPath) {
      currentPath = path;
      let text = "";
      try { text = await readFormatted(path); } catch { text = "(could not read file)"; }
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
```

with:

```ts
    if (path !== currentPath) {
      currentPath = path;
      let text = "";
      try { text = await readFormatted(path); } catch { text = "(could not read file)"; }
      const large = text.length > HEAVY_LIMIT;
      view.dispatch({
        changes: { from: 0, to: view.state.doc.length, insert: text },
        effects: heavyComp.reconfigure(large ? [] : HEAVY),
      });
      viewerLarge.set(large);
    }
```

- [ ] **Step 6: Hide fold buttons + show a notice in App.svelte**

In `app/src/App.svelte`, add `viewerLarge` to the stores import:

```ts
  import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer, schemaDialogOpen } from "./lib/stores";
```

becomes:

```ts
  import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer, schemaDialogOpen, viewerLarge } from "./lib/stores";
```

Then replace the XML-tab button block:

```svelte
        {#if viewerTab === "xml"}
          <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
          <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
          <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
        {/if}
```

with:

```svelte
        {#if viewerTab === "xml"}
          <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
          {#if !$viewerLarge}
            <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
            <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
          {:else}
            <span class="viewer-note">Große Datei: Syntax-Highlighting &amp; Falten deaktiviert (Performance)</span>
          {/if}
        {/if}
```

- [ ] **Step 7: Style the notice**

In `app/src/app.css`, find:

```css
.viewer-bar button:disabled { opacity: 0.45; cursor: default; }
```

and insert directly after it:

```css
.viewer-bar .viewer-note { font-size: 11px; opacity: 0.7; align-self: center; }
```

- [ ] **Step 8: Type-check**

Run: `cd app && npm run check`
Expected: 0 errors, 0 warnings.

- [ ] **Step 9: Commit**

```bash
git add app/src/lib/stores.ts app/src/lib/CodeViewer.svelte app/src/App.svelte app/src/app.css
git commit -m "perf(app): disable heavy CodeMirror extensions for large files"
```

---

## Self-Review

**Spec coverage:**
- Threshold 10 MB on formatted text length → Step 5 (`text.length > HEAVY_LIMIT`).
- Heavy extensions (`xml`/`codeFolding`/`foldGutter`/`highlightSelectionMatches`) in a compartment, disabled when large → Steps 3-5.
- Kept features (line numbers, error/active-line, click-to-line, search, read-only, theme) → unchanged array remainder (Step 4 note).
- `viewerLarge` store set on load → Steps 1, 5.
- Search always; Collapse/Expand only when not large; notice when large → Step 6.
- No backend change → only frontend files touched.
- YAGNI exclusions (raw mode, configurable threshold, custom viewer) → not implemented. ✓

**Placeholder scan:** No TBD/TODO; every step shows exact code. ✓

**Type consistency:** `viewerLarge` declared `writable<boolean>` in stores.ts (Step 1), imported in CodeViewer (Step 2) and App (Step 6), set via `viewerLarge.set(large)` (Step 5) and read as `$viewerLarge` (Step 6). `heavyComp`/`HEAVY`/`HEAVY_LIMIT` defined in Step 3 and used in Steps 4-5. `Compartment.reconfigure` and `.of` are the CodeMirror 6 API. ✓
