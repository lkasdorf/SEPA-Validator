<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  $: ps = $paymentSummary;
</script>

<div class="summary">
  {#if !$selectedResult}
    <p class="muted">No file selected.</p>
  {:else if ps.state === "error"}
    <p class="muted">Could not read file as XML.</p>
  {:else if ps.state === "ready" && ps.data}
    {@const data = ps.data}
    {#if data.creditor}
      <h3>Creditor (Cdtr)</h3>
      <dl class="cdtr">
        <dt>Name</dt><dd>{data.creditor.name ?? "—"}</dd>
        <dt>IBAN</dt><dd class="mono">{data.creditor.iban ?? "—"}</dd>
        <dt>BIC</dt><dd class="mono">{data.creditor.bic ?? "—"}</dd>
        <dt>Creditor ID</dt><dd class="mono">{data.creditor.creditorId ?? "—"}</dd>
      </dl>
    {/if}
    <h3>
      {data.pmtInfCount} PmtInf {data.pmtInfCount === 1 ? "block" : "blocks"}{data.messageType
        ? ` · ${data.messageType}`
        : ""}
    </h3>
    {#if data.blocks.length === 0}
      <p class="muted">No payment blocks in this file.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>#</th><th>NbOfTxs</th><th>CtrlSum</th><th>SvcLvl/Cd</th>
            <th>LclInstrm</th><th>SeqTp</th><th>Date</th>
          </tr>
        </thead>
        <tbody>
          {#each data.blocks as b, i}
            <tr>
              <td>{i + 1}</td>
              <td>{b.nbOfTxs ?? "—"}</td>
              <td>{b.ctrlSum ?? "—"}</td>
              <td>{b.svcLvlCd ?? "—"}</td>
              <td>{b.lclInstrm ?? "—"}</td>
              <td>{b.seqTp ?? "—"}</td>
              <td>{b.reqdDate ?? "—"}</td>
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
  .summary h3 { font-size: 13px; margin: var(--sp-4) 0 var(--sp-1); }
  .summary h3:first-child { margin-top: 0; }
  .muted { color: var(--muted); font-style: italic; }
  table { border-collapse: collapse; width: 100%; font-size: 12px; }
  th, td { text-align: left; padding: var(--sp-1) var(--sp-2); border-bottom: 1px solid var(--border); }
  th { font-weight: 600; }
  tbody td { font-family: var(--font-mono); font-variant-numeric: tabular-nums; }
  dl.cdtr { display: grid; grid-template-columns: max-content 1fr; gap: 2px 12px; margin: 0 0 6px; font-size: 12px; }
  dl.cdtr dt { font-weight: 600; }
  dl.cdtr dd { margin: 0; word-break: break-word; }
</style>
