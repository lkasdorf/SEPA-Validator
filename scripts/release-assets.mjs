#!/usr/bin/env node
// Release workflow helpers (see .github/workflows/release.yml).
//   node scripts/release-assets.mjs patch-nsis <app.exe>
//       write the NSIS bundle type into app.exe (before it is code-signed)
//   node scripts/release-assets.mjs manifest <version> <installer url> <installer.sig> <out latest.json>
//       write the updater manifest, with the CHANGELOG section as notes
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { patchBundleTypeNsis, releaseNotes, updaterManifest } from "./release-lib.mjs";

const [command, ...args] = process.argv.slice(2);
try {
  if (command === "patch-nsis" && args.length === 1) {
    writeFileSync(args[0], patchBundleTypeNsis(readFileSync(args[0])));
    console.log(`Patched ${args[0]} with bundle type nsis.`);
  } else if (command === "manifest" && args.length === 4) {
    const [version, url, sigPath, out] = args;
    const changelog = readFileSync(fileURLToPath(new URL("../CHANGELOG.md", import.meta.url)), "utf8");
    const manifest = updaterManifest({
      version,
      notes: releaseNotes(changelog, version),
      pubDate: new Date().toISOString(),
      signature: readFileSync(sigPath, "utf8"),
      url,
    });
    writeFileSync(out, `${JSON.stringify(manifest, null, 2)}\n`);
    console.log(`Wrote ${out}.`);
  } else {
    throw new Error("usage: release-assets.mjs patch-nsis <exe> | manifest <version> <url> <sig> <out>");
  }
} catch (e) {
  console.error(`release-assets: ${e.message}`);
  process.exit(1);
}
