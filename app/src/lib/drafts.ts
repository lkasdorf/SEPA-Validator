import { writable, derived, get } from "svelte/store";
import { ask } from "@tauri-apps/plugin-dialog";
import type { EditorState, Text } from "@codemirror/state";
import { selectedResult } from "./stores";
import type { ValidationResult } from "./types";

/** Unsaved edits of one file in the XML viewer. */
export interface Draft {
  /** Result of the last Validate of the edited text; null until validated. */
  result: ValidationResult | null;
  /** The text changed since that Validate (its line numbers may be off). */
  stale: boolean;
}

/** Files with unsaved edits, by path. A path is present only while its text
 *  differs from the file as loaded. Kept until saved, discarded or a new run. */
export const drafts = writable<Record<string, Draft>>({});

/** Editor state of drafts whose file is not shown right now (undo history,
 *  cursor and folds included), so switching back restores the edit. */
export const editorStates = new Map<string, { state: EditorState; original: Text }>();

/** The result the log and the error-line highlights show: the edited text's
 *  once it was validated, otherwise the file's from the last run. */
export const shownResult = derived([selectedResult, drafts], ([$sel, $drafts]) =>
  ($sel && $drafts[$sel.path]?.result) || $sel
);

/** The draft of the selected file, if it has unsaved edits. */
export const selectedDraft = derived([selectedResult, drafts], ([$sel, $drafts]) =>
  ($sel ? $drafts[$sel.path] ?? null : null)
);

export function hasDrafts(): boolean {
  return Object.keys(get(drafts)).length > 0;
}

/** Record whether `path`'s text now differs from the file. */
export function setDirty(path: string, dirty: boolean): void {
  const cur = get(drafts)[path];
  if (dirty && (!cur || (cur.result && !cur.stale))) {
    drafts.update((d) => ({ ...d, [path]: { result: cur?.result ?? null, stale: !!cur?.result } }));
  } else if (!dirty && cur) {
    editorStates.delete(path);
    drafts.update((d) => {
      const { [path]: _, ...rest } = d;
      return rest;
    });
  }
}

export function setDraftResult(path: string, result: ValidationResult): void {
  drafts.update((d) => (d[path] ? { ...d, [path]: { result, stale: false } } : d));
}

export function clearDrafts(): void {
  editorStates.clear();
  drafts.set({});
}

/** Ask before unsaved edits are lost; true when there are none or the user agrees. */
export async function confirmDiscardDrafts(action: string): Promise<boolean> {
  const n = Object.keys(get(drafts)).length;
  if (n === 0) return true;
  return ask(
    `${n === 1 ? "1 file has" : `${n} files have`} unsaved edits. ${action} anyway and discard them?`,
    { title: "Unsaved edits", kind: "warning", okLabel: "Discard edits", cancelLabel: "Cancel" }
  );
}
