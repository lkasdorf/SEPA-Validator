<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { schemaStatus, importSchemas, openSchemaDir } from "./api";
  import { schemaDialogOpen } from "./stores";
  import type { SchemaInfo } from "./types";

  let rows: SchemaInfo[] = [];
  let busy = false;
  let note = "";

  onMount(refresh);

  async function refresh() {
    try {
      rows = await schemaStatus();
    } catch {
      note = "Status konnte nicht geladen werden.";
    }
  }

  async function importFiles() {
    const sel = await openDialog({ multiple: true, filters: [{ name: "XSD", extensions: ["xsd"] }] });
    if (!sel) return;
    await runImport(Array.isArray(sel) ? sel : [sel]);
  }

  async function importFolder() {
    const sel = await openDialog({ directory: true });
    if (!sel) return;
    await runImport([sel as string]);
  }

  async function runImport(paths: string[]) {
    busy = true;
    note = "";
    try {
      const r = await importSchemas(paths);
      note = `${r.imported} XSD-Datei(en) importiert${r.skipped.length ? `, ${r.skipped.length} übersprungen` : ""}.`;
      await refresh();
    } catch {
      note = "Import fehlgeschlagen.";
    }
    busy = false;
  }

  function close() {
    schemaDialogOpen.set(false);
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Schemas">
    <header>
      <strong>Schemas</strong>
      <button class="x" on:click={close} aria-label="Schließen">✕</button>
    </header>
    <p class="hint">Die XSDs werden nicht mitgeliefert. Importiere die ISO-20022/GBIC-Schemas, um zu validieren.</p>
    <div class="tablewrap">
      <table>
        <thead><tr><th>Namespace</th><th>Datei</th><th>Status</th></tr></thead>
        <tbody>
          {#each rows as r}
            <tr>
              <td class="ns">{r.namespace}</td>
              <td>{r.filename}</td>
              <td class={r.present ? "ok" : "missing"}>{r.present ? "✓ vorhanden" : "✗ fehlt"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if note}<p class="note">{note}</p>{/if}
    <footer>
      <button on:click={importFiles} disabled={busy}>XSD-Dateien…</button>
      <button on:click={importFolder} disabled={busy}>Ordner…</button>
      <button on:click={openSchemaDir}>Ordner öffnen</button>
      <button class="close" on:click={close}>Schließen</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .dialog {
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: min(720px, 92vw);
    max-height: 86vh;
    display: flex;
    flex-direction: column;
    padding: 14px 16px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .hint { opacity: 0.8; font-size: 12px; margin: 6px 0 10px; }
  .tablewrap { overflow: auto; min-height: 0; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); }
  td.ns { word-break: break-all; }
  td.ok { color: var(--ok); }
  td.missing { color: var(--err); }
  .note { font-size: 12px; margin: 8px 0 0; }
  footer { display: flex; gap: 8px; margin-top: 12px; }
  footer button { background: var(--accent); color: #fff; border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; font-size: 12px; }
  footer button:hover:not(:disabled) { filter: brightness(1.1); }
  footer button:disabled { opacity: 0.45; cursor: default; }
  footer button.close { margin-left: auto; background: transparent; color: var(--fg); border: 1px solid var(--border); }
</style>
