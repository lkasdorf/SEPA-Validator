import { describe, expect, it } from "vitest";
import { errorLinesOf } from "./viewer";
import type { Message } from "./types";

const msg = (line: number | null): Message => ({ severity: "error", text: "x", line, column: null });

describe("errorLinesOf", () => {
  it("returns located lines sorted and without duplicates (CodeMirror needs sorted ranges)", () => {
    expect(errorLinesOf([msg(20), msg(7), msg(null), msg(16), msg(7)])).toEqual([7, 16, 20]);
  });
});
