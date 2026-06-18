import { invoke, Channel } from "@tauri-apps/api/core";
import type { ValidationEvent, ValidationResult } from "./types";

/** Start validation; `onEvent` is called for each streamed event in order. */
export async function startValidation(
  paths: string[],
  onEvent: (ev: ValidationEvent) => void
): Promise<void> {
  const channel = new Channel<ValidationEvent>();
  channel.onmessage = onEvent;
  await invoke("start_validation", { paths, onEvent: channel });
}

export function readFile(path: string): Promise<string> {
  return invoke<string>("read_file", { path });
}

export function readFormatted(path: string): Promise<string> {
  return invoke<string>("read_formatted", { path });
}

export function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke("write_text_file", { path, contents });
}

export type { ValidationResult };
