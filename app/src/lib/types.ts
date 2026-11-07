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
  /** Statement entries (camt) only. */
  bookingDate?: string | null;
  /** Statement entries (camt) only: "CRDT" or "DBIT". */
  creditDebit?: string | null;
  /** Statement entries (camt) only: ISO bank transaction code, e.g. "PMNT-RCDT-ESCT". */
  isoTxCode?: string | null;
  /** Statement entries (camt) only: DK business transaction code (GVC), e.g. "166". */
  gvc?: string | null;
  /** Name of the GVC from the imported DK mapping table, e.g. "SEPA Credit Transfer (Einzelbuchung-Haben)". */
  gvcText?: string | null;
  /** Statement entries (camt) only: proprietary code as given, e.g. "NTRF+166". */
  prtryTxCode?: string | null;
  /** Statement entries (camt) only: further labelled information. */
  details?: { label: string; value: string }[];
}

/** Amount as shown: debits of a statement get a minus sign. */
export function signedAmount(t: RemittanceEntry): string | null {
  if (t.amount == null) return null;
  return t.creditDebit === "DBIT" ? `-${t.amount}` : t.amount;
}

/** One balance of a statement; `amount` is signed (negative for DBIT). */
export interface BalanceSummary {
  code: string | null;
  amount: string | null;
  currency: string | null;
  date: string | null;
}

/** One camt.052 report, camt.053 statement or camt.054 notification block. */
export interface StatementSummary {
  kind: string;
  id: string | null;
  electronicSeqNb: string | null;
  legalSeqNb: string | null;
  from: string | null;
  to: string | null;
  iban: string | null;
  currency: string | null;
  owner: string | null;
  bic: string | null;
  balances: BalanceSummary[];
  entryCount: number;
  creditCount: number;
  creditSum: string;
  debitCount: number;
  debitSum: string;
}

/** camt.052/053/054: the file holds account entries, not payment orders. */
export function isStatement(s: PaymentSummary): boolean {
  return (s.statements?.length ?? 0) > 0 || s.messageType.startsWith("camt.05");
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
  /** camt statement blocks; empty for payment files. */
  statements?: StatementSummary[];
}

export interface SchemaInfo {
  namespace: string;
  filename: string;
  present: boolean;
}

/** The imported DK GVC mapping table. */
export interface GvcInfo {
  source: string;
  codes: number;
}

export interface ImportResult {
  imported: number;
  skipped: string[];
  /** Number of GVCs if a DK mapping table was imported. */
  gvcCodes?: number | null;
}
