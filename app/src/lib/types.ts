export type Severity = "error" | "warning";
export type StatusKind = "ok" | "invalid" | "warnings" | "no_schema" | "error";

export interface Message {
  severity: Severity;
  text: string;
  line: number | null;
  column: number | null;
  /** Plain-language explanation for common mistakes. */
  hint: string | null;
}

export interface ValidationResult {
  file: string;
  path: string;
  namespace: string;
  schema: string;
  status: StatusKind;
  errors: number;
  warnings: number;
  messages: Message[];
}

export type ValidationEvent =
  | { event: "started"; data: { total: number } }
  | { event: "result"; data: { index: number; result: ValidationResult } }
  | { event: "finished"; data: { total: number; cancelled: boolean } };

const count = (n: number, word: string) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** Human label like the old tool: "INVALID (2 errors, 1 warning)". */
export function statusLabel(r: ValidationResult): string {
  switch (r.status) {
    case "ok": return "OK";
    case "warnings": return `WARNINGS (${r.warnings})`;
    case "no_schema": return "NO SCHEMA";
    case "error": return "ERROR";
    case "invalid":
      return `INVALID (${count(r.errors, "error")}${r.warnings ? `, ${count(r.warnings, "warning")}` : ""})`;
  }
}

export interface Creditor {
  name: string | null;
  iban: string | null;
  bic: string | null;
  creditorId: string | null;
}

/** Ordering party of a credit transfer (pain.001): PmtInf-level Dbtr/DbtrAcct/DbtrAgt. */
export interface Debtor {
  name: string | null;
  iban: string | null;
  bic: string | null;
}

export interface PmtInfSummary {
  nbOfTxs: string | null;
  ctrlSum: string | null;
  svcLvlCd: string | null;
  lclInstrm: string | null;
  seqTp: string | null;
  reqdDate: string | null;
}

/** One transaction. `name`/`iban` are the counterparty: Cdtr for pain.001,
 *  Dbtr for pain.008. Mandate fields only exist for direct debits. */
export interface RemittanceEntry {
  instrId: string | null;
  endToEndId: string | null;
  ustrd: string | null;
  name: string | null;
  iban: string | null;
  /** InstdAmt verbatim, e.g. "123.45". */
  amount: string | null;
  currency: string | null;
  mandateId: string | null;
  signatureDate: string | null;
}

/** pain.008 (or any file whose transactions carry a mandate). */
export function isDirectDebit(s: PaymentSummary): boolean {
  return s.messageType.startsWith("pain.008") || s.transactions.some((t) => t.mandateId != null);
}

export interface PaymentSummary {
  messageType: string;
  pmtInfCount: number;
  creditor: Creditor | null;
  debtor: Debtor | null;
  blocks: PmtInfSummary[];
  transactions: RemittanceEntry[];
}

export interface SchemaInfo {
  namespace: string;
  filename: string;
  present: boolean;
}

export interface ImportResult {
  imported: number;
  skipped: string[];
}
