const REPO = "https://github.com/lkasdorf/SEPA-Validator";

/** Direct download of the portable exe for an offered version (release asset name, see RELEASING.md). */
export function portableDownloadUrl(version: string): string {
  return `${REPO}/releases/download/v${version}/SEPA-Validator-${version}-windows-x64-portable.exe`;
}
