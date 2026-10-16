import { describe, expect, it } from "vitest";
import { transactionsCsv } from "./transactionsCsv";
import { visibleRange } from "./virtualRows";
import { isDirectDebit, type PaymentSummary, type RemittanceEntry } from "./types";

const tx = (o: Partial<RemittanceEntry>): RemittanceEntry => ({
  instrId: null, endToEndId: null, ustrd: null, name: null, iban: null,
  amount: null, currency: null, mandateId: null, signatureDate: null, ...o,
});

describe("transactionsCsv", () => {
  it("credit transfer: no mandate columns, decimal comma, escaped quotes, joined lines", () => {
    const csv = transactionsCsv(
      [tx({ name: 'Müller "GmbH"', iban: "DE89370400440532013000", amount: "1234.50", currency: "EUR", ustrd: "Line 1\nLine 2" }), tx({})],
      false,
    );
    expect(csv.startsWith("﻿")).toBe(true);
    const lines = csv.slice(1).trimEnd().split("\n");
    expect(lines[0]).toBe("#;Name;IBAN;Amount;Currency;Remittance info");
    expect(lines[1]).toBe('1;"Müller ""GmbH""";"DE89370400440532013000";"1234,50";"EUR";"Line 1 Line 2"');
    expect(lines[2]).toBe('2;"";"";"";"";""');
  });

  it("direct debit adds mandate ID and signature date", () => {
    const csv = transactionsCsv([tx({ name: "A", mandateId: "M-1", signatureDate: "2024-03-15" })], true);
    const lines = csv.slice(1).trimEnd().split("\n");
    expect(lines[0]).toBe("#;Name;IBAN;Amount;Currency;Remittance info;Mandate ID;Signature date");
    expect(lines[1]).toBe('1;"A";"";"";"";"";"M-1";"2024-03-15"');
  });
});

describe("isDirectDebit", () => {
  const s = (messageType: string, transactions: RemittanceEntry[] = []): PaymentSummary => ({
    messageType, pmtInfCount: 0, creditor: null, debtor: null, blocks: [], transactions,
  });
  it("by message type or mandate data", () => {
    expect(isDirectDebit(s("pain.008.001.08"))).toBe(true);
    expect(isDirectDebit(s("pain.001.001.09"))).toBe(false);
    expect(isDirectDebit(s("", [tx({ mandateId: "M" })]))).toBe(true);
  });
});

describe("visibleRange", () => {
  it("renders the viewport plus overscan, clamped", () => {
    expect(visibleRange(0, 260, 26, 50_000, 5)).toEqual({ start: 0, end: 15 });
    expect(visibleRange(2600, 260, 26, 50_000, 5)).toEqual({ start: 95, end: 115 });
    expect(visibleRange(1_299_740, 260, 26, 50_000, 5)).toEqual({ start: 49_985, end: 50_000 });
    expect(visibleRange(0, 600, 26, 3)).toEqual({ start: 0, end: 3 });
    expect(visibleRange(0, 600, 26, 0)).toEqual({ start: 0, end: 0 });
  });
});
