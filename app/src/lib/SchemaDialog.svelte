<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { schemaStatus, importSchemas, openSchemaDir, openUrl } from "./api";
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
      note = "Could not load status.";
    }
  }

  async function importFiles() {
    const sel = await openDialog({ multiple: true, filters: [{ name: "XSD/ZIP", extensions: ["xsd", "zip"] }] });
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
      note = `${r.imported} schema file(s) imported${r.skipped.length ? `, ${r.skipped.length} skipped` : ""}.`;
      await refresh();
    } catch {
      note = "Import failed.";
    }
    busy = false;
  }

  function close() {
    schemaDialogOpen.set(false);
  }

  async function openFolder() {
    try {
      await openSchemaDir();
    } catch {
      note = "Could not open folder.";
    }
  }

  const DOWNLOAD_URL = "https://www.ebics.de/de/datenformate";
  async function download() {
    try {
      await openUrl(DOWNLOAD_URL);
    } catch {
      note = "Could not open download page.";
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Schemas">
    <header>
      <strong>Schemas</strong>
      <button class="x" on:click={close} aria-label="Close">✕</button>
    </header>
    <p class="hint">The XSDs are not bundled. Download them from the official source (ebics.de for DK/GBIC, iso20022.org for the ISO schemas) and import them here as ZIP or XSD.</p>
    <div class="tablewrap">
      <table>
        <thead><tr><th>Namespace</th><th>File</th><th>Status</th></tr></thead>
        <tbody>
          {#each rows as r}
            <tr>
              <td class="ns mono">{r.namespace}</td>
              <td class="mono">{r.filename}</td>
              <td class={r.present ? "ok" : "missing"}>{r.present ? "✓ present" : "✗ missing"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
    {#if note}<p class="note">{note}</p>{/if}
    <footer>
      <button class="btn btn--ghost" on:click={download}>Download…</button>
      <button class="btn btn--primary" on:click={importFiles} disabled={busy}>XSD/ZIP files…</button>
      <button class="btn btn--primary" on:click={importFolder} disabled={busy}>Folder…</button>
      <button class="btn btn--ghost" on:click={openFolder}>Open folder</button>
      <button class="btn btn--ghost close" on:click={close}>Close</button>
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
  footer { display: flex; gap: var(--sp-2); margin-top: var(--sp-3); }
  footer .close { margin-left: auto; }
</style>
