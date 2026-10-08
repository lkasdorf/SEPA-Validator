import { describe, expect, it } from "vitest";
import { errorLinesOf, formattedCopyPath } from "./viewer";
import type { Message } from "./types";

const msg = (line: number | null): Message => ({ severity: "error", text: "x", line, column: null, hint: null });

describe("errorLinesOf", () => {
  it("returns located lines sorted and without duplicates (CodeMirror needs sorted ranges)", () => {
    expect(errorLinesOf([msg(20), msg(7), msg(null), msg(16), msg(7)])).toEqual([7, 16, 20]);
  });
});

describe("formattedCopyPath", () => {
  it("suggests a sibling file with a _formatted suffix", () => {
    expect(formattedCopyPath(String.raw`C:\Daten\SEPA\20261001_MUSTER_PAIN00800108.xml`)).toBe(
      String.raw`C:\Daten\SEPA\20261001_MUSTER_PAIN00800108_formatted.xml`,
    );
  });

  it("keeps the extension's case and handles names without an extension", () => {
    expect(formattedCopyPath(String.raw`C:\x\a.XML`)).toBe(String.raw`C:\x\a_formatted.XML`);
    expect(formattedCopyPath(String.raw`C:\x.d\noext`)).toBe(String.raw`C:\x.d\noext_formatted.xml`);
  });
});
