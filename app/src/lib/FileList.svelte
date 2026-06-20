<script lang="ts">
  import { results, selectedIndex } from "./stores";
  import { statusLabel } from "./types";
  import type { ValidationResult } from "./types";

  function icon(r: ValidationResult): string {
    switch (r.status) {
      case "ok": return "✓";
      case "invalid": case "error": return "✗";
      default: return "⚠";
    }
  }
  function cls(r: ValidationResult): string {
    if (r.status === "ok") return "ok";
    if (r.status === "invalid" || r.status === "error") return "err";
    return "warn";
  }
</script>

<ul class="filelist">
  {#each $results as r, i}
    <!-- svelte-ignore a11y_no_noninteractive_element_to_interactive_role -->
    <li class:selected={i === $selectedIndex} on:click={() => selectedIndex.set(i)}
        on:keydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); selectedIndex.set(i); } }}
        role="button" tabindex="0"
        title={statusLabel(r)}>
      <span class="icon {cls(r)}">{icon(r)}</span>
      <span class="name">{r.file}</span>
    </li>
  {/each}
  {#if $results.length === 0}
    <li class="empty">No files yet — add files or drag &amp; drop.</li>
  {/if}
</ul>

<style>
  .filelist { list-style: none; margin: 0; padding: 0; overflow-y: auto; height: 100%; }
  li { display: flex; gap: var(--sp-2); align-items: center; padding: var(--sp-2) var(--sp-3); cursor: pointer; border-bottom: 1px solid var(--border); }
  li.selected { background: var(--sel); }
  .icon { width: 1em; font-weight: 700; }
  .icon.ok { color: var(--ok); } .icon.err { color: var(--err); } .icon.warn { color: var(--warn); }
  .name { white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .empty { color: var(--muted); cursor: default; }
</style>
