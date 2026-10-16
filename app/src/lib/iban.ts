import { openUrl } from "./api";

/** Link to the online IBAN check (validity, bank, BIC) for `iban`, or null if
 *  it is not shaped like an IBAN. Spaces are dropped and letters uppercased;
 *  only [A-Z0-9] ends up in the URL path. */
export function ibanCheckUrl(iban: string | null | undefined): string | null {
  const v = (iban ?? "").replace(/\s+/g, "").toUpperCase();
  return /^[A-Z]{2}\d{2}[A-Z0-9]{1,30}$/.test(v) ? `https://www.ibancalculator.com/validate/${v}` : null;
}

/** Open the online check in the default browser. This sends the IBAN to
 *  ibancalculator.com, so it only ever runs on an explicit click. */
export function checkIbanOnline(iban: string | null | undefined): void {
  const url = ibanCheckUrl(iban);
  if (url) void openUrl(url).catch((e) => console.error("Opening the IBAN check failed:", e));
}
