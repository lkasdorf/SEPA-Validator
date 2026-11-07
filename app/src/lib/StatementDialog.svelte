<script lang="ts">
  import { onMount } from "svelte";
  import { previewStatements, generateStatements, schemaStatus } from "./api";
  import { progress, selectedResult, statementDialogOpen } from "./stores";
  import { STATEMENT_XSD, canGenerate, normalizeOpening, previewLine, statementSchemaMissing } from "./statementGen";
  import { appendValidation } from "./validation";
  import type { StatementPreview } from "./types";

  // The file the dialog was opened for; a later selection change doesn't retarget it.
  const source = $selectedResult?.path ?? "";
  const sourceName = $selectedResult?.file ?? "";

  let preview: StatementPreview | null = null;
  let opening = "10000.00";
  let busy = false;
  let error = "";
  let schemaMissing = false;

  $: normalized = normalizeOpening(opening);
  $: allowed = canGenerate({ preview, opening: normalized, busy, validating: $progress.running });

  onMount(async () => {
    try {
      preview = await previewStatements(source);
    } catch (e) {
      error = String(e);
    }
    try {
      schemaMissing = statementSchemaMissing(await schemaStatus());
    } catch {
      // No status: skip the hint; validation reports a missing schema anyway.
    }
  });

  function close() {
    statementDialogOpen.set(false);
  }

  async function generate() {
    if (!allowed || normalized === null) return;
    busy = true;
    error = "";
    try {
      const paths = await generateStatements(source, normalized);
      close();
      await appendValidation(paths);
    } catch (e) {
      error = String(e);
      busy = false;
    }
  }
</script>

<svelte:window on:keydown={(e) => { if (e.key === "Escape") close(); }} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Generate bank statement">
    <header>
      <strong>Generate bank statement</strong>
      <button class="x" on:click={close} aria-label="Close">✕</button>
    </header>
    <p class="hint">A camt.053 statement for <span class="mono">{sourceName}</span>, as if the bank had booked it: one statement per account and booking day, each day's closing balance opening the next.</p>
    {#if preview === null && !error}
      <p class="line">Reading the file…</p>
    {:else if preview && preview.problems.length > 0}
      <p class="line">This file can't be booked:</p>
      <ul class="problems">
        {#each preview.problems as p}<li>{p}</li>{/each}
      </ul>
    {:else if preview}
      <p class="line">{previewLine(preview)}</p>
    {/if}
    <label class="field">
      <span>Opening balance</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="mono" type="text" inputmode="decimal" bind:value={opening} autofocus
             on:keydown={(e) => { if (e.key === "Enter") generate(); }} />
      <span>EUR</span>
    </label>
    {#if normalized === null}<p class="err">Enter an amount like 10000.00</p>{/if}
    <p class="hint">Saved next to the source file. All bank data in the statement is synthetic.</p>
    {#if schemaMissing}<p class="warn">{STATEMENT_XSD} isn't imported yet, so the new statements will show "schema missing". Import it via Schemas… to validate them.</p>{/if}
    {#if $progress.running && !busy}<p class="hint">A validation is running; Generate is available when it has finished.</p>{/if}
    {#if error}<p class="err">{error}</p>{/if}
    <footer>
      <button class="btn btn--primary" on:click={generate} disabled={!allowed}>{busy ? "Generating…" : "Generate"}</button>
      <button class="btn btn--ghost close" on:click={close}>Close</button>
    </footer>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 50;
  }
  .dialog {
    background: var(--panel);
    color: var(--fg);
    border: 1px solid var(--border);
    border-radius: 8px;
    width: min(520px, 92vw);
    max-height: 86vh;
    overflow: auto;
    display: flex;
    flex-direction: column;
    padding: 14px 16px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .hint { opacity: 0.8; font-size: 12px; margin: 6px 0; }
  .line { font-size: 13px; margin: 8px 0 4px; }
  .problems { font-size: 12px; color: var(--err); margin: 0 0 8px; padding-left: 18px; }
  .field { display: flex; align-items: center; gap: var(--sp-2); font-size: 13px; margin: 8px 0 2px; }
  .field input { width: 140px; text-align: right; }
  .warn { font-size: 12px; color: var(--warn); margin: 4px 0; }
  .err { font-size: 12px; color: var(--err); margin: 4px 0; white-space: pre-line; }
  footer { display: flex; gap: var(--sp-2); margin-top: var(--sp-3); }
  footer .close { margin-left: auto; }
</style>
