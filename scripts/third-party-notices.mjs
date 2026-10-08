#!/usr/bin/env node
// Generate app/THIRD-PARTY-NOTICES.txt from what is built into the app: Rust crates
// (normal dependencies for Windows, without proc-macros), the npm packages that end
// up in the frontend bundle (read from a sourcemap build), and the statically linked
// native libraries from vcpkg (libxml2[core,zlib] + zlib).
//
// Usage: node scripts/third-party-notices.mjs [--check]
//   Needs `cargo`, app/node_modules (npm ci) and VCPKG_ROOT (or app/src-tauri/.cargo/config.toml).
//   --check: exit 1 if the committed file is out of date (CI).
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { bundledPackages, fallbackText, pickLicenseFiles, renderNotices, sameText } from "./notices-lib.mjs";

const root = fileURLToPath(new URL("..", import.meta.url));
const appDir = join(root, "app");
const tauriDir = join(appDir, "src-tauri");
const outFile = join(appDir, "THIRD-PARTY-NOTICES.txt");
const TARGET = "x86_64-pc-windows-msvc";
const TRIPLET = process.env.VCPKGRS_TRIPLET || "x64-windows-static-md";
// libxml2 is built as libxml2[core,zlib]: no libiconv (LGPL).
const NATIVE = [
  ["libxml2", "MIT"],
  ["zlib", "Zlib"],
];

const read = (dir, files) => files.map((f) => readFileSync(join(dir, f), "utf8"));

/** License texts of a package directory, or the standard terms if it ships none. */
function licenseTexts(dir, license, name) {
  const texts = dir ? read(dir, pickLicenseFiles(readdirSync(dir), license)) : [];
  const fallback = texts.length ? "" : fallbackText(license, name);
  return fallback ? [fallback] : texts;
}

function rustCrates() {
  const cargo = (args) => execFileSync("cargo", args, { cwd: tauriDir, encoding: "utf8", maxBuffer: 1 << 28 });
  const meta = JSON.parse(cargo(["metadata", "--format-version", "1", "--locked", "--filter-platform", TARGET]));
  const byId = new Map(meta.packages.map((p) => [`${p.name} ${p.version}`, p]));
  const tree = cargo(["tree", "-e", "normal,no-proc-macro", "--target", TARGET, "--prefix", "none", "--format", "{p}", "--locked"]);
  const crates = new Map();
  for (const line of tree.split(/\r?\n/)) {
    const [name, v] = line.trim().split(" ");
    if (!name || name === "app") continue; // the app itself
    const version = v.replace(/^v/, "");
    const key = `${name} ${version}`;
    if (crates.has(key)) continue;
    const pkg = byId.get(key);
    const license = pkg?.license ?? (pkg?.license_file ? "see license file" : "unknown");
    const dir = pkg && dirname(pkg.manifest_path);
    crates.set(key, { name, version, license, origin: "Rust crate", texts: licenseTexts(dir, license, name) });
  }
  return [...crates.values()];
}

/** npm packages compiled into the frontend bundle (e.g. the Svelte runtime, which is
 *  a devDependency), found via the sourcemap of a production build. */
function npmPackages() {
  const out = mkdtempSync(join(tmpdir(), "sepa-notices-"));
  try {
    const vite = join(appDir, "node_modules", "vite", "bin", "vite.js");
    execFileSync(process.execPath, [vite, "build", "--sourcemap", "--outDir", out, "--emptyOutDir", "--logLevel", "error"], { cwd: appDir, stdio: "inherit" });
    const assets = join(out, "assets");
    const sources = readdirSync(assets)
      .filter((f) => f.endsWith(".map"))
      .flatMap((f) => JSON.parse(readFileSync(join(assets, f), "utf8")).sources);
    const lock = JSON.parse(readFileSync(join(appDir, "package-lock.json"), "utf8"));
    return bundledPackages(sources).map((key) => {
      const dir = join(appDir, key);
      const pj = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));
      const license = typeof pj.license === "string" ? pj.license : (pj.license?.type ?? "unknown");
      const name = key.replace(/^.*node_modules\//, "");
      return { name, version: lock.packages[key]?.version ?? pj.version, license, origin: "npm package", texts: licenseTexts(dir, license, name) };
    });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}

function vcpkgRoot() {
  if (process.env.VCPKG_ROOT) return process.env.VCPKG_ROOT;
  const config = join(tauriDir, ".cargo", "config.toml");
  const m = existsSync(config) && readFileSync(config, "utf8").match(/^VCPKG_ROOT\s*=\s*"([^"]+)"/m);
  if (!m) throw new Error("set VCPKG_ROOT (or add it to app/src-tauri/.cargo/config.toml)");
  return m[1];
}

function nativeLibraries() {
  const shareRoot = join(vcpkgRoot(), "installed", TRIPLET, "share");
  return NATIVE.map(([port, license]) => {
    const share = join(shareRoot, port);
    const spdx = JSON.parse(readFileSync(join(share, "vcpkg.spdx.json"), "utf8"));
    const version = String(spdx.packages[0].versionInfo).replace(/#.*$/, "");
    return { name: port, version, license, origin: "native library, statically linked", texts: read(share, ["copyright"]) };
  });
}

try {
  const components = [...nativeLibraries(), ...rustCrates(), ...npmPackages()];
  const text = renderNotices(components);
  if (process.argv.includes("--check")) {
    const current = existsSync(outFile) ? readFileSync(outFile, "utf8") : "";
    if (!sameText(current, text)) {
      console.error("app/THIRD-PARTY-NOTICES.txt is out of date: run `node scripts/third-party-notices.mjs` and commit it.");
      process.exit(1);
    }
    console.log(`THIRD-PARTY-NOTICES.txt is up to date (${components.length} components).`);
  } else {
    writeFileSync(outFile, text);
    console.log(`Wrote app/THIRD-PARTY-NOTICES.txt (${components.length} components, ${Math.round(text.length / 1024)} KB).`);
  }
} catch (e) {
  console.error(`third-party-notices: ${e.message}`);
  process.exit(1);
}
