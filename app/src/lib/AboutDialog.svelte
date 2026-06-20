<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "./api";
  import { aboutDialogOpen, aboutTab } from "./stores";

  const REPO = "https://github.com/lkasdorf/SEPA-Validator";

  let version = "";
  onMount(async () => {
    try { version = await getVersion(); } catch { version = "?"; }
  });

  function close() { aboutDialogOpen.set(false); }
  function link(url: string) { void openUrl(url).catch(() => {}); }

  const shortcuts: [string, string][] = [
    ["Ctrl + F", "Find in the XML viewer (step through matches)"],
    ["Click an error / warning", "Jump to its line in the viewer"],
    ["Enter / Space", "Open the focused file in the list"],
    ["Drag & drop", "Validate dropped files or folders"],
    ["Gutter arrows", "Fold / unfold individual XML blocks"],
  ];

  const licenses: [string, string][] = [
    ["Tauri", "MIT / Apache-2.0"],
    ["Svelte", "MIT"],
    ["CodeMirror 6", "MIT"],
    ["libxml2", "MIT"],
    ["Rust crates (quick-xml, zip, libxml, serde, log)", "MIT / Apache-2.0"],
  ];
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="About SEPA Validator">
    <header>
      <strong>SEPA Validator</strong>
      <button class="x" on:click={close} aria-label="Close">✕</button>
    </header>

    <div class="seg tabs">
      <button class:active={$aboutTab === "about"} on:click={() => aboutTab.set("about")}>About</button>
      <button class:active={$aboutTab === "shortcuts"} on:click={() => aboutTab.set("shortcuts")}>Shortcuts</button>
      <button class:active={$aboutTab === "licenses"} on:click={() => aboutTab.set("licenses")}>Licenses</button>
      <button class:active={$aboutTab === "privacy"} on:click={() => aboutTab.set("privacy")}>Privacy</button>
    </div>

    <div class="content">
      {#if $aboutTab === "about"}
        <p class="lead">Version <span class="mono">{version || "…"}</span></p>
        <p>Validates SEPA payment XML files against ISO 20022 XSD schemas. All validation runs locally on your machine.</p>
        <p>Built with Tauri, Rust and Svelte; XSD validation by libxml2.</p>
        <p class="links">
          <button class="linkbtn" on:click={() => link(REPO)}>GitHub repository</button>
          <span>·</span>
          <button class="linkbtn" on:click={() => link(`${REPO}/releases`)}>Releases</button>
        </p>
        <p class="muted">© 2026 Leon Kasdorf · MIT License</p>

      {:else if $aboutTab === "shortcuts"}
        <table>
          <tbody>
            {#each shortcuts as [key, what]}
              <tr><td class="mono key">{key}</td><td>{what}</td></tr>
            {/each}
          </tbody>
        </table>

      {:else if $aboutTab === "licenses"}
        <p>SEPA Validator is released under the <button class="linkbtn" on:click={() => link(`${REPO}/blob/master/LICENSE`)}>MIT License</button>.</p>
        <p class="muted">Open-source components:</p>
        <table>
          <tbody>
            {#each licenses as [name, lic]}
              <tr><td>{name}</td><td class="mono">{lic}</td></tr>
            {/each}
          </tbody>
        </table>
        <p class="note">The ISO 20022 / DK / GBIC <strong>XSD schemas are not bundled</strong> and are not covered by this license. They remain the property of their respective owners (e.g. ebics.de, iso20022.org) under their own terms — all rights reserved.</p>

      {:else}
        <p>All validation happens entirely on your machine. SEPA Validator does not upload, transmit, or otherwise send your XML files or their contents anywhere.</p>
        <p>The only network actions are ones you trigger yourself: opening external links (GitHub, ebics.de, iso20022.org) in your browser, and — when you use <em>Check for Updates</em> — asking GitHub for the latest released version.</p>
      {/if}
    </div>

    <footer>
      <button class="btn btn--ghost close" on:click={close}>Close</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 60;
    background: rgba(0, 0, 0, 0.45);
    display: flex; align-items: center; justify-content: center;
  }
  .dialog {
    background: var(--panel); color: var(--fg);
    border: 1px solid var(--border); border-radius: 8px;
    width: min(560px, 92vw); max-height: 86vh;
    display: flex; flex-direction: column;
    padding: var(--sp-3) var(--sp-4);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .tabs { margin: var(--sp-3) 0 var(--sp-3); }
  .content { overflow: auto; min-height: 0; font-size: 13px; line-height: 1.5; }
  .content p { margin: 0 0 var(--sp-2); }
  .lead { font-size: 14px; }
  .muted { color: var(--muted); }
  .links { display: flex; gap: var(--sp-2); align-items: center; }
  .linkbtn { background: none; border: none; padding: 0; color: var(--accent); cursor: pointer; font: inherit; text-decoration: underline; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  td { text-align: left; padding: var(--sp-1) var(--sp-2); border-bottom: 1px solid var(--border); vertical-align: top; }
  td.key { white-space: nowrap; }
  .note { margin-top: var(--sp-3); font-size: 12px; color: var(--muted); }
  footer { display: flex; margin-top: var(--sp-3); }
  footer .close { margin-left: auto; }
</style>
