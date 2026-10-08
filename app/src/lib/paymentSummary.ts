import { writable, get } from "svelte/store";
import { readPaymentSummary } from "./api";
import { createLatest } from "./latest";
import type { PaymentSummary } from "./types";

export type SummaryState = "idle" | "loading" | "ready" | "error";

export const paymentSummary = writable<{
  path: string;
  runId: number;
  state: SummaryState;
  data: PaymentSummary | null;
}>({ path: "", runId: 0, state: "idle", data: null });

const latest = createLatest();

/** Load the summary for `path` into the store, deduped by path within one
 *  validation run (a new run re-reads, since the file may have changed). */
export async function loadPaymentSummary(path: string | undefined, runId: number): Promise<void> {
  if (!path) {
    latest.begin(); // drop any request still in flight
    paymentSummary.set({ path: "", runId, state: "idle", data: null });
    return;
  }
  const cur = get(paymentSummary);
  if (cur.path === path && cur.runId === runId && (cur.state === "ready" || cur.state === "loading")) return;
  const token = latest.begin();
  paymentSummary.set({ path, runId, state: "loading", data: null });
  try {
    const data = await readPaymentSummary(path);
    if (latest.isCurrent(token)) paymentSummary.set({ path, runId, state: "ready", data });
  } catch {
    if (latest.isCurrent(token)) paymentSummary.set({ path, runId, state: "error", data: null });
  }
}
