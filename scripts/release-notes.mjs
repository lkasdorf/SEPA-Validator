#!/usr/bin/env node
// Print the CHANGELOG section for a version (used as GitHub release notes and in latest.json).
// Usage: node scripts/release-notes.mjs 2.3.0
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { releaseNotes } from "./release-lib.mjs";

const changelog = readFileSync(fileURLToPath(new URL("../CHANGELOG.md", import.meta.url)), "utf8");
try {
  process.stdout.write(`${releaseNotes(changelog, process.argv[2] ?? "")}\n`);
} catch (e) {
  console.error(`release-notes: ${e.message}`);
  process.exit(1);
}
