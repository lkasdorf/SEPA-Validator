<script lang="ts">
  import { schemaCounts, schemaDialogOpen, dragging } from "./stores";
  import { pickFiles, pickFolder } from "./validation";

  $: missing = $schemaCounts.total - $schemaCounts.present;
</script>

<div class="empty" class:dragging={$dragging}>
  <div class="card">
    <svg class="icon" viewBox="0 0 24 24" aria-hidden="true">
      <path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z" />
      <path d="M14 3v5h5M9.5 13.5l-1.5 1.5 1.5 1.5M14.5 13.5l1.5 1.5-1.5 1.5" />
    </svg>
    <h2>Drop SEPA XML files or a folder here</h2>
    <p>They are checked against the ISO 20022 / SPS schemas. Everything stays on this computer.</p>
    <div class="actions">
      <button class="btn btn--primary" on:click={pickFiles}>Select Files…</button>
      <button class="btn btn--ghost" on:click={pickFolder}>Select Folder…</button>
    </div>
    {#if $schemaCounts.total > 0 && missing > 0}
      <p class="schemas">
        {$schemaCounts.present} of {$schemaCounts.total} schemas imported. Files whose schema is missing can't be checked.
        <button class="link" on:click={() => schemaDialogOpen.set(true)}>Import schemas…</button>
      </p>
    {/if}
  </div>
</div>

<style>
  .empty { flex: 1 1 auto; min-height: 0; display: grid; place-items: center; padding: var(--sp-4); background: var(--panel); }
  .card {
    max-width: 440px; text-align: center; padding: 32px 28px;
    border: 2px dashed var(--border); border-radius: 12px;
    transition: border-color .12s ease, background-color .12s ease;
  }
  .dragging .card { border-color: var(--accent); background: var(--sel); }
  .icon { width: 40px; height: 40px; fill: none; stroke: var(--accent); stroke-width: 1.6; stroke-linecap: round; stroke-linejoin: round; }
  h2 { margin: var(--sp-2) 0 var(--sp-1); font-size: 16px; font-weight: 600; }
  p { margin: 0 0 var(--sp-3); color: var(--muted); font-size: 13px; line-height: 1.5; }
  .actions { display: flex; gap: var(--sp-2); justify-content: center; }
  .schemas { margin: var(--sp-4) 0 0; color: var(--warn); }
  .link { font: inherit; background: none; border: none; padding: 0; color: var(--accent); text-decoration: underline; cursor: pointer; }
</style>
