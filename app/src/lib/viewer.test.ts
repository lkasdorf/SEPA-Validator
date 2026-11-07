import { describe, expect, it } from "vitest";
import { errorLinesOf, formattedCopyPath, minimalChange } from "./viewer";
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

describe("minimalChange", () => {
  const apply = (a: string, c: { from: number; to: number; insert: string }) =>
    a.slice(0, c.from) + c.insert + a.slice(c.to);

  it("replaces only the part between the common prefix and suffix", () => {
    const before = "<a>\n  <b>1</b><c>2</c>\n</a>";
    const after = "<a>\n  <b>1</b>\n  <c>2</c>\n</a>";
    const c = minimalChange(before, after);
    expect(c).toEqual({ from: 14, to: 14, insert: "\n  " });
    expect(apply(before, c)).toBe(after);
  });

  it("is empty for equal texts and handles repeated characters", () => {
    expect(minimalChange("abc", "abc")).toEqual({ from: 3, to: 3, insert: "" });
    for (const [a, b] of [["aaa", "aa"], ["aa", "aaaa"], ["", "x"], ["xy", ""], ["abcabc", "abc"]]) {
      expect(apply(a, minimalChange(a, b))).toBe(b);
    }
  });
});
