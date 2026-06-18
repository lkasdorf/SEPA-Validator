<script lang="ts">
  import { selectedResult, logFilter, search, jumpToLine } from "./stores";
  import { statusLabel } from "./types";
  import type { Message } from "./types";

  $: msgs = filterMsgs($selectedResult?.messages ?? [], $logFilter, $search);

  function filterMsgs(all: Message[], filter: string, q: string): Message[] {
    const term = q.trim().toLowerCase();
    return all.filter((m) => {
      if (filter === "errors" && m.severity !== "error") return false;
      if (filter === "warnings" && m.severity !== "warning") return false;
      if (term && !m.text.toLowerCase().includes(term)) return false;
      return true;
    });
  }
  function click(m: Message) {
    if (m.line) $jumpToLine(m.line);
  }
</script>

<div class="logpanel">
  <header>
    {#if $selectedResult}
      <div class="status {$selectedResult.status}">{statusLabel($selectedResult)}</div>
      <div class="meta">{$selectedResult.schema || "—"}</div>
    {:else}
      <div class="meta">Select a file to see its log.</div>
    {/if}
    <div class="controls">
      <input placeholder="Search…" bind:value={$search} />
      <div class="filters">
        <button class:active={$logFilter === "errors"} on:click={() => logFilter.set("errors")}>Errors</button>
        <button class:active={$logFilter === "warnings"} on:click={() => logFilter.set("warnings")}>Warnings</button>
        <button class:active={$logFilter === "all"} on:click={() => logFilter.set("all")}>All</button>
      </div>
    </div>
  </header>

  <ul>
    {#each msgs as m}
      <li class={m.severity} class:clickable={!!m.line} on:click={() => click(m)}>
        <span class="badge">{m.severity === "error" ? "ERROR" : "WARN"}</span>
        <span class="text">{m.text}</span>
        {#if m.line}<span class="loc">L{m.line}{m.column ? `:${m.column}` : ""}</span>{/if}
      </li>
    {/each}
    {#if $selectedResult && msgs.length === 0}
      <li class="none">No messages match.</li>
    {/if}
  </ul>
</div>

<style>
  .logpanel { display: grid; grid-template-rows: auto 1fr; height: 100%; }
  header { padding: 8px 10px; border-bottom: 1px solid var(--border); display: grid; gap: 6px; }
  .status { font-weight: 700; }
  .status.ok { color: var(--ok); } .status.invalid, .status.error { color: var(--err); }
  .status.warnings, .status.no_schema { color: var(--warn); }
  .meta { color: #888; font-size: 12px; }
  .controls { display: flex; gap: 8px; align-items: center; }
  .controls input { flex: 1; padding: 4px 8px; background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: 6px; }
  .filters button { border: 1px solid var(--border); background: var(--panel); color: var(--fg); padding: 3px 8px; border-radius: 6px; cursor: pointer; }
  .filters button.active { background: var(--accent); color: #fff; border-color: var(--accent); }
  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
  li { display: flex; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); align-items: baseline; }
  li.clickable { cursor: pointer; }
  li.clickable:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .badge { font-size: 11px; font-weight: 700; padding: 1px 5px; border-radius: 4px; }
  li.error .badge { background: var(--err); color: #fff; }
  li.warning .badge { background: var(--warn); color: #fff; }
  .loc { margin-left: auto; color: #888; font-variant-numeric: tabular-nums; }
  .none { color: #888; }
</style>
