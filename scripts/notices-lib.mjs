// Pure helpers for scripts/third-party-notices.mjs (tested in notices.test.mjs).

const LICENSE_FILE = /^(licen[cs]e|copying|copyright|unlicense|notice)/i;

/** Which license files of a package to reproduce. For an "X OR MIT" choice we
 *  take the MIT text only; NOTICE files are always kept (Apache-2.0 §4d). */
export function pickLicenseFiles(files, expression) {
  const candidates = files.filter((f) => LICENSE_FILE.test(f)).sort();
  const notices = candidates.filter((f) => /^notice/i.test(f));
  const licenses = candidates.filter((f) => !/^notice/i.test(f));
  const mit = licenses.find((f) => /mit/i.test(f));
  if (/\bMIT\b/.test(expression) && mit) return [mit, ...notices];
  return [...licenses, ...notices];
}

/** package-lock keys ("node_modules/x", nested "node_modules/a/node_modules/b") of
 *  the npm packages whose files appear in a build's sourcemap sources. */
export function bundledPackages(sources) {
  const keys = new Set();
  for (const s of sources) {
    const path = s.replace(/\\/g, "/");
    const at = path.indexOf("node_modules/");
    if (at < 0) continue;
    const parts = path.slice(at).split("/");
    const key = [];
    for (let i = 0; i < parts.length && parts[i] === "node_modules"; ) {
      const scoped = parts[i + 1]?.startsWith("@");
      key.push("node_modules", ...parts.slice(i + 1, i + (scoped ? 3 : 2)));
      i += scoped ? 3 : 2;
    }
    keys.add(key.join("/"));
  }
  return [...keys].sort();
}

const MIT_TEXT = (name) => `MIT License (standard text; the published package of ${name} has no license file)

Copyright (c) The ${name} authors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.`;

const BSD3_TEXT = (name) => `BSD 3-Clause License (standard text; the published package of ${name} has no license file)

Copyright (c) The ${name} authors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.
2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.
3. Neither the name of the copyright holder nor the names of its contributors
   may be used to endorse or promote products derived from this software
   without specific prior written permission.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.`;

/** Standard license terms for a package that ships no license file ("" if unknown). */
export function fallbackText(expression, name) {
  if (/\bMIT\b/.test(expression)) return MIT_TEXT(name);
  if (/\bBSD-3-Clause\b/.test(expression)) return BSD3_TEXT(name);
  return "";
}

function normalize(text) {
  return text
    .replace(/\r\n?/g, "\n")
    .split("\n")
    .map((l) => l.trimEnd())
    .join("\n")
    .trim();
}

export function sameText(a, b) {
  return normalize(a) === normalize(b);
}

const RULE = "-".repeat(78);

/** components: [{ name, version, license, origin, texts: string[] }] → notices text (LF). */
export function renderNotices(components) {
  const sorted = [...components].sort((a, b) =>
    a.name.toLowerCase() < b.name.toLowerCase() ? -1 : a.name.toLowerCase() > b.name.toLowerCase() ? 1 : a.version < b.version ? -1 : 1,
  );
  const label = (c) => `${c.name} ${c.version}`;
  const out = [
    "SEPA Validator - Third-Party Notices",
    "",
    "SEPA Validator includes the open-source components listed below. Each is",
    "used under its own license; the license texts follow the list.",
    "Generated by scripts/third-party-notices.mjs - do not edit by hand.",
    "",
    "Components",
    "==========",
    "",
  ];
  for (const c of sorted) {
    const missing = c.texts.length ? "" : " - no license file in the published package";
    out.push(`${label(c)}  [${c.license}]  (${c.origin})${missing}`);
  }
  out.push("", "", "License texts", "=============");

  const blocks = new Map(); // normalized text -> component labels
  for (const c of sorted) {
    for (const t of c.texts) {
      const key = normalize(t);
      if (!blocks.has(key)) blocks.set(key, []);
      const users = blocks.get(key);
      if (!users.includes(label(c))) users.push(label(c));
    }
  }
  for (const [text, users] of blocks) {
    out.push("", RULE, `Used by: ${users.join(", ")}`, "", text);
  }
  return `${out.join("\n")}\n`;
}
