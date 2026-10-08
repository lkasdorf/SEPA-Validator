/// <reference types="node" />
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";

// Guards the theme tokens in app.css against unreadable text (WCAG AA: 4.5:1).
const css = readFileSync(new URL("../app.css", import.meta.url), "utf8");

function tokens(selector: string): Record<string, string> {
  const start = css.indexOf(`${selector} {`);
  const block = css.slice(start, css.indexOf("}", start));
  return Object.fromEntries([...block.matchAll(/--([\w-]+):\s*(#[0-9a-fA-F]{6})/g)].map((m) => [m[1], m[2]]));
}

const light = tokens(":root");
const dark = { ...light, ...tokens(':root[data-theme="dark"]') };

function luminance(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
  const lin = (c: number) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b);
}

function contrast(a: string, b: string): number {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

const pairs: [string, string][] = [
  ["ok", "panel"], ["ok", "chrome"],
  ["err", "panel"], ["err", "chrome"],
  ["warn", "panel"], ["warn", "chrome"],
  ["muted", "panel"], ["muted", "chrome"],
  ["fg", "panel"], ["accent-fg", "accent"],
  ["err-fg", "err"], ["warn-fg", "warn"], // ERROR / WARN badges
];

describe.each([["light", light], ["dark", dark]] as const)("%s theme", (_name, t) => {
  it.each(pairs)("--%s on --%s is readable (>= 4.5:1)", (fg, bg) => {
    expect(t[fg], `--${fg} missing`).toBeDefined();
    expect(t[bg], `--${bg} missing`).toBeDefined();
    expect(contrast(t[fg], t[bg])).toBeGreaterThanOrEqual(4.5);
  });
});
