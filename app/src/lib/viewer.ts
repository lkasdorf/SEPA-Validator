import type { Message } from "./types";

/** 1-based lines that carry a message, ascending and unique: CodeMirror's
 *  RangeSetBuilder throws on unsorted input, which silently dropped all highlights. */
export function errorLinesOf(messages: Message[]): number[] {
  const lines = messages.map((m) => m.line ?? 0).filter((l) => l > 0);
  return [...new Set(lines)].sort((a, b) => a - b);
}
