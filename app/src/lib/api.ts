import { invoke, Channel } from "@tauri-apps/api/core";
import type { ValidationEvent, ValidationResult, PaymentSummary, SchemaInfo, ImportResult } from "./types";

/** Start validation; `onEvent` is called for each streamed event in order. */
export async function startValidation(
  paths: string[],
  onEvent: (ev: ValidationEvent) => void
): Promise<void> {
  const channel = new Channel<ValidationEvent>();
  channel.onmessage = onEvent;
  await invoke("start_validation", { paths, onEvent: channel });
}

/** Stop the running validation after the file it is currently validating. */
export function cancelValidation(): Promise<void> {
  return invoke("cancel_validation");
}

/** Write the formatted XML of `source` to `target` (formatted in the backend). */
export function saveFormatted(source: string, target: string): Promise<void> {
  return invoke("save_formatted", { source, target });
}

export function readFormatted(path: string): Promise<string> {
  return invoke<string>("read_formatted", { path });
}

export function readPaymentSummary(path: string): Promise<PaymentSummary> {
  return invoke<PaymentSummary>("read_payment_summary", { path });
}

export function writeTextFile(path: string, contents: string): Promise<void> {
  return invoke("write_text_file", { path, contents });
}

export type { ValidationResult };

export function schemaStatus(): Promise<SchemaInfo[]> {
  return invoke<SchemaInfo[]>("schema_status");
}

export function importSchemas(paths: string[]): Promise<ImportResult> {
  return invoke<ImportResult>("import_schemas", { paths });
}

export function openSchemaDir(): Promise<void> {
  return invoke("open_schema_dir");
}

export function openUrl(url: string): Promise<void> {
  return invoke("open_url", { url });
}
