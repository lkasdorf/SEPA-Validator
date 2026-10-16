<script lang="ts">
  import { checkIbanOnline, ibanCheckUrl } from "./iban";
  export let iban: string | null | undefined;
  $: url = ibanCheckUrl(iban);
</script>

{#if url}
  <button
    class="iban-check"
    type="button"
    title="Check this IBAN online (opens ibancalculator.com in your browser and sends the IBAN there)"
    aria-label="Check IBAN online"
    on:click|stopPropagation={() => checkIbanOnline(iban)}
  >↗</button>
{/if}

<style>
  .iban-check {
    all: unset; cursor: pointer; margin-left: 4px; padding: 0 3px; border-radius: 3px;
    font-size: 11px; line-height: 1; color: var(--muted);
  }
  .iban-check:hover { color: var(--accent); background: var(--sel); }
  .iban-check:focus-visible { outline: 2px solid var(--accent); outline-offset: 1px; }
</style>
