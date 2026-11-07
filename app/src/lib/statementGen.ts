import type { StatementPreview } from "./types";

/** True for credit transfer (pain.001) and direct debit (pain.008) files, the
 *  ones a bank statement can be generated from. Matches ISO and Swiss namespaces. */
export function isPaymentFile(namespace: string): boolean {
  return /pain\.00[18]\./.test(namespace);
}

/** The opening balance as typed, as `-?digits.dd`, or null if it isn't one.
 *  Commas are only accepted as thousands separators, so a German `12,50` or
 *  `10.000,00` is refused rather than misread. Mirrors `parse_opening` in Rust. */
export function normalizeOpening(input: string): string | null {
  const s = input.replace(/\s+/g, "");
  const m = /^(-?)(\d{1,3}(?:,\d{3})+|\d+)(?:\.(\d{1,2}))?$/.exec(s);
  if (!m) return null;
  const int = m[2].replace(/,/g, "").replace(/^0+(?=\d)/, "");
  return `${m[1]}${int}.${(m[3] ?? "").padEnd(2, "0")}`;
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** One line for the dialog: what Generate would write. */
export function previewLine(p: StatementPreview): string {
  const days = plural(p.days, "booking day", "booking days");
  const txs = plural(p.transactions, "transaction", "transactions");
  if (p.accounts > 1) {
    return `${plural(p.statements, "statement", "statements")} for ${p.accounts} accounts over ${days}, ${txs}`;
  }
  return `${days}, ${txs}`;
}
