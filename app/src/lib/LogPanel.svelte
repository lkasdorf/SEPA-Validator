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
      if (term && !`${m.text} ${m.hint ?? ""}`.toLowerCase().includes(term)) return false;
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
      <div class="meta mono">{$selectedResult.schema || "—"}</div>
    {:else}
      <div class="meta">Select a file to see its log.</div>
    {/if}
    <div class="controls">
      <input placeholder="Search…" aria-label="Search messages" bind:value={$search} />
      <div class="seg filters">
        <button class:active={$logFilter === "errors"} on:click={() => logFilter.set("errors")}>Errors</button>
        <button class:active={$logFilter === "warnings"} on:click={() => logFilter.set("warnings")}>Warnings</button>
        <button class:active={$logFilter === "all"} on:click={() => logFilter.set("all")}>All</button>
      </div>
    </div>
  </header>

  <ul>
    {#each msgs as m}
      {#if m.line}
        <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
        <li class="{m.severity} clickable" on:click={() => click(m)}
            on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); click(m); } }}
            role="button" tabindex="0" title="Show line {m.line} in the XML">
          <span class="badge">{m.severity === "error" ? "ERROR" : "WARN"}</span>
          <span class="body">
            <span class="text">{m.text}</span>
            {#if m.hint}<span class="hint">{m.hint}</span>{/if}
          </span>
          <span class="loc mono">L{m.line}{m.column ? `:${m.column}` : ""}</span>
        </li>
      {:else}
        <li class={m.severity}>
          <span class="badge">{m.severity === "error" ? "ERROR" : "WARN"}</span>
          <span class="body">
            <span class="text">{m.text}</span>
            {#if m.hint}<span class="hint">{m.hint}</span>{/if}
          </span>
        </li>
      {/if}
    {/each}
    {#if $selectedResult && msgs.length === 0}
      <li class="none">
        {#if $selectedResult.messages.length === 0 && $selectedResult.status === "ok"}
          No problems found: the file is valid against {$selectedResult.schema}.
        {:else}
          No matches.
        {/if}
      </li>
    {/if}
  </ul>
</div>

<style>
  .logpanel { display: grid; grid-template-rows: auto 1fr; height: 100%; min-width: 0; }
  header { padding: 8px 10px; border-bottom: 1px solid var(--border); display: grid; gap: 6px; min-width: 0; }
  .status { font-weight: 700; }
  .status.ok { color: var(--ok); } .status.invalid, .status.error { color: var(--err); }
  .status.warnings, .status.no_schema { color: var(--warn); }
  .meta { color: var(--muted); font-size: 12px; overflow-wrap: anywhere; }
  .controls { display: flex; flex-wrap: wrap; gap: var(--sp-2); align-items: center; min-width: 0; }
  .controls input { flex: 1 1 8em; min-width: 0; padding: var(--sp-1) var(--sp-2); background: var(--bg); color: var(--fg); border: 1px solid var(--border); border-radius: var(--radius); }
  ul { list-style: none; margin: 0; padding: 0; overflow-y: auto; overflow-x: hidden; }
  li { display: flex; gap: 8px; padding: 8px 10px; border-bottom: 1px solid var(--border); align-items: baseline; }
  li.clickable { cursor: pointer; }
  li.clickable:hover { background: color-mix(in srgb, var(--accent) 10%, transparent); }
  .badge { flex: none; font-size: 11px; font-weight: 700; padding: 1px 5px; border-radius: 4px; }
  li.error .badge { background: var(--err); color: var(--err-fg); }
  li.warning .badge { background: var(--warn); color: var(--warn-fg); }
  .body { flex: 1; min-width: 0; display: grid; gap: 2px; }
  .text { overflow-wrap: anywhere; }
  .hint { color: var(--muted); font-size: 12px; overflow-wrap: anywhere; }
  .loc { flex: none; margin-left: auto; color: var(--muted); white-space: nowrap; }
  .none { color: var(--muted); }
</style>
