<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { schemaStatus } from "./api";
  import { results, progress, theme, schemaDialogOpen, schemaCounts, dragging } from "./stores";
  import { runValidation as run, cancelValidation, pickFiles, pickFolder } from "./validation";
  import { exportTxt, exportCsv } from "./export";
  import { get } from "svelte/store";
  import Menu from "./Menu.svelte";
  function doExportTxt() { exportTxt(get(results)); }
  function doExportCsv() { exportCsv(get(results)); }

  async function refreshSchemaBadge() {
    try {
      const s = await schemaStatus();
      schemaCounts.set({ present: s.filter((x) => x.present).length, total: s.length });
    } catch {
      schemaCounts.set({ present: 0, total: 0 });
    }
  }
  $: if (!$schemaDialogOpen) refreshSchemaBadge();
  function toggleTheme() {
    theme.update((t) => (t === "dark" ? "light" : "dark"));
  }

  onMount(() => {
    const un = getCurrentWebview().onDragDropEvent((event) => {
      const type = event.payload.type;
      dragging.set(type === "enter" || type === "over");
      if (type === "drop") run(event.payload.paths);
    });
    return () => { un.then((f) => f()); };
  });
</script>

<header class="toolbar">
  <strong class="brand">SEPA XML Validator</strong>
  <button class="btn btn--primary" on:click={pickFiles}>Select Files…</button>
  <button class="btn btn--ghost" on:click={pickFolder}>Select Folder…</button>
  <button class="btn btn--ghost" on:click={doExportTxt} disabled={$results.length === 0}>Export TXT</button>
  <button class="btn btn--ghost" on:click={doExportCsv} disabled={$results.length === 0}>Export CSV</button>
  <button class="btn btn--ghost" on:click={() => schemaDialogOpen.set(true)}>Schemas… {$schemaCounts.total ? `(${$schemaCounts.present}/${$schemaCounts.total})` : ""}</button>
  {#if $progress.running}
    <button class="btn btn--ghost" on:click={cancelValidation}>Cancel</button>
  {:else}
    <span class="hint">or drag &amp; drop files here</span>
  {/if}
  <div class="right">
    <Menu />
    <button class="btn btn--ghost" on:click={toggleTheme} title="Toggle theme" aria-label="Toggle theme">◐</button>
  </div>
</header>

<style>
  .toolbar { display: flex; gap: var(--sp-2); align-items: center; padding: var(--sp-2) var(--sp-3); background: var(--chrome); color: var(--fg); border-bottom: 1px solid var(--border); }
  .brand { margin-right: var(--sp-2); padding-left: var(--sp-2); font-weight: 600; letter-spacing: .01em; border-left: 3px solid var(--accent); white-space: nowrap; }
  .hint { color: var(--muted); font-size: 12px; }
  @media (max-width: 960px) { .hint { display: none; } }
  .right { margin-left: auto; display: flex; gap: var(--sp-2); align-items: center; }
  .right button { font-size: 14px; }
</style>
