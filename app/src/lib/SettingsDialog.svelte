<script lang="ts">
  import { disabledRules, RULE_GROUPS, setRuleGroup, settingsDialogOpen } from "./settings";
  import { results, progress } from "./stores";
  import { runValidation } from "./validation";

  // The groups as they were when the dialog opened, to offer a new run on changes.
  const before = JSON.stringify([...$disabledRules].sort());
  $: changed = JSON.stringify([...$disabledRules].sort()) !== before;
  $: paths = $results.map((r) => r.path);

  function close() { settingsDialogOpen.set(false); }

  function revalidate() {
    close();
    void runValidation(paths);
  }
</script>

<svelte:window on:keydown={(e) => { if (e.key === "Escape") close(); }} />

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div class="overlay" on:click|self={close}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title">
    <header>
      <strong id="settings-title">Settings</strong>
      <button class="x" on:click={close} aria-label="Close">✕</button>
    </header>

    <div class="content">
      <h3>DK usage rules</h3>
      <p class="muted">
        Rules of the German banking industry (DFÜ-Abkommen Anlage 3) for SEPA credit transfers and direct debits that the
        XSD can't check. Switch off a group if your bank doesn't apply it. XSD validation and the other checks always run.
      </p>
      <ul>
        {#each RULE_GROUPS as g (g.id)}
          <li>
            <label>
              <input
                type="checkbox"
                checked={!$disabledRules.includes(g.id)}
                on:change={(e) => setRuleGroup(g.id, e.currentTarget.checked)}
              />
              <span>
                <span class="label">{g.label}</span>
                <span class="detail">{g.detail}</span>
              </span>
            </label>
          </li>
        {/each}
      </ul>
      <p class="note">Changes apply to the next validation.</p>
    </div>

    <footer>
      {#if changed && paths.length > 0}
        <button class="btn" on:click={revalidate} disabled={$progress.running}>
          Validate {paths.length === 1 ? "the file" : `${paths.length} files`} again
        </button>
      {/if}
      <button class="btn btn--ghost close" on:click={close}>Close</button>
    </footer>
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
    width: min(560px, 92vw); max-height: 86vh;
    display: flex; flex-direction: column;
    padding: var(--sp-3) var(--sp-4);
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  header .x { background: transparent; border: none; color: var(--fg); cursor: pointer; font-size: 16px; }
  .content { overflow: auto; min-height: 0; font-size: 13px; line-height: 1.5; margin-top: var(--sp-3); }
  h3 { margin: 0 0 var(--sp-1); font-size: 13px; }
  .content p { margin: 0 0 var(--sp-2); }
  .muted { color: var(--muted); }
  ul { list-style: none; margin: 0; padding: 0; }
  li { border-bottom: 1px solid var(--border); }
  li:last-child { border-bottom: none; }
  label { display: flex; gap: var(--sp-2); align-items: flex-start; padding: var(--sp-2) 0; cursor: pointer; }
  input { margin-top: 3px; }
  .label { display: block; font-weight: 600; }
  .detail { display: block; color: var(--muted); font-size: 12px; }
  .note { margin-top: var(--sp-2); font-size: 12px; color: var(--muted); }
  footer { display: flex; gap: var(--sp-2); margin-top: var(--sp-3); }
  footer .close { margin-left: auto; }
</style>
