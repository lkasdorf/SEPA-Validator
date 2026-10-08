<script lang="ts">
  import { save } from "@tauri-apps/plugin-dialog";
  import { selectedResult } from "./stores";
  import { saveFormatted } from "./api";
  import { formattedCopyPath } from "./viewer";

  let note = "";
  let failed = false;
  let timer: ReturnType<typeof setTimeout> | undefined;

  function show(text: string, isError: boolean) {
    note = text;
    failed = isError;
    clearTimeout(timer);
    timer = setTimeout(() => (note = ""), isError ? 8000 : 3000);
  }

  async function saveCopy() {
    const source = $selectedResult?.path;
    if (!source) return;
    const target = await save({
      defaultPath: formattedCopyPath(source),
      filters: [{ name: "XML", extensions: ["xml"] }],
    });
    if (!target) return;
    try {
      await saveFormatted(source, target);
      show(`Saved ${target.split(/[\\/]/).pop()}`, false);
    } catch (e) {
      show(String(e), true);
    }
  }
</script>

<button class="btn btn--ghost" on:click={saveCopy} disabled={!$selectedResult}
        title="Save the indented XML as shown here as a new file (values stay unchanged)">Save formatted…</button>
<span class="note" class:failed role="status" aria-live="polite">{note}</span>

<style>
  .note { font-size: 11px; color: var(--ok); }
  .note.failed { color: var(--err); }
</style>
