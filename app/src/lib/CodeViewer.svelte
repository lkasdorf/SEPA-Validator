<script lang="ts">
  import { onMount } from "svelte";
  import { EditorState, StateEffect, StateField, RangeSetBuilder } from "@codemirror/state";
  import { EditorView, lineNumbers, keymap, Decoration, type DecorationSet } from "@codemirror/view";
  import { xml } from "@codemirror/lang-xml";
  import { oneDark } from "@codemirror/theme-one-dark";
  import { search, searchKeymap, highlightSelectionMatches, openSearchPanel } from "@codemirror/search";
  import { selectedResult, jumpToLine } from "./stores";
  import { readFormatted } from "./api";

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

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: "",
        extensions: [lineNumbers(), xml(), oneDark,
          highlightSelectionMatches(),
          search({ top: true }),
          keymap.of(searchKeymap),
          errorField, activeLineField,
          EditorView.editable.of(false),
          EditorState.readOnly.of(true),
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
          })],
      }),
    });
    jumpToLine.set(jumpTo);
    return () => view?.destroy();
  });

  // Load file content when selection changes.
  $: void loadFor($selectedResult?.path, $selectedResult?.messages.map((m) => m.line ?? 0).filter((l) => l > 0));

  async function loadFor(path: string | undefined, errorLines: number[] | undefined) {
    if (!view || !path) return;
    if (path !== currentPath) {
      currentPath = path;
      let text = "";
      try { text = await readFormatted(path); } catch { text = "(could not read file)"; }
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text } });
    }
    view.dispatch({ effects: [setErrorLines.of(errorLines ?? []), setActiveLine.of(null)] });
  }

  /** Scroll to and flash a 1-based line (called from LogPanel via the jumpToLine store). */
  function jumpTo(line: number) {
    if (!view || line < 1 || line > view.state.doc.lines) return;
    const pos = view.state.doc.line(line).from;
    view.dispatch({
      effects: [setActiveLine.of(line), EditorView.scrollIntoView(pos, { y: "center" })],
    });
  }
</script>

<div class="codehost" bind:this={host}></div>

<style>
  .codehost { height: 100%; }
  :global(.codehost .cm-editor) { height: 100%; }
</style>
