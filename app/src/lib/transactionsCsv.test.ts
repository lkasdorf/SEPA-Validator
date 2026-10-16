import { describe, expect, it } from "vitest";
import { transactionsCsv } from "./transactionsCsv";
import { visibleRange } from "./virtualRows";
import { filterIndices, searchIndex } from "./transactionFilter";
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

describe("transaction search", () => {
  const rows = [
    tx({ name: "Anna Müller", iban: "DE89370400440532013000", ustrd: "Beitrag Mai", mandateId: "MND-001" }),
    tx({ name: "Bernd Schmidt", iban: "DE02120300000000202051", ustrd: "Beitrag Juni\nNachzahlung", mandateId: "MND-002" }),
    tx({ name: null, iban: null, ustrd: null, mandateId: null, amount: "10.00" }),
  ];
  const idx = searchIndex(rows);

  it("empty query matches everything", () => {
    expect(filterIndices(idx, "")).toEqual([0, 1, 2]);
    expect(filterIndices(idx, "   ")).toEqual([0, 1, 2]);
  });

  it("matches name, IBAN, remittance info and mandate ID case-insensitively", () => {
    expect(filterIndices(idx, "MÜLLER")).toEqual([0]);
    expect(filterIndices(idx, "de0212")).toEqual([1]);
    expect(filterIndices(idx, "nachzahlung")).toEqual([1]);
    expect(filterIndices(idx, "mnd-00")).toEqual([0, 1]);
  });

  it("requires every term, across fields", () => {
    expect(filterIndices(idx, "beitrag mai")).toEqual([0]);
    expect(filterIndices(idx, "schmidt mnd-001")).toEqual([]);
  });

  it("does not search the amount", () => {
    expect(filterIndices(idx, "10.00")).toEqual([]);
  });

  it("CSV keeps the original transaction numbers of a filtered list", () => {
    const csv = transactionsCsv([rows[1]], true, [2]);
    expect(csv.slice(1).split("\n")[1].startsWith('2;"Bernd Schmidt"')).toBe(true);
  });
});
