// Plain-language names for camt statement codes shown in the Overview.

const KINDS: Record<string, string> = {
  Stmt: "Statement",
  Rpt: "Account report",
  Ntfctn: "Notification",
};

const BALANCES: Record<string, string> = {
  OPBD: "Opening booked",
  PRCD: "Previously closed booked",
  CLBD: "Closing booked",
  CLAV: "Closing available",
  FWAV: "Forward available",
  ITBD: "Interim booked",
  ITAV: "Interim available",
  OPAV: "Opening available",
  INFO: "Information",
};

export function kindLabel(kind: string): string {
  return KINDS[kind] ?? kind;
}

export function balanceLabel(code: string | null): string {
  return code ? (BALANCES[code] ?? code) : "";
}
