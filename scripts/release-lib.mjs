// Release helpers shared by scripts/bump-version.mjs and the release workflow.
// The app version lives in exactly one place: app/src-tauri/tauri.conf.json.

const VERSION_KEY = /^(\s*"version":\s*")([^"]+)(",?\s*)$/m;

/** Set the version in tauri.conf.json text; returns the new text and the old version. */
export function bumpConfig(text, version) {
  // Plain X.Y.Z only: the MSI target rejects pre-release suffixes, and the
  // updater compares plain semver.
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error(`version must be X.Y.Z, got "${version}"`);
  const match = text.match(VERSION_KEY);
  if (!match) throw new Error('no "version" key in tauri.conf.json');
  return { text: text.replace(VERSION_KEY, `$1${version}$3`), previous: match[2] };
}

/** Start the version's section from [Unreleased] and update the compare links. */
export function bumpChangelog(text, previous, version, date) {
  const eol = text.includes("\r\n") ? "\r\n" : "\n";
  const lines = text.split(eol);
  const at = lines.indexOf("## [Unreleased]");
  if (at < 0) throw new Error("CHANGELOG.md has no '## [Unreleased]' heading");
  const next = lines.findIndex((l, i) => i > at && l.startsWith("## ["));
  const body = lines.slice(at + 1, next < 0 ? undefined : next).filter((l) => l.trim());
  if (body.length === 0) throw new Error("the [Unreleased] section is empty: nothing to release");
  lines.splice(at + 1, 0, "", `## [${version}] - ${date}`);
  const link = lines.findIndex((l) => l.startsWith("[Unreleased]: "));
  if (link >= 0) {
    const base = lines[link].slice("[Unreleased]: ".length).replace(/\/compare\/.*$/, "");
    lines.splice(link, 1, `[Unreleased]: ${base}/compare/v${version}...HEAD`, `[${version}]: ${base}/compare/v${previous}...v${version}`);
  }
  return lines.join(eol);
}

/** The body of one version's CHANGELOG section (for the GitHub release / latest.json). */
export function releaseNotes(text, version) {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  const at = lines.findIndex((l) => l.startsWith(`## [${version}]`));
  if (at < 0) throw new Error(`CHANGELOG.md has no section for ${version}`);
  const next = lines.findIndex((l, i) => i > at && (l.startsWith("## [") || /^\[[^\]]+\]: /.test(l)));
  return lines.slice(at + 1, next < 0 ? undefined : next).join("\n").trim();
}

// `tauri bundle` writes the bundle type into the main binary (the updater reads it)
// and restores the unpatched binary afterwards. Code signing must happen after that
// patch, so the release workflow patches app.exe itself before it gets signed.
const BUNDLE_TYPE_UNKNOWN = "__TAURI_BUNDLE_TYPE_VAR_UNK";
const BUNDLE_TYPE_NSIS = "__TAURI_BUNDLE_TYPE_VAR_NSS";

/** Mark a built (unsigned) app.exe as installed by NSIS; returns a patched copy. */
export function patchBundleTypeNsis(binary) {
  const token = Buffer.from(BUNDLE_TYPE_UNKNOWN);
  const at = binary.indexOf(token);
  if (at < 0) throw new Error(`no ${BUNDLE_TYPE_UNKNOWN} marker in the binary (already patched?)`);
  if (binary.indexOf(token, at + 1) >= 0) throw new Error(`more than one ${BUNDLE_TYPE_UNKNOWN} marker in the binary`);
  const out = Buffer.from(binary);
  out.write(BUNDLE_TYPE_NSIS, at, "latin1");
  return out;
}

/** The updater manifest (latest.json) for one Windows NSIS installer. */
export function updaterManifest({ version, notes, pubDate, signature, url }) {
  if (!signature.trim()) throw new Error("empty updater signature");
  const entry = { signature: signature.trim(), url };
  return {
    version,
    notes,
    pub_date: pubDate,
    // Both keys, as tauri-action wrote them: the updater looks up
    // "<os>-<arch>-<bundle type>" first and falls back to "<os>-<arch>".
    platforms: { "windows-x86_64": entry, "windows-x86_64-nsis": entry },
  };
}
