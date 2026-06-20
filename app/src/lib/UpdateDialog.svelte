<script lang="ts">
  import { onMount } from "svelte";
  import { check, type Update } from "@tauri-apps/plugin-updater";
  import { relaunch } from "@tauri-apps/plugin-process";
  import { updateDialogOpen } from "./stores";

  type Status = "checking" | "available" | "none" | "downloading" | "error";
  let status: Status = "checking";
  let version = "";
  let notes = "";
  let error = "";
  let received = 0;
  let total = 0;
  let update: Update | null = null;

  onMount(runCheck);

  async function runCheck() {
    status = "checking";
    error = "";
    try {
      update = await check();
      if (update) {
        version = update.version;
        notes = update.body ?? "";
        status = "available";
      } else {
        status = "none";
      }
    } catch (e) {
      status = "error";
      error = e instanceof Error ? e.message : String(e);
    }
  }

  async function install() {
    if (!update) return;
    status = "downloading";
    received = 0;
    total = 0;
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") received += ev.data.chunkLength;
      });
      await relaunch();
    } catch (e) {
      status = "error";
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function close() { updateDialogOpen.set(false); }

  $: pct = total > 0 ? Math.min(100, Math.round((received / total) * 100)) : 0;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={() => { if (status !== "downloading") close(); }}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Software update">
    <header>
      <strong>Software update</strong>
      {#if status !== "downloading"}
        <button class="x" on:click={close} aria-label="Close">✕</button>
      {/if}
    </header>

    <div class="content">
      {#if status === "checking"}
        <p>Checking for updates…</p>
      {:else if status === "none"}
        <p>You're on the latest version.</p>
      {:else if status === "available"}
        <p>Version <span class="mono">{version}</span> is available.</p>
        {#if notes}<pre class="notes">{notes}</pre>{/if}
      {:else if status === "downloading"}
        <p>Downloading and installing…</p>
        <div class="bar"><div class="fill" style="width:{pct}%"></div></div>
        <p class="muted">{pct}%{total ? "" : " — the app will restart when finished"}</p>
      {:else}
        <p>Couldn't check for updates.</p>
        <p class="muted err">{error}</p>
      {/if}
    </div>

    <footer>
      {#if status === "available"}
        <button class="btn btn--primary" on:click={install}>Install &amp; Restart</button>
        <button class="btn btn--ghost close" on:click={close}>Later</button>
      {:else if status === "error"}
        <button class="btn btn--ghost" on:click={runCheck}>Retry</button>
        <button class="btn btn--ghost close" on:click={close}>Close</button>
      {:else if status !== "downloading"}
        <button class="btn btn--ghost close" on:click={close}>Close</button>
      {/if}
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
    width: min(440px, 92vw); max-height: 86vh;
    display: flex; flex-direction: column;
    padding: var(--sp-3) var(--sp-4);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .content { margin: var(--sp-3) 0; font-size: 13px; line-height: 1.5; }
  .content p { margin: 0 0 var(--sp-2); }
  .muted { color: var(--muted); }
  .err { white-space: pre-wrap; word-break: break-word; font-size: 12px; }
  .notes { margin: var(--sp-2) 0 0; padding: var(--sp-2); background: var(--bg); border: 1px solid var(--border); border-radius: var(--radius); font-size: 12px; white-space: pre-wrap; max-height: 30vh; overflow: auto; }
  .bar { height: 8px; background: var(--bg); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 120ms; }
  footer { display: flex; gap: var(--sp-2); margin-top: var(--sp-2); }
  footer .close { margin-left: auto; }
</style>
