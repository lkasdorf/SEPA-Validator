import type { RemittanceEntry } from "./types";

const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;

/** CSV of the Transactions table for Excel in a German locale: `;`-separated,
 *  decimal comma in amounts, UTF-8 BOM so umlauts in names survive. Mandate
 *  columns only for direct debits. */
export function transactionsCsv(transactions: RemittanceEntry[], directDebit: boolean): string {
  const header = ["#", "Name", "IBAN", "Amount", "Currency", "Remittance info"];
  if (directDebit) header.push("Mandate ID", "Signature date");
  let out = "﻿" + header.join(";") + "\n";
  transactions.forEach((t, i) => {
    const cells: (string | number)[] = [
      i + 1,
      esc(t.name ?? ""),
      esc(t.iban ?? ""),
      esc((t.amount ?? "").replace(".", ",")),
      esc(t.currency ?? ""),
      esc((t.ustrd ?? "").replace(/\r?\n/g, " ")),
    ];
    if (directDebit) cells.push(esc(t.mandateId ?? ""), esc(t.signatureDate ?? ""));
    out += cells.join(";") + "\n";
  });
  return out;
}
