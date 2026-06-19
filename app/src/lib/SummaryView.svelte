<script lang="ts">
  import { selectedResult } from "./stores";
  import { readPaymentSummary } from "./api";
  import type { PaymentSummary } from "./types";

  let summary: PaymentSummary | null = null;
  let error = "";
  let loadedPath = "";

  $: void load($selectedResult?.path);

  async function load(path: string | undefined) {
    if (!path) {
      summary = null;
      error = "";
      loadedPath = "";
      return;
    }
    if (path === loadedPath) return;
    loadedPath = path;
    error = "";
    summary = null;
    try {
      summary = await readPaymentSummary(path);
    } catch {
      summary = null;
      error = "Datei konnte nicht als XML gelesen werden.";
    }
  }
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">Keine Datei ausgewählt.</p>
  {:else if error}
    <p class="muted">{error}</p>
  {:else if !summary}
    <p class="muted">Lädt…</p>
  {:else}
    <h3>
      {summary.pmtInfCount} PmtInf-{summary.pmtInfCount === 1 ? "Block" : "Blöcke"}{summary.messageType
        ? ` · ${summary.messageType}`
        : ""}
    </h3>
    {#if summary.blocks.length === 0}
      <p class="muted">Keine Zahlungsblöcke in dieser Datei.</p>
    {:else}
      <table>
        <thead>
          <tr><th>#</th><th>NbOfTxs</th><th>CtrlSum</th><th>SvcLvl/Cd</th><th>Datum</th></tr>
        </thead>
        <tbody>
          {#each summary.blocks as b, i}
            <tr>
              <td>{i + 1}</td>
              <td>{b.nbOfTxs ?? "—"}</td>
              <td>{b.ctrlSum ?? "—"}</td>
              <td>{b.svcLvlCd ?? "—"}</td>
              <td>{b.reqdDate ?? "—"}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <h3>Verwendungszwecke (Ustrd)</h3>
    {#if summary.ustrd.length === 0}
      <p class="muted">Keine Verwendungszwecke.</p>
    {:else}
      <ol class="ustrd">
        {#each summary.ustrd as u}
          <li>{u}</li>
        {/each}
      </ol>
    {/if}
  {/if}
</div>

<style>
  .summary { padding: 10px 14px; }
  .summary h3 { font-size: 13px; margin: 14px 0 6px; }
  .summary h3:first-child { margin-top: 0; }
  .muted { opacity: 0.7; font-style: italic; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: 4px 8px; border-bottom: 1px solid var(--border); }
  th { font-weight: 600; }
  ol.ustrd { margin: 0; padding-left: 22px; font-size: 13px; }
  ol.ustrd li { padding: 2px 0; word-break: break-word; }
</style>
