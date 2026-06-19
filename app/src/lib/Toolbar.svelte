<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { startValidation } from "./api";
  import { results, selectedIndex, progress, theme } from "./stores";
  import type { ValidationEvent } from "./types";
  import { exportTxt, exportCsv } from "./export";
  import { get } from "svelte/store";
  import { schemaDialogOpen } from "./stores";
  import { schemaStatus } from "./api";
  function doExportTxt() { exportTxt(get(results)); }
  function doExportCsv() { exportCsv(get(results)); }

  let schemaPresent = 0;
  let schemaTotal = 0;
  async function refreshSchemaBadge() {
    try {
      const s = await schemaStatus();
      schemaPresent = s.filter((x) => x.present).length;
      schemaTotal = s.length;
    } catch {
      schemaPresent = 0;
      schemaTotal = 0;
    }
  }
  $: if (!$schemaDialogOpen) refreshSchemaBadge();

  async function run(paths: string[]) {
    if (paths.length === 0) return;
    results.set([]);
    selectedIndex.set(-1);
    progress.set({ done: 0, total: 0, running: true });
    await startValidation(paths, (ev: ValidationEvent) => {
      if (ev.event === "started") {
        progress.set({ done: 0, total: ev.data.total, running: true });
      } else if (ev.event === "result") {
        results.update((r) => { r.push(ev.data.result); return r; });
        progress.update((p) => ({ ...p, done: p.done + 1 }));
        if (ev.data.index === 0) selectedIndex.set(0);
      } else if (ev.event === "finished") {
        progress.update((p) => ({ ...p, running: false }));
      }
    });
  }

  async function pickFiles() {
    const sel = await open({ multiple: true, filters: [{ name: "XML", extensions: ["xml"] }] });
    if (sel) run(Array.isArray(sel) ? sel : [sel]);
  }
  async function pickFolder() {
    const sel = await open({ directory: true });
    if (sel) run([sel as string]);
  }
  function toggleTheme() {
    theme.update((t) => (t === "dark" ? "light" : "dark"));
  }

  onMount(() => {
    const un = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop") run(event.payload.paths);
    });
    return () => { un.then((f) => f()); };
  });
</script>

<header class="toolbar">
  <strong class="brand">SEPA XML Validator</strong>
  <button on:click={pickFiles}>Select Files…</button>
  <button on:click={pickFolder}>Select Folder…</button>
  <button on:click={doExportTxt} disabled={$results.length === 0}>Export TXT</button>
  <button on:click={doExportCsv} disabled={$results.length === 0}>Export CSV</button>
  <button on:click={() => schemaDialogOpen.set(true)}>Schemas… {schemaTotal ? `(${schemaPresent}/${schemaTotal})` : ""}</button>
  <span class="hint">or drag &amp; drop files here</span>
  <button class="theme" on:click={toggleTheme} title="Toggle theme">◐</button>
</header>

<style>
  .toolbar { display: flex; gap: 10px; align-items: center; padding: 8px 12px; background: var(--accent); color: #fff; }
  .brand { margin-right: 8px; }
  .toolbar button { background: rgba(255,255,255,0.15); color: #fff; border: none; padding: 6px 12px; border-radius: 6px; cursor: pointer; }
  .toolbar button:hover { background: rgba(255,255,255,0.28); }
  .hint { opacity: 0.85; font-size: 12px; }
  .theme { margin-left: auto; }
</style>
