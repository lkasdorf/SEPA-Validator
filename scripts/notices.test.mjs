// Tests for the THIRD-PARTY-NOTICES generator. Run: node --test scripts/notices.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { bundledPackages, fallbackText, pickLicenseFiles, renderNotices, sameText } from "./notices-lib.mjs";

test("dual-licensed MIT OR Apache-2.0 crates use their MIT text", () => {
  assert.deepEqual(pickLicenseFiles(["README.md", "LICENSE-APACHE", "LICENSE-MIT"], "MIT OR Apache-2.0"), ["LICENSE-MIT"]);
  assert.deepEqual(pickLicenseFiles(["LICENSE-MIT", "LICENSE-APACHE"], "Apache-2.0/MIT"), ["LICENSE-MIT"]);
  assert.deepEqual(pickLicenseFiles(["COPYING", "LICENSE-MIT", "UNLICENSE"], "Unlicense OR MIT"), ["LICENSE-MIT"]);
});

test("single-license and NOTICE files are all kept", () => {
  assert.deepEqual(pickLicenseFiles(["LICENSE", "src"], "MIT"), ["LICENSE"]);
  assert.deepEqual(pickLicenseFiles(["NOTICE", "LICENSE-APACHE"], "Apache-2.0"), ["LICENSE-APACHE", "NOTICE"]);
  assert.deepEqual(pickLicenseFiles(["LICENSE.txt", "COPYRIGHT"], "MPL-2.0"), ["COPYRIGHT", "LICENSE.txt"]);
  assert.deepEqual(pickLicenseFiles(["Cargo.toml"], "MIT"), []);
});

test("identical license texts are printed once with everyone who uses them", () => {
  const mit = "MIT License\r\n\r\nCopyright (c) Someone\r\n";
  const out = renderNotices([
    { name: "zeta", version: "1.0.0", license: "MIT", origin: "Rust crate", texts: [mit] },
    { name: "alpha", version: "2.0.0", license: "MIT", origin: "Rust crate", texts: ["MIT License\n\nCopyright (c) Someone\n"] },
    { name: "libxml2", version: "2.15.3", license: "MIT", origin: "native library", texts: ["libxml2 copyright"] },
    { name: "bare", version: "0.1.0", license: "MIT", origin: "Rust crate", texts: [] },
  ]);
  assert.ok(!out.includes("\r"), "LF only");
  assert.ok(out.indexOf("alpha 2.0.0") < out.indexOf("zeta 1.0.0"), "components sorted");
  assert.equal(out.split("Copyright (c) Someone").length - 1, 1, "shared text printed once");
  assert.match(out, /Used by: alpha 2\.0\.0, zeta 1\.0\.0/);
  assert.match(out, /bare 0\.1\.0 .*MIT.*no license file/);
});

test("sameText ignores line endings and trailing whitespace", () => {
  assert.ok(sameText("a\r\nb  \r\n", "a\nb\n"));
  assert.ok(!sameText("a\nb", "a\nc"));
});

test("bundledPackages lists the npm packages a sourcemap pulls in", () => {
  const sources = [
    "../../src/App.svelte",
    "../../node_modules/svelte/src/internal/client/runtime.js",
    "../../node_modules/svelte/src/internal/client/dom/blocks/if.js",
    "../../node_modules/@codemirror/view/dist/index.js",
    "../../node_modules/a/node_modules/b/index.js",
  ];
  assert.deepEqual(bundledPackages(sources), [
    "node_modules/@codemirror/view",
    "node_modules/a/node_modules/b",
    "node_modules/svelte",
  ]);
});

test("fallbackText supplies the standard terms when a package ships no license file", () => {
  assert.match(fallbackText("MIT/Apache-2.0", "unic-common"), /Permission is hereby granted/);
  assert.match(fallbackText("MIT/Apache-2.0", "unic-common"), /unic-common/);
  assert.match(fallbackText("BSD-3-Clause", "alloc-stdlib"), /Neither the name/);
  assert.equal(fallbackText("LicenseRef-Custom", "x"), "");
});
