import { writable, get } from "svelte/store";
import { readPaymentSummary } from "./api";
import type { PaymentSummary } from "./types";

export type SummaryState = "idle" | "loading" | "ready" | "error";

export const paymentSummary = writable<{
  path: string;
  state: SummaryState;
  data: PaymentSummary | null;
}>({ path: "", state: "idle", data: null });

/** Load the summary for `path` into the store, deduped by path. */
export async function loadPaymentSummary(path: string | undefined): Promise<void> {
  if (!path) {
    paymentSummary.set({ path: "", state: "idle", data: null });
    return;
  }
  const cur = get(paymentSummary);
  if (cur.path === path && (cur.state === "ready" || cur.state === "loading")) return;
  paymentSummary.set({ path, state: "loading", data: null });
  try {
    const data = await readPaymentSummary(path);
    paymentSummary.set({ path, state: "ready", data });
  } catch {
    paymentSummary.set({ path, state: "error", data: null });
  }
}
