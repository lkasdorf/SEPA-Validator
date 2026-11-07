import { describe, expect, it } from "vitest";
import { sumAmounts } from "./amounts";
import { sortIndices } from "./transactionSort";
import { transactionsCsv } from "./transactionsCsv";
import { filterIndices, searchIndex } from "./transactionFilter";
import { balanceLabel, kindLabel } from "./statementLabels";
import { isStatement, signedAmount, type PaymentSummary, type RemittanceEntry } from "./types";

const entry = (amount: string, creditDebit: "CRDT" | "DBIT", o: Partial<RemittanceEntry> = {}): RemittanceEntry => ({
  instrId: null, endToEndId: null, ustrd: null, name: null, iban: null,
  amount, currency: "EUR", mandateId: null, signatureDate: null,
  bookingDate: "2026-11-02", creditDebit, ...o,
});

describe("statement entries", () => {
  const rows = [entry("100.00", "CRDT"), entry("30.50", "DBIT"), entry("80.00", "DBIT")];

  it("sum debits negative", () => {
    expect(sumAmounts(rows, [0, 1]).sums).toEqual([{ currency: "EUR", value: "69.50" }]);
    expect(sumAmounts(rows, [0, 1, 2]).sums).toEqual([{ currency: "EUR", value: "-10.50" }]);
  });

  it("show and sort debits as negative amounts", () => {
    expect(signedAmount(rows[1])).toBe("-30.50");
    expect(signedAmount(rows[0])).toBe("100.00");
    expect(sortIndices(rows, "amount", "asc")).toEqual([2, 1, 0]);
  });

  it("export a booking date column and signed amounts", () => {
    const csv = transactionsCsv([entry("30.50", "DBIT", { name: "Shop" })], false);
    const [header, line] = csv.replace("﻿", "").trim().split("\n");
    expect(header).toBe("#;Booking date;Name;IBAN;Amount;Currency;GVC;ISO code;Remittance info");
    expect(line).toBe('1;"2026-11-02";"Shop";"";"-30,50";"EUR";"";"";""');
  });

  it("export and search GVC and ISO transaction codes", () => {
    const t = entry("12.00", "DBIT", { gvc: "105", isoTxCode: "PMNT-RDDT-ESDD" });
    const line = transactionsCsv([t], false).replace("\ufeff", "").trim().split("\n")[1];
    expect(line).toBe('1;"2026-11-02";"";"";"-12,00";"EUR";"105";"PMNT-RDDT-ESDD";""');
    const index = searchIndex([t, entry("1.00", "CRDT", { gvc: "166" })]);
    expect(filterIndices(index, "105")).toEqual([0]);
    expect(filterIndices(index, "esdd")).toEqual([0]);
  });

  it("payment files keep their CSV layout", () => {
    const csv = transactionsCsv([{ ...entry("5.00", "CRDT"), creditDebit: null, bookingDate: null }], false);
    expect(csv.replace("﻿", "").split("\n")[0]).toBe("#;Name;IBAN;Amount;Currency;Remittance info");
  });
});

describe("isStatement", () => {
  const base: PaymentSummary = {
    messageType: "pain.001.001.09", pmtInfCount: 1, creditor: null, debtor: null, blocks: [], transactions: [],
  };
  it("is true for camt files only", () => {
    expect(isStatement(base)).toBe(false);
    expect(isStatement({ ...base, messageType: "camt.053.001.08", statements: [] })).toBe(true);
  });
});

describe("statement labels", () => {
  it("name kinds and balance codes", () => {
    expect(kindLabel("Stmt")).toBe("Statement");
    expect(balanceLabel("CLBD")).toBe("Closing booked");
    expect(balanceLabel("XXXX")).toBe("XXXX");
    expect(balanceLabel(null)).toBe("");
  });
});
