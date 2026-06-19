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

export interface PmtInfSummary {
  nbOfTxs: string | null;
  ctrlSum: string | null;
  svcLvlCd: string | null;
  reqdDate: string | null;
}

export interface PaymentSummary {
  messageType: string;
  pmtInfCount: number;
  blocks: PmtInfSummary[];
  ustrd: string[];
}
