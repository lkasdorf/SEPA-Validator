<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { startValidation, schemaStatus } from "./api";
  import { results, selectedIndex, progress, theme, schemaDialogOpen } from "./stores";
  import type { ValidationEvent } from "./types";
  import { exportTxt, exportCsv } from "./export";
  import { get } from "svelte/store";
  import Menu from "./Menu.svelte";
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
  <button class="btn btn--primary" on:click={pickFiles}>Select Files…</button>
  <button class="btn btn--ghost" on:click={pickFolder}>Select Folder…</button>
  <button class="btn btn--ghost" on:click={doExportTxt} disabled={$results.length === 0}>Export TXT</button>
  <button class="btn btn--ghost" on:click={doExportCsv} disabled={$results.length === 0}>Export CSV</button>
  <button class="btn btn--ghost" on:click={() => schemaDialogOpen.set(true)}>Schemas… {schemaTotal ? `(${schemaPresent}/${schemaTotal})` : ""}</button>
  <span class="hint">or drag &amp; drop files here</span>
  <div class="right">
    <Menu />
    <button class="btn btn--ghost" on:click={toggleTheme} title="Toggle theme" aria-label="Toggle theme">◐</button>
  </div>
</header>

<style>
  .toolbar { display: flex; gap: var(--sp-2); align-items: center; padding: var(--sp-2) var(--sp-3); background: var(--chrome); color: var(--fg); border-bottom: 1px solid var(--border); }
  .brand { margin-right: var(--sp-2); padding-left: var(--sp-2); font-weight: 600; letter-spacing: .01em; border-left: 3px solid var(--accent); }
  .hint { color: var(--muted); font-size: 12px; }
  .right { margin-left: auto; display: flex; gap: var(--sp-2); align-items: center; }
  .right button { font-size: 14px; }
</style>
