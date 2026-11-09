// Tests for the release helpers. Run: node --test scripts/
import { test } from "node:test";
import assert from "node:assert/strict";
import { bumpConfig, bumpChangelog, patchBundleTypeNsis, releaseNotes, updaterManifest } from "./release-lib.mjs";

const conf = `{\r\n  "productName": "SEPA Validator",\r\n  "version": "2.2.0",\r\n  "identifier": "dev.sepa.validator"\r\n}\r\n`;

const changelog = [
  "# Changelog",
  "",
  "## [Unreleased]",
  "",
  "### Added",
  "- New thing.",
  "",
  "## [2.2.0] - 2026-09-19",
  "",
  "### Added",
  "- Swiss schemas.",
  "",
  "[Unreleased]: https://github.com/o/r/compare/v2.2.0...HEAD",
  "[2.2.0]: https://github.com/o/r/compare/v2.1.0...v2.2.0",
  "",
].join("\r\n");

test("bumpConfig changes only the app version and keeps line endings", () => {
  const { text, previous } = bumpConfig(conf, "2.3.0");
  assert.equal(previous, "2.2.0");
  assert.equal(text, conf.replace('"version": "2.2.0"', '"version": "2.3.0"'));
});

test("bumpConfig rejects versions the updater/MSI can't use", () => {
  assert.throws(() => bumpConfig(conf, "2.3.0-beta.1"));
  assert.throws(() => bumpConfig(conf, "v2.3.0"));
});

test("bumpChangelog turns Unreleased into the new version and fixes the links", () => {
  const out = bumpChangelog(changelog, "2.2.0", "2.3.0", "2026-10-09").split("\r\n");
  assert.deepEqual(out.slice(2, 7), ["## [Unreleased]", "", "## [2.3.0] - 2026-10-09", "", "### Added"]);
  assert.ok(out.includes("[Unreleased]: https://github.com/o/r/compare/v2.3.0...HEAD"));
  assert.ok(out.includes("[2.3.0]: https://github.com/o/r/compare/v2.2.0...v2.3.0"));
});

test("bumpChangelog refuses an empty Unreleased section", () => {
  const empty = changelog.replace("### Added\r\n- New thing.\r\n\r\n", "");
  assert.throws(() => bumpChangelog(empty, "2.2.0", "2.3.0", "2026-10-09"), /Unreleased/);
});

test("releaseNotes returns the body of one version's section", () => {
  assert.equal(releaseNotes(changelog, "2.2.0"), "### Added\n- Swiss schemas.");
  assert.throws(() => releaseNotes(changelog, "9.9.9"), /9\.9\.9/);
});

test("patchBundleTypeNsis replaces the one unknown-bundle marker and leaves the input alone", () => {
  const bin = Buffer.concat([Buffer.from([0, 1, 2]), Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK"), Buffer.from([3, 4])]);
  const out = patchBundleTypeNsis(bin);
  assert.equal(out.length, bin.length);
  assert.ok(out.includes("__TAURI_BUNDLE_TYPE_VAR_NSS"));
  assert.ok(!out.includes("__TAURI_BUNDLE_TYPE_VAR_UNK"));
  assert.deepEqual([...out.subarray(0, 3)], [0, 1, 2]);
  assert.ok(bin.includes("__TAURI_BUNDLE_TYPE_VAR_UNK"));
});

test("patchBundleTypeNsis refuses a patched binary or an ambiguous one", () => {
  assert.throws(() => patchBundleTypeNsis(Buffer.from("__TAURI_BUNDLE_TYPE_VAR_NSS")), /already patched/);
  const twice = Buffer.from("__TAURI_BUNDLE_TYPE_VAR_UNK..__TAURI_BUNDLE_TYPE_VAR_UNK");
  assert.throws(() => patchBundleTypeNsis(twice), /more than one/);
});

test("updaterManifest offers the installer under both Windows keys", () => {
  const m = updaterManifest({ version: "2.7.0", notes: "n", pubDate: "2026-11-09T00:00:00.000Z", signature: "c2ln\n", url: "https://x/setup.exe" });
  assert.deepEqual(m, {
    version: "2.7.0",
    notes: "n",
    pub_date: "2026-11-09T00:00:00.000Z",
    platforms: {
      "windows-x86_64": { signature: "c2ln", url: "https://x/setup.exe" },
      "windows-x86_64-nsis": { signature: "c2ln", url: "https://x/setup.exe" },
    },
  });
  assert.throws(() => updaterManifest({ version: "2.7.0", notes: "", pubDate: "", signature: " ", url: "" }), /signature/);
});
