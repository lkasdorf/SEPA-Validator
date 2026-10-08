#!/usr/bin/env node
// Bump the app version (single source: app/src-tauri/tauri.conf.json) and turn the
// CHANGELOG's [Unreleased] section into the new version's section.
// Usage: node scripts/bump-version.mjs 2.3.0
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { bumpConfig, bumpChangelog } from "./release-lib.mjs";

const version = process.argv[2];
if (!version) {
  console.error("usage: node scripts/bump-version.mjs X.Y.Z");
  process.exit(1);
}
const root = fileURLToPath(new URL("..", import.meta.url));
const confPath = `${root}app/src-tauri/tauri.conf.json`;
const changelogPath = `${root}CHANGELOG.md`;

try {
  const conf = bumpConfig(readFileSync(confPath, "utf8"), version);
  const d = new Date();
  const today = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  const changelog = bumpChangelog(readFileSync(changelogPath, "utf8"), conf.previous, version, today);
  writeFileSync(confPath, conf.text);
  writeFileSync(changelogPath, changelog);
  console.log(`Version ${conf.previous} -> ${version}.`);
  console.log("Next: review CHANGELOG.md, commit and merge, then tag the merge commit on master:");
  console.log(`  git tag v${version} && git push origin v${version}`);
} catch (e) {
  console.error(`bump-version: ${e.message}`);
  process.exit(1);
}
