import { beforeEach, describe, expect, it, vi } from "vitest";
import { get } from "svelte/store";
import type { PaymentSummary } from "./types";

const pending = new Map<string, (s: PaymentSummary) => void>();
const readPaymentSummary = vi.fn(
  (path: string) => new Promise<PaymentSummary>((resolve) => pending.set(path, resolve)),
);
vi.mock("./api", () => ({ readPaymentSummary: (p: string) => readPaymentSummary(p) }));

const { loadPaymentSummary, paymentSummary } = await import("./paymentSummary");

const summary = (messageType: string): PaymentSummary => ({
  messageType,
  pmtInfCount: 0,
  creditor: null,
  blocks: [],
  transactions: [],
});

describe("loadPaymentSummary", () => {
  beforeEach(() => {
    pending.clear();
    readPaymentSummary.mockClear();
    paymentSummary.set({ path: "", runId: 0, state: "idle", data: null });
  });

  it("keeps the newest file when an older request resolves last", async () => {
    const a = loadPaymentSummary("a.xml", 1);
    const b = loadPaymentSummary("b.xml", 1);
    pending.get("b.xml")!(summary("B"));
    await b;
    pending.get("a.xml")!(summary("A"));
    await a;
    const s = get(paymentSummary);
    expect(s.path).toBe("b.xml");
    expect(s.data?.messageType).toBe("B");
  });

  it("reloads the same file after a new validation run", async () => {
    const first = loadPaymentSummary("a.xml", 1);
    pending.get("a.xml")!(summary("old"));
    await first;
    const second = loadPaymentSummary("a.xml", 2);
    pending.get("a.xml")!(summary("new"));
    await second;
    expect(readPaymentSummary).toHaveBeenCalledTimes(2);
    expect(get(paymentSummary).data?.messageType).toBe("new");
  });

  it("still delivers a load when the same file is requested again while loading", async () => {
    const first = loadPaymentSummary("a.xml", 1);
    await loadPaymentSummary("a.xml", 1);
    pending.get("a.xml")!(summary("A"));
    await first;
    expect(get(paymentSummary)).toMatchObject({ state: "ready", path: "a.xml" });
  });

  it("does not refetch the same file within the same run", async () => {
    const first = loadPaymentSummary("a.xml", 1);
    pending.get("a.xml")!(summary("A"));
    await first;
    await loadPaymentSummary("a.xml", 1);
    expect(readPaymentSummary).toHaveBeenCalledTimes(1);
  });
});
