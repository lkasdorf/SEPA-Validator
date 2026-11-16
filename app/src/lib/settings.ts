import { writable } from "svelte/store";

/** Groups of DK usage rules; the ids match `rules::Group` in the backend. */
export type RuleGroup =
  | "characters"
  | "references"
  | "payment-type"
  | "creditor"
  | "amounts-names"
  | "addresses-remittance";

export const RULE_GROUPS: { id: RuleGroup; label: string; detail: string }[] = [
  {
    id: "characters",
    label: "Character set",
    detail: "Only the DK character set (IDs without umlauts or & * $ %), no namespace prefixes.",
  },
  {
    id: "references",
    label: "References and IDs",
    detail: "MsgId, PmtInfId, EndToEndId, MndtId … must not start or end with / or contain //.",
  },
  {
    id: "payment-type",
    label: "Payment type and levels",
    detail:
      "Service level, LclInstrm, no CORE/B2B mix, ChrgBr SLEV; PmtTpInf, CdtrSchmeId, ultimate parties and ChrgBr on one level only.",
  },
  {
    id: "creditor",
    label: "Creditor ID and mandate",
    detail: "Creditor identifier check digits and scheme SEPA, mandate amendments (AmdmntInd, SMNDA).",
  },
  {
    id: "amounts-names",
    label: "Amounts and names",
    detail: "Names at most 70 characters; amounts 0.01 to 999,999,999.99 with 2 decimals; direct debits in EUR.",
  },
  {
    id: "addresses-remittance",
    label: "Addresses and remittance information",
    detail:
      "Postal addresses with town and country (ISO 2019 versions), debtor address outside the EU/EEA; one Ustrd or one Strd (SCOR).",
  },
];

const KEY = "sepa-validator.disabledRules";
const IDS = new Set<string>(RULE_GROUPS.map((g) => g.id));

/** The stored list of switched-off groups. Unknown or broken values count as
 *  "all on", so new groups are on by default. */
export function parseDisabled(raw: string | null): RuleGroup[] {
  if (!raw) return [];
  try {
    const v: unknown = JSON.parse(raw);
    if (!Array.isArray(v)) return [];
    return [...new Set(v.filter((x): x is RuleGroup => typeof x === "string" && IDS.has(x)))];
  } catch {
    return [];
  }
}

function storage(): Storage | null {
  try {
    return globalThis.localStorage ?? null;
  } catch {
    return null;
  }
}

/** DK rule groups switched off in the settings (all on by default). */
export const disabledRules = writable<RuleGroup[]>(parseDisabled(storage()?.getItem(KEY) ?? null));

disabledRules.subscribe((groups) => {
  try {
    storage()?.setItem(KEY, JSON.stringify(groups));
  } catch {
    /* storage unavailable: the setting lasts for this session */
  }
});

/** Switch one group on or off. */
export function setRuleGroup(id: RuleGroup, enabled: boolean): void {
  disabledRules.update((d) => (enabled ? d.filter((x) => x !== id) : d.includes(id) ? d : [...d, id]));
}

export const settingsDialogOpen = writable<boolean>(false);
