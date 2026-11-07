<script lang="ts">
  import { selectedDraft } from "./drafts";
  import { validateDraft, saveDraft, discardDraft, editBusy, editNote } from "./editing";
</script>

{#if $selectedDraft}
  <span class="modified" title="The text differs from the file on disk">● Modified</span>
  <button class="btn btn--primary" on:click={validateDraft} disabled={$editBusy}
          title="Check the edited text without saving it">Validate</button>
  <button class="btn btn--ghost" on:click={saveDraft} disabled={$editBusy}
          title="Save the edited text as NAME_editN.xml next to the original (Ctrl+S)">Save copy</button>
  <button class="btn btn--ghost" on:click={discardDraft} disabled={$editBusy}
          title="Put the file's text back (Ctrl+Z brings the edits back)">Discard</button>
{/if}
{#if $editNote}
  <span class="note" class:failed={$editNote.failed} role="status" aria-live="polite">{$editNote.text}</span>
{/if}

<style>
  .modified { font-size: 12px; font-weight: 600; color: var(--warn); white-space: nowrap; }
  .note { font-size: 11px; color: var(--ok); }
  .note.failed { color: var(--err); }
</style>
