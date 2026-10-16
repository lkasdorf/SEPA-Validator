import type { RemittanceEntry } from "./types";

export type SortKey = "index" | "name" | "iban" | "amount" | "ustrd" | "mandateId" | "signatureDate";
export type SortDir = "asc" | "desc";

const collator = new Intl.Collator("de", { sensitivity: "base", numeric: true });

function value(t: RemittanceEntry, key: Exclude<SortKey, "index">): string | number | null {
  if (key === "amount") {
    const n = t.amount != null ? Number(t.amount) : NaN;
    return Number.isFinite(n) ? n : null;
  }
  const v = t[key];
  return v == null || v === "" ? null : v;
}

/** Row indices in the requested order. Empty values always go last, ties keep
 *  file order. Text compares case-insensitively with numbers in natural order. */
export function sortIndices(rows: RemittanceEntry[], key: SortKey, dir: SortDir): number[] {
  const idx = rows.map((_, i) => i);
  const sign = dir === "asc" ? 1 : -1;
  if (key === "index") return dir === "asc" ? idx : idx.reverse();
  const vals = rows.map((t) => value(t, key));
  return idx.sort((a, b) => {
    const va = vals[a];
    const vb = vals[b];
    if (va == null || vb == null) return va == null && vb == null ? a - b : va == null ? 1 : -1;
    const c = typeof va === "number" && typeof vb === "number" ? va - vb : collator.compare(String(va), String(vb));
    return c !== 0 ? sign * c : a - b;
  });
}
