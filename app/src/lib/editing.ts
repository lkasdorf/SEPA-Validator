import { writable, get } from "svelte/store";
import { validateEdited, saveEdited } from "./api";
import { drafts, setDraftResult } from "./drafts";
import { results, selectedIndex, selectedResult } from "./stores";

/** What the XML viewer exposes to the edit actions (registered on mount). */
export interface EditorHooks {
  /** Path of the file whose text the editor shows, or "" while none/loading. */
  path(): string;
  text(): string;
  /** Replace the text as one undoable change (keeps the cursor where possible). */
  setText(text: string): void;
  /** Put the file's text as loaded back, as one undoable change. */
  revert(): void;
}

let editor: EditorHooks | null = null;
export function registerEditor(hooks: EditorHooks | null): void {
  editor = hooks;
}

/** An edit action is running (validate or save). */
export const editBusy = writable(false);
/** Short feedback next to the edit buttons. */
export const editNote = writable<{ text: string; failed: boolean } | null>(null);

let noteTimer: ReturnType<typeof setTimeout> | undefined;
function note(text: string, failed = false) {
  editNote.set({ text, failed });
  clearTimeout(noteTimer);
  noteTimer = setTimeout(() => editNote.set(null), failed ? 8000 : 3000);
}

/** The selected file's path when the editor shows it with unsaved edits. */
function editedPath(): string | null {
  const path = get(selectedResult)?.path;
  if (!editor || !path || editor.path() !== path || !get(drafts)[path]) return null;
  return path;
}

/** Validate the edited text without saving. A well-formed text is re-indented
 *  first, exactly as the validator formats it, so the log's lines match. */
export async function validateDraft(): Promise<void> {
  const path = editedPath();
  if (!path || !editor || get(editBusy)) return;
  const text = editor.text();
  editBusy.set(true);
  try {
    const { formatted, result } = await validateEdited(path, text);
    // The user may have switched files or typed on while it ran.
    if (editor.path() !== path || editor.text() !== text) {
      note("Text changed while validating; validate again.", true);
      return;
    }
    if (formatted !== null && formatted !== text) editor.setText(formatted);
    if (!get(drafts)[path]) {
      note("No changes left: the text equals the file.");
      return;
    }
    setDraftResult(path, result);
  } catch (e) {
    note(String(e), true);
  } finally {
    editBusy.set(false);
  }
}

/** Save the edited text as `NAME_editN.xml` next to the original, then list,
 *  validate and select the new file. The original's draft is discarded. */
export async function saveDraft(): Promise<void> {
  const path = editedPath();
  if (!path || !editor || get(editBusy)) return;
  const text = editor.text();
  editBusy.set(true);
  try {
    const saved = await saveEdited(path, text);
    discardEditsOf(path);
    const list = get(results);
    const at = list.findIndex((r) => r.path === path);
    const index = at < 0 ? list.length : at + 1;
    results.update((r) => [...r.slice(0, index), saved, ...r.slice(index)]);
    selectedIndex.set(index);
    note(`Saved ${saved.file}`);
  } catch (e) {
    note(String(e), true);
  } finally {
    editBusy.set(false);
  }
}

/** Put the file's text back (one undoable change, so Ctrl+Z restores the edits). */
export function discardDraft(): void {
  const path = editedPath();
  if (path && !get(editBusy)) discardEditsOf(path);
}

function discardEditsOf(path: string) {
  if (editor?.path() === path) editor.revert();
}
