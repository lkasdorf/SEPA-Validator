import { describe, expect, it } from "vitest";
import { createLatest } from "./latest";

describe("createLatest", () => {
  it("treats only the most recent request as current", () => {
    const latest = createLatest();
    const a = latest.begin();
    const b = latest.begin();
    expect(latest.isCurrent(a)).toBe(false);
    expect(latest.isCurrent(b)).toBe(true);
  });
});
