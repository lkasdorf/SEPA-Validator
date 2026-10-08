import { describe, expect, it } from "vitest";
import { statusLabel, type ValidationResult } from "./types";

const r = (status: ValidationResult["status"], errors: number, warnings: number): ValidationResult => ({
  file: "f.xml", path: "f.xml", namespace: "", schema: "", status, errors, warnings, messages: [],
});

describe("statusLabel", () => {
  it("uses singular and plural correctly", () => {
    expect(statusLabel(r("invalid", 1, 1))).toBe("INVALID (1 error, 1 warning)");
    expect(statusLabel(r("invalid", 3, 0))).toBe("INVALID (3 errors)");
    expect(statusLabel(r("warnings", 0, 2))).toBe("WARNINGS (2)");
  });
});
