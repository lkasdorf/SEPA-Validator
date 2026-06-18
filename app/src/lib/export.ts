import { save } from "@tauri-apps/plugin-dialog";
import { writeTextFile } from "./api";
import type { ValidationResult } from "./types";
import { statusLabel } from "./types";

function stamp(): string {
  const d = new Date();
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}_${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}`;
}

export async function exportTxt(results: ValidationResult[]): Promise<void> {
  const path = await save({ defaultPath: `SEPA_Validation_${stamp()}.txt`, filters: [{ name: "Text", extensions: ["txt"] }] });
  if (!path) return;
  const ok = results.filter((r) => r.status === "ok").length;
  let out = `SEPA XML Validation - ${new Date().toISOString()}\n${results.length} files | OK: ${ok} | Failed: ${results.length - ok}\n${"=".repeat(80)}\n`;
  for (const r of results) {
    out += `\nFile: ${r.path}\nNamespace: ${r.namespace}\nSchema: ${r.schema}\nStatus: ${statusLabel(r)}\n`;
    r.messages.forEach((m, i) => {
      const loc = m.line ? ` (Line ${m.line}${m.column ? `, Col ${m.column}` : ""})` : "";
      out += `[${i + 1}] ${m.severity.toUpperCase()}: ${m.text}${loc}\n`;
    });
    out += `${"-".repeat(80)}\n`;
  }
  await writeTextFile(path, out);
}

export async function exportCsv(results: ValidationResult[]): Promise<void> {
  const path = await save({ defaultPath: `SEPA_Validation_${stamp()}.csv`, filters: [{ name: "CSV", extensions: ["csv"] }] });
  if (!path) return;
  const esc = (s: string) => `"${s.replace(/"/g, '""')}"`;
  let out = "file;namespace;schema;status;errors;warnings\n";
  for (const r of results) {
    out += [esc(r.file), esc(r.namespace), esc(r.schema), esc(statusLabel(r)), r.errors, r.warnings].join(";") + "\n";
  }
  await writeTextFile(path, out);
}
