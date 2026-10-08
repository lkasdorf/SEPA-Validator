<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import SummaryView from "./lib/SummaryView.svelte";
  import RemittanceView from "./lib/RemittanceView.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";
  import SchemaDialog from "./lib/SchemaDialog.svelte";
  import AboutDialog from "./lib/AboutDialog.svelte";
  import UpdateDialog from "./lib/UpdateDialog.svelte";
  import SaveFormatted from "./lib/SaveFormatted.svelte";
  import EmptyState from "./lib/EmptyState.svelte";
  import { results, progress, dragging, selectedResult, runId, openViewerSearch, foldAllInViewer, unfoldAllInViewer, schemaDialogOpen, viewerLarge, aboutDialogOpen, updateDialogOpen } from "./lib/stores";
  import { loadPaymentSummary } from "./lib/paymentSummary";

  let viewerTab: "xml" | "summary" | "remittance" = "xml";
  $: empty = $results.length === 0 && !$progress.running;
  $: if (viewerTab !== "xml") loadPaymentSummary($selectedResult?.path, $runId);

  let leftWidth = 260;
  let rightWidth = 360;

  function startDrag(which: "left" | "right", e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startLeft = leftWidth;
    const startRight = rightWidth;
    const onMove = (ev: MouseEvent) => {
      const dx = ev.clientX - startX;
      if (which === "left") {
        leftWidth = Math.min(480, Math.max(160, startLeft + dx));
      } else {
        rightWidth = Math.min(640, Math.max(240, startRight - dx));
      }
    };
    const onUp = () => {
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
    document.body.style.cursor = "col-resize";
    document.body.style.userSelect = "none";
  }
</script>

<div class="app">
  <Toolbar />
  <!-- Side panels shrink toward their minimum before the viewer drops below 280px. -->
  <main class="body" style="grid-template-columns: minmax(140px, {leftWidth}px) 6px minmax(280px, 1fr) 6px minmax(220px, {rightWidth}px);">
    <aside class="files"><FileList /></aside>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("left", e)}></div>
    <section class="viewer">
      <div class="viewer-bar">
        <div class="seg viewer-tabs">
          <button class:active={viewerTab === "xml"} on:click={() => (viewerTab = "xml")}>XML</button>
          <button class:active={viewerTab === "summary"} on:click={() => (viewerTab = "summary")}>Overview</button>
          <button class:active={viewerTab === "remittance"} on:click={() => (viewerTab = "remittance")}>Remittance</button>
        </div>
        {#if viewerTab === "xml"}
          <button class="btn btn--ghost" on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
          <SaveFormatted />
          {#if !$viewerLarge}
            <button class="btn btn--ghost" on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
            <button class="btn btn--ghost" on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
          {:else}
            <span class="viewer-note">Large file: syntax highlighting &amp; folding disabled (performance)</span>
          {/if}
        {/if}
      </div>
      {#if empty}<EmptyState />{/if}
      <div class="viewer-pane" class:hidden={viewerTab !== "xml" || empty}><CodeViewer /></div>
      {#if viewerTab === "summary" && !empty}<SummaryView />{/if}
      {#if viewerTab === "remittance" && !empty}<RemittanceView />{/if}
    </section>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("right", e)}></div>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
  {#if $dragging && $results.length > 0}<div class="drop-overlay" aria-hidden="true"><span>Drop to validate</span></div>{/if}
  {#if $schemaDialogOpen}<SchemaDialog />{/if}
  {#if $aboutDialogOpen}<AboutDialog />{/if}
  {#if $updateDialogOpen}<UpdateDialog />{/if}
</div>
