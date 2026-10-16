import { describe, expect, it } from "vitest";
import { formatSums, sumAmounts } from "./amounts";
import { sortIndices } from "./transactionSort";
import { filterIndices, searchIndex } from "./transactionFilter";
import type { RemittanceEntry } from "./types";

const tx = (o: Partial<RemittanceEntry>): RemittanceEntry => ({
  instrId: null, endToEndId: null, ustrd: null, name: null, iban: null,
  amount: null, currency: null, mandateId: null, signatureDate: null, ...o,
});
const all = (rows: unknown[]) => rows.map((_, i) => i);

describe("sumAmounts", () => {
  it("sums exactly without float rounding", () => {
    const rows = [tx({ amount: "0.10", currency: "EUR" }), tx({ amount: "0.20", currency: "EUR" })];
    expect(sumAmounts(rows, all(rows))).toEqual({ sums: [{ currency: "EUR", value: "0.30" }], skipped: 0 });
  });

  it("handles mixed fraction digits, large values and integers", () => {
    const rows = [
      tx({ amount: "999999999.99", currency: "EUR" }),
      tx({ amount: "0.001", currency: "EUR" }),
      tx({ amount: "5", currency: "EUR" }),
    ];
    expect(sumAmounts(rows, all(rows)).sums).toEqual([{ currency: "EUR", value: "1000000004.991" }]);
  });

  it("keeps currencies apart and counts missing or malformed amounts", () => {
    const rows = [
      tx({ amount: "1.50", currency: "EUR" }),
      tx({ amount: "2.00", currency: "CHF" }),
      tx({ amount: null }),
      tx({ amount: "1,50", currency: "EUR" }),
      tx({ amount: "-1.00", currency: "EUR" }),
    ];
    const r = sumAmounts(rows, all(rows));
    expect(r.sums).toEqual([{ currency: "CHF", value: "2.00" }, { currency: "EUR", value: "1.50" }]);
    expect(r.skipped).toBe(3);
    expect(formatSums(r.sums)).toBe("2.00 CHF · 1.50 EUR");
  });

  it("sums only the given rows; nothing gives an empty result", () => {
    const rows = [tx({ amount: "1.00", currency: "EUR" }), tx({ amount: "2.00", currency: "EUR" })];
    expect(sumAmounts(rows, [1]).sums).toEqual([{ currency: "EUR", value: "2.00" }]);
    expect(sumAmounts(rows, []).sums).toEqual([]);
    expect(formatSums([])).toBe("—");
  });
});

describe("sortIndices", () => {
  const rows = [
    tx({ name: "bernd", amount: "10.00", signatureDate: "2024-02-01" }),
    tx({ name: "Anna", amount: "9.50", signatureDate: null }),
    tx({ name: null, amount: "100.00", signatureDate: "2023-12-31" }),
    tx({ name: "Ärger", amount: null, signatureDate: "2024-02-01" }),
  ];

  it("index sorts by file order", () => {
    expect(sortIndices(rows, "index", "asc")).toEqual([0, 1, 2, 3]);
    expect(sortIndices(rows, "index", "desc")).toEqual([3, 2, 1, 0]);
  });

  it("text is case- and accent-insensitive, empty values last in both directions", () => {
    expect(sortIndices(rows, "name", "asc")).toEqual([1, 3, 0, 2]);
    expect(sortIndices(rows, "name", "desc")).toEqual([0, 3, 1, 2]);
  });

  it("amounts compare numerically, not as text", () => {
    expect(sortIndices(rows, "amount", "asc")).toEqual([1, 0, 2, 3]);
    expect(sortIndices(rows, "amount", "desc")).toEqual([2, 0, 1, 3]);
  });

  it("ties keep file order", () => {
    expect(sortIndices(rows, "signatureDate", "asc")).toEqual([2, 0, 3, 1]);
    expect(sortIndices(rows, "signatureDate", "desc")).toEqual([0, 3, 2, 1]);
  });

  it("filtering keeps the sort order", () => {
    const r = [tx({ name: "Zoe Beitrag" }), tx({ name: "Max" }), tx({ name: "Anna Beitrag" })];
    expect(filterIndices(searchIndex(r), "beitrag", sortIndices(r, "name", "asc"))).toEqual([2, 0]);
  });
});
