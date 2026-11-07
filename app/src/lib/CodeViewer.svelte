<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState, StateEffect, StateField, RangeSetBuilder, type Extension, type Text } from "@codemirror/state";
  import { EditorView, lineNumbers, keymap, Decoration, type DecorationSet } from "@codemirror/view";
  import { xml } from "@codemirror/lang-xml";
  import { oneDark } from "@codemirror/theme-one-dark";
  import { search, searchKeymap, highlightSelectionMatches, openSearchPanel } from "@codemirror/search";
  import { codeFolding, foldGutter, foldKeymap, foldAll, unfoldAll } from "@codemirror/language";
  import { history, historyKeymap, defaultKeymap } from "@codemirror/commands";
  import { selectedResult, runId, jumpToLine, openViewerSearch, foldAllInViewer, unfoldAllInViewer, viewerLarge } from "./stores";
  import { readFormatted } from "./api";
  import { createLatest } from "./latest";
  import { errorLinesOf, minimalChange } from "./viewer";
  import { drafts, editorStates, setDirty, shownResult } from "./drafts";
  import { registerEditor, saveDraft } from "./editing";

  let host: HTMLDivElement;
  let view: EditorView | null = null;
  // A file is identified per validation run: re-validating the same path reloads it.
  let requestedKey = ""; // file the viewer should show
  let shownKey = ""; // file whose text is in the editor right now
  let decoratedFor = ""; // key + error lines last decorated (avoid redundant re-dispatches)
  let shownPath = ""; // path of the file in the editor ("" while none)
  let original: Text | null = null; // its text as loaded: edits are compared against it
  const latest = createLatest();

  const HEAVY_LIMIT = 10 * 1024 * 1024;
  const HEAVY = [xml(), foldGutter(), codeFolding(), highlightSelectionMatches()];

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

  // Strong highlight for the single line the user clicked (jumped to).
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

  const BASE: Extension[] = [lineNumbers(), oneDark,
    search({ top: true }),
    history(),
    keymap.of([
      { key: "Mod-s", preventDefault: true, run: () => { void saveDraft(); return true; } },
      ...searchKeymap, ...foldKeymap, ...historyKeymap, ...defaultKeymap,
    ]),
    errorField, activeLineField,
    // Editing marks the file as a draft until its text equals the loaded one again.
    EditorView.updateListener.of((u) => {
      if (u.docChanged && shownPath && original) setDirty(shownPath, !u.state.doc.eq(original));
    }),
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
    })];

  /** A fresh state per file. Large files skip highlighting and folding. */
  function stateFor(text: string): EditorState {
    const large = text.length > HEAVY_LIMIT;
    return EditorState.create({ doc: text, extensions: large ? BASE : [...BASE, ...HEAVY] });
  }

  onMount(() => {
    view = new EditorView({ parent: host, state: stateFor("") });
    registerEditor({
      path: () => (shownKey === requestedKey ? shownPath : ""),
      text: () => view?.state.doc.toString() ?? "",
      setText: (text) => replaceText(text),
      revert: () => { if (original) replaceText(original.toString()); },
    });
    jumpToLine.set(jumpTo);
    openViewerSearch.set(() => { if (view) openSearchPanel(view); });
    foldAllInViewer.set(() => { if (view) foldAll(view); });
    unfoldAllInViewer.set(() => { if (view) unfoldAll(view); });
    return () => { registerEditor(null); view?.destroy(); };
  });

  // Load file content when the selection (or the validation run) changes.
  // Error lines come from the edited text's result once it was validated.
  $: void loadFor($selectedResult?.path, $runId, errorLinesOf($shownResult?.messages ?? []));

  // Replace the whole state rather than dispatching a document change: while
  // the viewer is hidden (another tab is open), CodeMirror cannot measure, and
  // after a file tall enough for its height scaling (~7M px) replacing the text
  // again throws in the scroll-anchor lookup and leaves the view broken. A new
  // state also resets folds, search and decorations, which belong to the old file.
  // A draft's state is kept when another file is shown and restored when it
  // comes back, with its undo history.
  function show(text: string, key: string, path: string) {
    if (!view) return;
    if (shownPath && original && $drafts[shownPath]) {
      editorStates.set(shownPath, { state: view.state, original });
    }
    const kept = editorStates.get(path);
    if (kept && $drafts[path]) {
      view.setState(kept.state);
      original = kept.original;
    } else {
      view.setState(stateFor(text));
      original = view.state.doc;
    }
    editorStates.delete(path);
    viewerLarge.set(view.state.doc.length > HEAVY_LIMIT);
    shownKey = key;
    shownPath = path;
    decoratedFor = "";
  }

  /** Replace the text as one undoable change, touching only what differs.
   *  Only called from the edit actions, which need the viewer visible. */
  function replaceText(text: string) {
    if (!view) return;
    const change = minimalChange(view.state.doc.toString(), text);
    if (change.from === change.to && change.insert === "") return;
    view.dispatch({ changes: change });
  }

  async function loadFor(path: string | undefined, run: number, errorLines: number[]) {
    if (!view) return;
    if (!path) {
      latest.begin(); // drop any load still in flight
      requestedKey = "";
      if (shownKey !== "") show("", "", "");
      return;
    }
    const key = `${run}|${path}`;
    if (key !== requestedKey) {
      requestedKey = key;
      const token = latest.begin();
      let text = "";
      if (!(editorStates.has(path) && $drafts[path])) { // a draft needs no reload
        try { text = await readFormatted(path); } catch { text = "(could not read file)"; }
      }
      if (!latest.isCurrent(token)) return; // a newer selection won; don't overwrite it
      show(text, key, path);
    }
    const deco = `${key}|${errorLines.join(",")}`;
    if (shownKey === key && decoratedFor !== deco) {
      decoratedFor = deco;
      view.dispatch({ effects: [setErrorLines.of(errorLines), setActiveLine.of(null)] });
    }
  }

  /** Scroll to and flash a 1-based line (called from LogPanel via the jumpToLine store). */
  function jumpTo(line: number) {
    if (shownKey !== requestedKey) return; // still loading: the text belongs to another file
    if (!view || line < 1 || line > view.state.doc.lines) return;
    const pos = view.state.doc.line(line).from;
    view.dispatch({
      effects: [setActiveLine.of(line), EditorView.scrollIntoView(pos, { y: "center" })],
    });
  }
</script>

<div class="codehost" class:loading={shownKey !== requestedKey} bind:this={host}></div>

<style>
  .codehost { flex: 1 1 auto; min-height: 0; transition: opacity .12s ease; }
  .codehost.loading { opacity: .45; cursor: progress; }
  :global(.codehost .cm-editor) { height: 100%; }
</style>
