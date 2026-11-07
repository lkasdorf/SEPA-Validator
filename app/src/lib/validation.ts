import { get } from "svelte/store";
import { open } from "@tauri-apps/plugin-dialog";
import { startValidation, cancelValidation as cancelValidationApi } from "./api";
import { results, selectedIndex, progress, runId } from "./stores";
import { hasDrafts, confirmDiscardDrafts, clearDrafts } from "./drafts";
import type { ValidationEvent } from "./types";

let currentRun = 0;

/** Validate `paths`, streaming results into the stores. Starting a new run
 *  supersedes the previous one: its late events are ignored here, and the
 *  backend stops it before its next file. */
export function runValidation(paths: string[]): Promise<void> {
  return start(paths, false);
}

/** Validate `paths` and add them after the current results (which stay),
 *  selecting the first new one. Used for generated statements. */
export function appendValidation(paths: string[]): Promise<void> {
  return start(paths, true);
}

async function start(paths: string[], append: boolean): Promise<void> {
  if (paths.length === 0) return;
  // A new run replaces the file list, so unsaved edits would be lost.
  // Appending keeps the list and its drafts.
  if (!append && hasDrafts()) {
    if (!(await confirmDiscardDrafts("Validate other files"))) return;
    clearDrafts();
  }
  const run = ++currentRun;
  runId.set(run);
  const base = append ? get(results).length : 0;
  if (!append) {
    results.set([]);
    selectedIndex.set(-1);
  }
  progress.set({ done: 0, total: 0, running: true, cancelled: false });
  try {
    await startValidation(paths, (ev: ValidationEvent) => {
      if (run === currentRun) handle(ev, base);
    });
  } catch (e) {
    console.error("Validation could not be started:", e);
    if (run === currentRun) progress.update((p) => ({ ...p, running: false }));
  }
}

/** Stop the current run. The UI stops right away; the backend finishes the
 *  file it is on and then stops. */
export async function cancelValidation(): Promise<void> {
  currentRun++;
  progress.update((p) => ({ ...p, running: false, cancelled: true }));
  await cancelValidationApi();
}

export async function pickFiles(): Promise<void> {
  const sel = await open({ multiple: true, filters: [{ name: "XML", extensions: ["xml"] }] });
  if (sel) await runValidation(Array.isArray(sel) ? sel : [sel]);
}

export async function pickFolder(): Promise<void> {
  const sel = await open({ directory: true });
  if (sel) await runValidation([sel as string]);
}

function handle(ev: ValidationEvent, base: number) {
  if (ev.event === "started") {
    progress.update((p) => ({ ...p, done: 0, total: ev.data.total }));
  } else if (ev.event === "result") {
    results.update((r) => { r.push(ev.data.result); return r; });
    progress.update((p) => ({ ...p, done: p.done + 1 }));
    if (ev.data.index === 0) selectedIndex.set(base);
  } else if (ev.event === "finished") {
    progress.update((p) => ({ ...p, running: false, cancelled: ev.data.cancelled }));
  }
}
