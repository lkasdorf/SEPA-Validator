import { writable, derived } from "svelte/store";
import type { ValidationResult } from "./types";

export const results = writable<ValidationResult[]>([]);
export const selectedIndex = writable<number>(-1);
export const progress = writable<{ done: number; total: number; running: boolean }>({
  done: 0, total: 0, running: false,
});
export type LogFilter = "all" | "errors" | "warnings";
export const logFilter = writable<LogFilter>("all");
export const search = writable<string>("");
export const theme = writable<"system" | "light" | "dark">("system");

export const selectedResult = derived(
  [results, selectedIndex],
  ([$results, $i]) => ($i >= 0 && $i < $results.length ? $results[$i] : null)
);

export const summary = derived(results, ($r) => ({
  total: $r.length,
  ok: $r.filter((x) => x.status === "ok").length,
  invalid: $r.filter((x) => x.status === "invalid" || x.status === "error").length,
  warnings: $r.filter((x) => x.status === "warnings").length,
  noSchema: $r.filter((x) => x.status === "no_schema").length,
}));

export const jumpToLine = writable<(line: number) => void>(() => {});
