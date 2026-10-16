import type { RemittanceEntry } from "./types";

export interface CurrencySum {
  currency: string;
  /** Exact decimal, at least 2 fraction digits, e.g. "1234.50". */
  value: string;
}

const AMOUNT = /^(\d+)(?:\.(\d+))?$/;

/** Exact sum of `InstdAmt` per currency (BigInt, no float rounding) over the
 *  rows at `indices`. Missing or malformed amounts are counted, not summed. */
export function sumAmounts(
  rows: RemittanceEntry[],
  indices: Iterable<number>
): { sums: CurrencySum[]; skipped: number } {
  const acc = new Map<string, { units: bigint; scale: number }>();
  let skipped = 0;
  for (const i of indices) {
    const t = rows[i];
    const m = t.amount != null ? AMOUNT.exec(t.amount.trim()) : null;
    if (!m) {
      skipped++;
      continue;
    }
    const frac = m[2] ?? "";
    const ccy = t.currency ?? "";
    const cur = acc.get(ccy) ?? { units: 0n, scale: 0 };
    const scale = Math.max(cur.scale, frac.length);
    const units =
      cur.units * 10n ** BigInt(scale - cur.scale) +
      BigInt(m[1] + frac) * 10n ** BigInt(scale - frac.length);
    acc.set(ccy, { units, scale });
  }
  const sums = [...acc.entries()]
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([currency, { units, scale }]) => ({ currency, value: formatUnits(units, scale) }));
  return { sums, skipped };
}

function formatUnits(units: bigint, scale: number): string {
  const digits = Math.max(2, scale);
  const s = (units * 10n ** BigInt(digits - scale)).toString().padStart(digits + 1, "0");
  return `${s.slice(0, -digits)}.${s.slice(-digits)}`;
}

export function formatSums(sums: CurrencySum[]): string {
  return sums.length ? sums.map((s) => `${s.value}${s.currency ? ` ${s.currency}` : ""}`).join(" · ") : "—";
}
