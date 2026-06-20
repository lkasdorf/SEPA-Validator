<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl, schemaStatus } from "./api";
  import { aboutDialogOpen, aboutTab, updateDialogOpen, type AboutTab } from "./stores";

  const REPO = "https://github.com/lkasdorf/SEPA-Validator";

  let open = false;
  let copied = false;
  let copyTimer: ReturnType<typeof setTimeout> | undefined;

  function close() { open = false; copied = false; }
  function toggle() { open = !open; if (!open) copied = false; }

  function showAbout(tab: AboutTab) {
    aboutTab.set(tab);
    aboutDialogOpen.set(true);
    close();
  }

  function link(url: string) {
    void openUrl(url).catch(() => {});
    close();
  }

  function checkUpdates() {
    updateDialogOpen.set(true);
    close();
  }

  async function copyDiagnostics() {
    let version = "?";
    try { version = await getVersion(); } catch { /* keep ? */ }
    let schemas = "?";
    try {
      const s = await schemaStatus();
      schemas = `${s.filter((x) => x.present).length}/${s.length}`;
    } catch { /* keep ? */ }
    const ua = navigator.userAgent;
    const webview = (ua.match(/Edg\/([\d.]+)/) ?? [])[1] ?? "unknown";
    const text = [
      `SEPA Validator ${version}`,
      `WebView2: ${webview}`,
      `Schemas present: ${schemas}`,
      `User agent: ${ua}`,
    ].join("\n");
    try {
      await navigator.clipboard.writeText(text);
      copied = true;
      clearTimeout(copyTimer);
      copyTimer = setTimeout(() => (copied = false), 1800);
    } catch { /* clipboard unavailable */ }
  }
</script>

<svelte:window on:keydown={(e) => { if (open && e.key === "Escape") close(); }} />

<div class="menu">
  <button class="btn btn--ghost" on:click={toggle} aria-haspopup="menu" aria-expanded={open} aria-label="Menu" title="Menu">☰</button>
  {#if open}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div class="catcher" on:click={close}></div>
    <div class="dropdown" role="menu">
      <button role="menuitem" on:click={() => showAbout("about")}>About SEPA Validator</button>
      <button role="menuitem" on:click={checkUpdates}>Check for Updates…</button>
      <div class="sep"></div>
      <button role="menuitem" on:click={() => showAbout("shortcuts")}>Keyboard Shortcuts</button>
      <button role="menuitem" on:click={() => link(`${REPO}#readme`)}>Documentation</button>
      <button role="menuitem" on:click={() => link(`${REPO}/blob/master/CHANGELOG.md`)}>Changelog</button>
      <button role="menuitem" on:click={() => link(`${REPO}/issues/new`)}>Report an Issue…</button>
      <div class="sep"></div>
      <button role="menuitem" on:click={copyDiagnostics}>{copied ? "✓ Copied to clipboard" : "Copy Diagnostics"}</button>
      <button role="menuitem" on:click={() => showAbout("licenses")}>Licenses</button>
      <button role="menuitem" on:click={() => showAbout("privacy")}>Privacy</button>
    </div>
  {/if}
</div>

<style>
  .menu { position: relative; display: inline-flex; }
  .catcher { position: fixed; inset: 0; z-index: 40; }
  .dropdown {
    position: absolute; top: calc(100% + 6px); right: 0; z-index: 41;
    min-width: 220px; padding: var(--sp-1);
    background: var(--panel); color: var(--fg);
    border: 1px solid var(--border); border-radius: var(--radius);
    box-shadow: 0 8px 28px rgba(0, 0, 0, 0.22);
    display: flex; flex-direction: column;
  }
  .dropdown button {
    text-align: left; background: transparent; color: var(--fg);
    border: none; border-radius: 4px; cursor: pointer;
    padding: var(--sp-2) var(--sp-2); font: inherit; font-size: 13px;
  }
  .dropdown button:hover { background: color-mix(in srgb, var(--fg) 7%, transparent); }
  .sep { height: 1px; margin: var(--sp-1) 0; background: var(--border); }
</style>
