import { describe, expect, it } from "vitest";
import { portableDownloadUrl } from "./update";

describe("portableDownloadUrl", () => {
  it("points at the portable exe of the offered release", () => {
    expect(portableDownloadUrl("2.3.0")).toBe(
      "https://github.com/lkasdorf/SEPA-Validator/releases/download/v2.3.0/SEPA-Validator-2.3.0-windows-x64-portable.exe",
    );
  });
});
