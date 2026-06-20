export type Severity = "error" | "warning";
export type StatusKind = "ok" | "invalid" | "warnings" | "no_schema" | "error";

export interface Message {
  severity: Severity;
  text: string;
  line: number | null;
  column: number | null;
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
  | { event: "finished"; data: { total: number } };

/** Human label like the old tool: "INVALID (2 errors, 1 warning)". */
export function statusLabel(r: ValidationResult): string {
  switch (r.status) {
    case "ok": return "OK";
    case "warnings": return `WARNINGS (${r.warnings})`;
    case "no_schema": return "NO SCHEMA";
    case "error": return "ERROR";
    case "invalid": return `INVALID (${r.errors} errors, ${r.warnings} warnings)`;
  }
}

export interface Creditor {
  name: string | null;
  iban: string | null;
  bic: string | null;
  creditorId: string | null;
}

export interface PmtInfSummary {
  nbOfTxs: string | null;
  ctrlSum: string | null;
  svcLvlCd: string | null;
  lclInstrm: string | null;
  seqTp: string | null;
  reqdDate: string | null;
}

export interface RemittanceEntry {
  instrId: string | null;
  endToEndId: string | null;
  ustrd: string | null;
}

export interface PaymentSummary {
  messageType: string;
  pmtInfCount: number;
  creditor: Creditor | null;
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
