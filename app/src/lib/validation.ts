import { startValidation, cancelValidation as cancelValidationApi } from "./api";
import { results, selectedIndex, progress, runId } from "./stores";
import type { ValidationEvent } from "./types";

let currentRun = 0;

/** Validate `paths`, streaming results into the stores. Starting a new run
 *  supersedes the previous one: its late events are ignored here, and the
 *  backend stops it before its next file. */
export async function runValidation(paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  const run = ++currentRun;
  runId.set(run);
  results.set([]);
  selectedIndex.set(-1);
  progress.set({ done: 0, total: 0, running: true, cancelled: false });
  try {
    await startValidation(paths, (ev: ValidationEvent) => {
      if (run === currentRun) handle(ev);
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

function handle(ev: ValidationEvent) {
  if (ev.event === "started") {
    progress.update((p) => ({ ...p, done: 0, total: ev.data.total }));
  } else if (ev.event === "result") {
    results.update((r) => { r.push(ev.data.result); return r; });
    progress.update((p) => ({ ...p, done: p.done + 1 }));
    if (ev.data.index === 0) selectedIndex.set(0);
  } else if (ev.event === "finished") {
    progress.update((p) => ({ ...p, running: false, cancelled: ev.data.cancelled }));
  }
}
