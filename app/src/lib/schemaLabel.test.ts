import { describe, expect, it } from "vitest";
import { schemaLabel } from "./schemaLabel";

describe("schemaLabel", () => {
  it("names the DK pain.001.001.09 subsets apart", () => {
    expect(schemaLabel("pain.001.001.09.xsd")).toBe("SEPA credit transfer (DK)");
    expect(schemaLabel("pain.001.001.09_AXZ_GBIC_5.xsd")).toBe("Foreign payment (DK AXZ)");
    expect(schemaLabel("pain.001.001.09_CCU_GBIC_5.xsd")).toBe("Urgent euro transfer (DK CCU)");
  });

  it("returns undefined for unknown schemas", () => {
    expect(schemaLabel("made-up.xsd")).toBeUndefined();
    expect(schemaLabel("")).toBeUndefined();
  });
});
