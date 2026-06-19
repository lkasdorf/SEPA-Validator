<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  $: ps = $paymentSummary;
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">Keine Datei ausgewählt.</p>
  {:else if ps.state === "error"}
    <p class="muted">Datei konnte nicht als XML gelesen werden.</p>
  {:else if ps.state === "ready" && ps.data}
    {@const data = ps.data}
    {@const total = data.transactions.length}
    {@const missing = data.transactions.filter((t) => t.ustrd == null).length}
    {#if total === 0}
      <p class="muted">Keine Transaktionen in dieser Datei.</p>
    {:else}
      {#if missing > 0}
        <p class="warn-banner">⚠ {missing} von {total} Transaktionen ohne Verwendungszweck</p>
      {/if}
      <ol class="ustrd">
        {#each data.transactions as t}
          {#if t.ustrd == null}
            <li class="missing">⚠ kein Verwendungszweck</li>
          {:else}
            <li>{t.ustrd}</li>
          {/if}
        {/each}
      </ol>
    {/if}
  {:else}
    <p class="muted">Lädt…</p>
  {/if}
</div>

<style>
  .summary { padding: 10px 14px; }
  .muted { opacity: 0.7; font-style: italic; }
  .warn-banner {
    background: rgba(196, 39, 28, 0.12);
    color: var(--err);
    border: 1px solid var(--err);
    border-radius: 6px;
    padding: 6px 10px;
    font-size: 12px;
    margin: 0 0 10px;
  }
  ol.ustrd { margin: 0; padding-left: 22px; font-size: 13px; }
  ol.ustrd li { padding: 2px 0; word-break: break-word; white-space: pre-wrap; }
  ol.ustrd li.missing { color: var(--err); font-style: italic; }
</style>
