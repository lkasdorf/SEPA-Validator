<script lang="ts">
  import { selectedResult } from "./stores";
  import { paymentSummary } from "./paymentSummary";
  import { exportTransactionsCsv } from "./export";
  import { isDirectDebit } from "./types";
  import { visibleRange } from "./virtualRows";
  import { filterIndices, searchIndex } from "./transactionFilter";
  import { sortIndices, type SortDir, type SortKey } from "./transactionSort";
  import { formatSums, sumAmounts } from "./amounts";

  // Rows have a fixed height and only the visible window (plus overscan) is
  // rendered, so files with tens of thousands of transactions stay responsive.
  const ROW = 26;

  $: ps = $paymentSummary;
  $: ready = !!$selectedResult && ps.path === $selectedResult.path && ps.state === "ready" && !!ps.data;
  $: rows = ready ? ps.data!.transactions : [];
  $: dd = ready ? isDirectDebit(ps.data!) : false;

  // Sorting reorders all rows once per click; the search then keeps that
  // order. `order` holds the indices of the rows to show.
  let sortKey: SortKey = "index";
  let sortDir: SortDir = "asc";
  $: sorted = sortIndices(rows, sortKey, sortDir);

  // Search: the index is built once per file.
  let query = "";
  $: index = searchIndex(rows);
  $: order = filterIndices(index, query, sorted);
  $: filtered = query.trim() !== "";

  // Exact sums per currency: of the whole file and of what is shown.
  $: total = sumAmounts(rows, rows.keys());
  $: shown = filtered ? sumAmounts(rows, order) : total;

  $: columns = [
    { key: "index", label: "#" },
    { key: "name", label: "Name" },
    { key: "iban", label: "IBAN" },
    { key: "amount", label: "Amount", num: true },
    { key: "ustrd", label: "Remittance info" },
    ...(dd
      ? [
          { key: "mandateId", label: "Mandate ID" },
          { key: "signatureDate", label: "Signature date" },
        ]
      : []),
  ] as { key: SortKey; label: string; num?: boolean }[];

  /** Click cycles ascending → descending → file order. */
  function sortBy(key: SortKey) {
    if (key !== sortKey) [sortKey, sortDir] = [key, "asc"];
    else if (sortDir === "asc") sortDir = "desc";
    else [sortKey, sortDir] = ["index", "asc"];
    toTop();
  }

  let toolbarHeight = 40;

  let scroller: HTMLDivElement | undefined;
  let scrollTop = 0;
  let viewHeight = 600;
  $: range = visibleRange(scrollTop, viewHeight, ROW, order.length);

  function toTop() {
    scrollTop = 0;
    if (scroller) scroller.scrollTop = 0;
  }

  // A new file starts at the top.
  let shownPath = "";
  $: if (ps.path !== shownPath) {
    shownPath = ps.path;
    toTop();
  }

  function onSearchKey(e: KeyboardEvent) {
    if (e.key === "Escape" && query) {
      query = "";
      toTop();
    }
  }

  async function doExport() {
    if (!ready || !order.length) return; // still showing another file's data
    try {
      // Exports what is shown, in the shown order, with each row's position in the file.
      await exportTransactionsCsv(order.map((i) => rows[i]), dd, $selectedResult?.file ?? "file", order.map((i) => i + 1));
    } catch (e) {
      console.error("Transactions export failed:", e);
    }
  }
</script>

<div class="summary" style="--tx-toolbar-h: {toolbarHeight}px" bind:this={scroller} bind:clientHeight={viewHeight} on:scroll={() => (scrollTop = scroller?.scrollTop ?? 0)}>
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
      <div class="tx-toolbar" bind:clientHeight={toolbarHeight}>
        <input
          class="search"
          type="search"
          placeholder={dd ? "Search name, IBAN, remittance info, mandate ID" : "Search name, IBAN, remittance info"}
          aria-label="Search transactions"
          bind:value={query}
          on:input={toTop}
          on:keydown={onSearchKey}
        />
        <button class="btn btn--primary" on:click={doExport} disabled={!order.length}>Export CSV</button>
        <span class="count">
          {#if filtered}{order.length} of {rows.length}{:else}{rows.length}{/if}
          {rows.length === 1 ? "transaction" : "transactions"}
        </span>
        <span class="sum">
          Sum: <strong class="mono">{formatSums(shown.sums)}</strong>
          {#if filtered}<span class="count">of <span class="mono">{formatSums(total.sums)}</span></span>{/if}
          {#if shown.skipped > 0}<span class="count">({shown.skipped} without valid amount)</span>{/if}
        </span>
      </div>
      <table class:dd>
        <colgroup>
          <col class="c-no" /><col class="c-name" /><col class="c-iban" /><col class="c-amt" /><col class="c-rmt" />
          {#if dd}<col class="c-mndt" /><col class="c-date" />{/if}
        </colgroup>
        <thead>
          <tr>
            {#each columns as c (c.key)}
              {@const active = sortKey === c.key && !(c.key === "index" && sortDir === "asc")}
              <th class:num={c.num} aria-sort={sortKey === c.key ? (sortDir === "asc" ? "ascending" : "descending") : "none"}>
                <button class="sort" class:active on:click={() => sortBy(c.key)} title="Sort by {c.label}">
                  {c.label}<span class="arrow" aria-hidden="true">{active ? (sortDir === "asc" ? "▲" : "▼") : ""}</span>
                </button>
              </th>
            {/each}
          </tr>
        </thead>
        <tbody>
          {#if range.start > 0}<tr class="spacer" style="height: {range.start * ROW}px"><td colspan={dd ? 7 : 5}></td></tr>{/if}
          {#each order.slice(range.start, range.end) as i (i)}
            {@const t = rows[i]}
            {@const rmt = t.ustrd?.replace(/\r?\n/g, " ")}
            <tr style="height: {ROW}px">
              <td>{i + 1}</td>
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
          {#if range.end < order.length}<tr class="spacer" style="height: {(order.length - range.end) * ROW}px"><td colspan={dd ? 7 : 5}></td></tr>{/if}
        </tbody>
      </table>
      {#if filtered && order.length === 0}
        <p class="muted">No transactions match “{query.trim()}”.</p>
      {/if}
    {/if}
  {:else}
    <p class="muted">Loading…</p>
  {/if}
</div>

<style>
  .summary { padding: var(--sp-3) var(--sp-4); }
  .muted { color: var(--muted); font-style: italic; }
  /* Toolbar and table header stay visible while scrolling; the header sits right below the toolbar. */
  .tx-toolbar {
    display: flex; flex-wrap: wrap; align-items: center; gap: var(--sp-2) var(--sp-3);
    min-height: 40px; padding: var(--sp-1) 0; box-sizing: border-box;
    position: sticky; top: 0; left: 0; z-index: 2; background: var(--panel);
  }
  .count { font-size: 12px; color: var(--muted); }
  .search {
    flex: 0 1 320px; min-width: 160px; font: inherit; font-size: 12px; color: var(--fg);
    background: var(--panel); border: 1px solid var(--border); border-radius: var(--radius);
    padding: var(--sp-1) var(--sp-2);
  }
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
  th { font-weight: 600; position: sticky; top: var(--tx-toolbar-h); background: var(--panel); height: 28px; z-index: 1; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .sum { font-size: 12px; display: inline-flex; gap: var(--sp-1); align-items: baseline; flex-wrap: wrap; }
  .sort {
    all: unset; cursor: pointer; display: inline-flex; align-items: center; gap: 2px;
    max-width: 100%; overflow: hidden; text-overflow: ellipsis;
  }
  .sort:hover, .sort.active { color: var(--accent); }
  .sort:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
  th.num .sort { flex-direction: row-reverse; }
  .arrow { font-size: 9px; width: 9px; }
  td.missing { color: var(--muted); }
  tr.spacer td, tr.spacer { border: 0; padding: 0; }
</style>
