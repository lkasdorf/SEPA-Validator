<script lang="ts">
  import { createEventDispatcher, onMount } from "svelte";
  import { signedAmount, type RemittanceEntry } from "./types";
  import IbanCheckButton from "./IbanCheckButton.svelte";

  export let entry: RemittanceEntry;
  /** 1-based position of the entry in the file. */
  export let position: number;

  const dispatch = createEventDispatcher<{ close: void }>();
  const close = () => dispatch("close");

  let closeButton: HTMLButtonElement | undefined;
  onMount(() => closeButton?.focus());

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") close();
  }

  $: amount = entry.amount != null ? `${signedAmount(entry)} ${entry.currency ?? ""}` : "—";
</script>

<svelte:window on:keydown={onKey} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label="Entry {position}">
    <header>
      <strong>Entry {position}</strong>
      <button class="x" bind:this={closeButton} on:click={close} aria-label="Close">✕</button>
    </header>
    <div class="content">
      <dl>
        <dt>Booking date</dt><dd class="mono">{entry.bookingDate ?? "—"}</dd>
        <dt>Amount</dt><dd class="mono" class:debit={entry.creditDebit === "DBIT"}>{amount}</dd>
        <dt>Counterparty</dt><dd>{entry.name ?? "—"}</dd>
        <dt>IBAN</dt><dd class="mono">{entry.iban ?? "—"}<IbanCheckButton iban={entry.iban ?? null} /></dd>
        <dt>GVC</dt>
        <dd><span class="mono">{entry.gvc ?? "—"}{entry.prtryTxCode && entry.prtryTxCode !== entry.gvc ? ` (${entry.prtryTxCode})` : ""}</span>{#if entry.gvcText} · {entry.gvcText}{/if}</dd>
        <dt>ISO code</dt><dd class="mono">{entry.isoTxCode ?? "—"}</dd>
        <dt>Remittance info</dt><dd class="pre">{entry.ustrd ?? "—"}</dd>
        {#each entry.details ?? [] as d}
          <dt>{d.label}</dt><dd class="pre">{d.value}</dd>
        {/each}
      </dl>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 60;
    background: rgba(0, 0, 0, 0.45);
    display: flex; align-items: center; justify-content: center;
  }
  .dialog {
    background: var(--panel); color: var(--fg);
    border: 1px solid var(--border); border-radius: 8px;
    width: min(620px, 92vw); max-height: 86vh;
    display: flex; flex-direction: column;
    padding: var(--sp-3) var(--sp-4);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; margin-bottom: var(--sp-2); }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .content { overflow: auto; min-height: 0; font-size: 12px; }
  dl { display: grid; grid-template-columns: max-content 1fr; gap: 4px 16px; margin: 0; }
  dt { font-weight: 600; color: var(--muted); }
  dd { margin: 0; overflow-wrap: anywhere; }
  .pre { white-space: pre-wrap; }
  .debit { color: var(--err); }
</style>
