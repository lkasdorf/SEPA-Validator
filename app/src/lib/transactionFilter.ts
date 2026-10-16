import type { RemittanceEntry } from "./types";

/** Lowercased search text per transaction: name, IBAN, remittance info and
 *  mandate ID. Built once per file so filtering on each keystroke stays cheap. */
export function searchIndex(transactions: RemittanceEntry[]): string[] {
  return transactions.map((t) =>
    [t.name, t.iban, t.ustrd, t.mandateId].filter((v) => v != null).join("\u0000").toLowerCase()
  );
}

/** Indices of the transactions matching every whitespace-separated term of
 *  `query` (case-insensitive substring), in the order of `candidates`
 *  (default: document order). An empty query matches all. Terms can span
 *  fields, e.g. "müller mai". */
export function filterIndices(index: string[], query: string, candidates?: number[]): number[] {
  const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
  const order = candidates ?? index.map((_, i) => i);
  if (!terms.length) return order;
  return order.filter((i) => terms.every((term) => index[i].includes(term)));
}
