<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  import { exportRemittanceCsv } from "./export";
  $: ps = $paymentSummary;

  async function doExport() {
    const tx = ps.data?.transactions ?? [];
    if (!tx.length) return;
    try {
      await exportRemittanceCsv(tx, $selectedResult?.file ?? "file");
    } catch (e) {
      console.error("Remittance export failed:", e);
    }
  }
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">No file selected.</p>
  {:else if ps.state === "error"}
    <p class="muted">Could not read file as XML.</p>
  {:else if ps.state === "ready" && ps.data}
    {@const data = ps.data}
    {@const total = data.transactions.length}
    {@const missing = data.transactions.filter((t) => t.ustrd == null).length}
    {#if total === 0}
      <p class="muted">No transactions in this file.</p>
    {:else}
      {#if missing > 0}
        <p class="warn-banner">⚠ {missing} of {total} transactions without remittance info</p>
      {/if}
      <div class="rmt-toolbar">
        <button class="btn btn--primary" on:click={doExport}>Export CSV</button>
      </div>
      <table>
        <thead><tr><th>#</th><th>Origin</th><th>Remittance info</th></tr></thead>
        <tbody>
          {#each data.transactions as t, i}
            <tr>
              <td>{i + 1}</td>
              <td class="mono">{t.instrId ?? t.endToEndId ?? "—"}</td>
              <td class={t.ustrd == null ? "missing" : ""}>{t.ustrd ?? "⚠ no remittance info"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .summary { padding: var(--sp-3) var(--sp-4); }
  .muted { color: var(--muted); font-style: italic; }
  .warn-banner {
    background: color-mix(in srgb, var(--err) 12%, transparent);
    color: var(--err);
    border: 1px solid var(--err);
    border-radius: var(--radius);
    padding: var(--sp-2) var(--sp-3);
    font-size: 12px;
    margin: 0 0 var(--sp-3);
  }
  .rmt-toolbar { margin: 0 0 var(--sp-2); }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: var(--sp-1) var(--sp-2); border-bottom: 1px solid var(--border); vertical-align: top; }
  th { font-weight: 600; }
  td { word-break: break-word; white-space: pre-wrap; }
  td.missing { color: var(--err); font-style: italic; }
</style>
