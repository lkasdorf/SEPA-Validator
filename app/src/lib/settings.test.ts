import { describe, expect, it } from "vitest";
import { get } from "svelte/store";
import { disabledRules, parseDisabled, RULE_GROUPS, setRuleGroup } from "./settings";

describe("parseDisabled", () => {
  it("treats a missing or broken setting as all rules on", () => {
    expect(parseDisabled(null)).toEqual([]);
    expect(parseDisabled("")).toEqual([]);
    expect(parseDisabled("{")).toEqual([]);
    expect(parseDisabled('{"a":1}')).toEqual([]);
  });

  it("keeps known groups once and drops unknown ones", () => {
    expect(parseDisabled('["characters","nope","characters",3,"creditor"]')).toEqual(["characters", "creditor"]);
  });
});

describe("setRuleGroup", () => {
  it("switches groups off and on again", () => {
    disabledRules.set([]);
    setRuleGroup("references", false);
    setRuleGroup("references", false);
    setRuleGroup("creditor", false);
    expect(get(disabledRules)).toEqual(["references", "creditor"]);
    setRuleGroup("references", true);
    expect(get(disabledRules)).toEqual(["creditor"]);
    disabledRules.set([]);
  });
});

describe("RULE_GROUPS", () => {
  it("has the six backend groups", () => {
    expect(RULE_GROUPS.map((g) => g.id)).toEqual([
      "characters",
      "references",
      "payment-type",
      "creditor",
      "amounts-names",
      "addresses-remittance",
    ]);
  });
});
