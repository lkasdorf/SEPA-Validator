import type { Message } from "./types";

/** Suggested path for "Save formatted…": next to the original, `name_formatted.ext`. */
export function formattedCopyPath(path: string): string {
  const sep = Math.max(path.lastIndexOf("\\"), path.lastIndexOf("/"));
  const dir = path.slice(0, sep + 1);
  const name = path.slice(sep + 1);
  const dot = name.lastIndexOf(".");
  return dot > 0
    ? `${dir}${name.slice(0, dot)}_formatted${name.slice(dot)}`
    : `${dir}${name}_formatted.xml`;
}

/** 1-based lines that carry a message, ascending and unique: CodeMirror's
 *  RangeSetBuilder throws on unsorted input, which silently dropped all highlights. */
export function errorLinesOf(messages: Message[]): number[] {
  const lines = messages.map((m) => m.line ?? 0).filter((l) => l > 0);
  return [...new Set(lines)].sort((a, b) => a - b);
}

/** The smallest single replacement turning `from` into `to` (common prefix and
 *  suffix kept), so re-indenting an edit doesn't move the cursor or the scroll
 *  position more than needed. Offsets are UTF-16 indices, as CodeMirror uses. */
export function minimalChange(from: string, to: string): { from: number; to: number; insert: string } {
  const max = Math.min(from.length, to.length);
  let start = 0;
  while (start < max && from.charCodeAt(start) === to.charCodeAt(start)) start++;
  let end = 0;
  while (end < max - start && from.charCodeAt(from.length - 1 - end) === to.charCodeAt(to.length - 1 - end)) end++;
  return { from: start, to: from.length - end, insert: to.slice(start, to.length - end) };
}
