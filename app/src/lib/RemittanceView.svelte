<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  import { exportRemittanceCsv } from "./export";
  $: ps = $paymentSummary;

  async function doExport() {
    const tx = ps.data?.transactions ?? [];
    if (!tx.length) return;
    try {
      await exportRemittanceCsv(tx, $selectedResult?.file ?? "datei");
    } catch (e) {
      console.error("Verwendungszweck-Export fehlgeschlagen:", e);
    }
  }
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
      <div class="rmt-toolbar">
        <button on:click={doExport}>Export CSV</button>
      </div>
      <table>
        <thead><tr><th>#</th><th>Herkunft</th><th>Verwendungszweck</th></tr></thead>
        <tbody>
          {#each data.transactions as t, i}
            <tr>
              <td>{i + 1}</td>
              <td>{t.instrId ?? t.endToEndId ?? "—"}</td>
              <td class={t.ustrd == null ? "missing" : ""}>{t.ustrd ?? "⚠ kein Verwendungszweck"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
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
  .rmt-toolbar { margin: 0 0 8px; }
  .rmt-toolbar button { background: var(--accent); color: #fff; border: none; padding: 4px 10px; border-radius: 6px; cursor: pointer; font-size: 12px; }
  .rmt-toolbar button:hover { filter: brightness(1.1); }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); vertical-align: top; }
  th { font-weight: 600; }
  td { word-break: break-word; white-space: pre-wrap; }
  td.missing { color: var(--err); font-style: italic; }
</style>
