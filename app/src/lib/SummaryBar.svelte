<script lang="ts">
  import { summary, progress } from "./stores";
</script>

<footer class="summary">
  {#if $progress.running}
    <div class="bar"><div class="fill" style="width:{$progress.total ? ($progress.done / $progress.total) * 100 : 0}%"></div></div>
    <span>Validating {$progress.done}/{$progress.total}…</span>
  {:else}
    <span>{$summary.total} files</span>
    <span class="ok">OK {$summary.ok}</span>
    <span class="err">Invalid {$summary.invalid}</span>
    <span class="warn">Warnings {$summary.warnings}</span>
    <span class="warn">No schema {$summary.noSchema}</span>
  {/if}
</footer>

<style>
  .summary { display: flex; gap: var(--sp-3); align-items: center; padding: var(--sp-2) var(--sp-3); border-top: 1px solid var(--border); background: var(--chrome); font-size: 13px; font-variant-numeric: tabular-nums; }
  .ok { color: var(--ok); } .err { color: var(--err); } .warn { color: var(--warn); }
  .bar { flex: 0 0 220px; height: 8px; background: var(--bg); border-radius: 4px; overflow: hidden; }
  .fill { height: 100%; background: var(--accent); transition: width 120ms; }
</style>
