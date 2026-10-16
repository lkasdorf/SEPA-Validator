<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  import { exportTransactionsCsv } from "./export";
  import { isDirectDebit } from "./types";
  import { visibleRange } from "./virtualRows";

  // Rows have a fixed height and only the visible window (plus overscan) is
  // rendered, so files with tens of thousands of transactions stay responsive.
  const ROW = 26;

  $: ps = $paymentSummary;
  $: ready = !!$selectedResult && ps.path === $selectedResult.path && ps.state === "ready" && !!ps.data;
  $: rows = ready ? ps.data!.transactions : [];
  $: dd = ready ? isDirectDebit(ps.data!) : false;

  let scroller: HTMLDivElement | undefined;
  let scrollTop = 0;
  let viewHeight = 600;
  $: range = visibleRange(scrollTop, viewHeight, ROW, rows.length);

  // A new file starts at the top.
  let shownPath = "";
  $: if (ps.path !== shownPath) {
    shownPath = ps.path;
    scrollTop = 0;
    if (scroller) scroller.scrollTop = 0;
  }

  async function doExport() {
    if (!ready || !rows.length) return; // still showing another file's data
    try {
      await exportTransactionsCsv(rows, dd, $selectedResult?.file ?? "file");
    } catch (e) {
      console.error("Transactions export failed:", e);
    }
  }
</script>

<div class="summary" bind:this={scroller} bind:clientHeight={viewHeight} on:scroll={() => (scrollTop = scroller?.scrollTop ?? 0)}>
  {#if !$selectedResult}
    <p class="muted">No file selected.</p>
  {:else if ps.path !== $selectedResult.path}
    <p class="muted">Loading…</p>
  {:else if ps.state === "error"}
    <p class="muted">Could not read file as XML.</p>
  {:else if ready}
    {#if rows.length === 0}
      <p class="muted">No transactions in this file.</p>
    {:else}
      <div class="tx-toolbar">
        <button class="btn btn--primary" on:click={doExport}>Export CSV</button>
        <span class="count">{rows.length} {rows.length === 1 ? "transaction" : "transactions"}</span>
      </div>
      <table class:dd>
        <colgroup>
          <col class="c-no" /><col class="c-name" /><col class="c-iban" /><col class="c-amt" /><col class="c-rmt" />
          {#if dd}<col class="c-mndt" /><col class="c-date" />{/if}
        </colgroup>
        <thead>
          <tr>
            <th>#</th><th>Name</th><th>IBAN</th><th class="num">Amount</th><th>Remittance info</th>
            {#if dd}<th>Mandate ID</th><th>Signature date</th>{/if}
          </tr>
        </thead>
        <tbody>
          {#if range.start > 0}<tr class="spacer" style="height: {range.start * ROW}px"><td colspan={dd ? 7 : 5}></td></tr>{/if}
          {#each rows.slice(range.start, range.end) as t, j (range.start + j)}
            {@const rmt = t.ustrd?.replace(/\r?\n/g, " ")}
            <tr style="height: {ROW}px">
              <td>{range.start + j + 1}</td>
              <td title={t.name ?? ""}>{t.name ?? "—"}</td>
              <td class="mono">{t.iban ?? "—"}</td>
              <td class="mono num">{t.amount != null ? `${t.amount} ${t.currency ?? ""}` : "—"}</td>
              <td title={rmt ?? ""} class:missing={rmt == null}>{rmt ?? "—"}</td>
              {#if dd}
                <td class="mono" title={t.mandateId ?? ""}>{t.mandateId ?? "—"}</td>
                <td class="mono">{t.signatureDate ?? "—"}</td>
              {/if}
            </tr>
          {/each}
          {#if range.end < rows.length}<tr class="spacer" style="height: {(rows.length - range.end) * ROW}px"><td colspan={dd ? 7 : 5}></td></tr>{/if}
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
  .tx-toolbar { display: flex; align-items: center; gap: var(--sp-3); margin: 0 0 var(--sp-2); }
  .count { font-size: 12px; color: var(--muted); }
  /* Fixed layout: column widths must not shift as different rows scroll into view. */
  table { border-collapse: collapse; width: 100%; min-width: 760px; table-layout: fixed; font-size: 12px; }
  table.dd { min-width: 980px; }
  .c-no { width: 52px; }
  .c-name { width: 22%; }
  .c-iban { width: 210px; }
  .c-amt { width: 120px; }
  .c-mndt { width: 160px; }
  .c-date { width: 104px; }
  th, td {
    text-align: left; padding: 0 var(--sp-2); border-bottom: 1px solid var(--border);
    white-space: nowrap; overflow: hidden; text-overflow: ellipsis; box-sizing: border-box;
  }
  th { font-weight: 600; position: sticky; top: 0; background: var(--panel); height: 28px; z-index: 1; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  td.missing { color: var(--muted); }
  tr.spacer td, tr.spacer { border: 0; padding: 0; }
</style>
