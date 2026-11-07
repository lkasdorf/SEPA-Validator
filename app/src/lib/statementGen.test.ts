import { describe, expect, it } from "vitest";
import { canGenerate, isPaymentFile, normalizeOpening, previewLine } from "./statementGen";
import type { StatementPreview } from "./types";

describe("isPaymentFile", () => {
  it("accepts pain.001 and pain.008, including the Swiss variants", () => {
    expect(isPaymentFile("urn:iso:std:iso:20022:tech:xsd:pain.001.001.09")).toBe(true);
    expect(isPaymentFile("urn:iso:std:iso:20022:tech:xsd:pain.008.001.02")).toBe(true);
    expect(isPaymentFile("http://www.six-interbank-clearing.com/de/pain.001.001.09.ch.03.xsd")).toBe(true);
  });

  it("rejects statements, status reports and unknown files", () => {
    expect(isPaymentFile("urn:iso:std:iso:20022:tech:xsd:camt.053.001.08")).toBe(false);
    expect(isPaymentFile("urn:iso:std:iso:20022:tech:xsd:pain.002.001.10")).toBe(false);
    expect(isPaymentFile("")).toBe(false);
  });
});

describe("normalizeOpening", () => {
  it("accepts plain and thousands-separated amounts", () => {
    expect(normalizeOpening("10000.00")).toBe("10000.00");
    expect(normalizeOpening("10,000.00")).toBe("10000.00");
    expect(normalizeOpening("-250.5")).toBe("-250.50");
    expect(normalizeOpening(" 42 ")).toBe("42.00");
  });

  it("refuses German decimal commas, three decimals and text", () => {
    for (const bad of ["10.000,00", "12,50", "1.234", "abc", "", "-"]) {
      expect(normalizeOpening(bad), bad).toBeNull();
    }
  });
});

const preview = (p: Partial<StatementPreview>): StatementPreview => ({
  kind: "pain.001",
  accounts: 1,
  days: 1,
  statements: 1,
  transactions: 1,
  problems: [],
  ...p,
});

describe("canGenerate", () => {
  const ready = { preview: preview({}), opening: "10000.00", busy: false, validating: false };

  it("allows generating a bookable file with a valid opening balance", () => {
    expect(canGenerate(ready)).toBe(true);
  });

  it("waits while a validation run is active, so appending can't cut it short", () => {
    expect(canGenerate({ ...ready, validating: true })).toBe(false);
  });

  it("refuses without a preview, with problems, while busy or with a bad amount", () => {
    expect(canGenerate({ ...ready, preview: null })).toBe(false);
    expect(canGenerate({ ...ready, preview: preview({ problems: ["x"] }) })).toBe(false);
    expect(canGenerate({ ...ready, busy: true })).toBe(false);
    expect(canGenerate({ ...ready, opening: null })).toBe(false);
  });
});

describe("previewLine", () => {
  it("counts booking days and transactions", () => {
    expect(previewLine(preview({ days: 3, statements: 3, transactions: 14 }))).toBe("3 booking days, 14 transactions");
    expect(previewLine(preview({}))).toBe("1 booking day, 1 transaction");
  });

  it("names the statements and accounts when there are several accounts", () => {
    expect(previewLine(preview({ accounts: 2, days: 3, statements: 4, transactions: 7 }))).toBe(
      "4 statements for 2 accounts over 3 booking days, 7 transactions"
    );
  });
});
