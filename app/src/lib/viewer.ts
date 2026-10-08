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
