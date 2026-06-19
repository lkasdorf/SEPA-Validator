<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import SummaryView from "./lib/SummaryView.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";
  import { selectedResult, openViewerSearch, foldAllInViewer, unfoldAllInViewer } from "./lib/stores";
  import { loadPaymentSummary } from "./lib/paymentSummary";

  let viewerTab: "xml" | "summary" = "xml";
  $: if (viewerTab !== "xml") loadPaymentSummary($selectedResult?.path);

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
  <main class="body" style="grid-template-columns: {leftWidth}px 6px 1fr 6px {rightWidth}px;">
    <aside class="files"><FileList /></aside>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("left", e)}></div>
    <section class="viewer">
      <div class="viewer-bar">
        <div class="viewer-tabs">
          <button class:active={viewerTab === "xml"} on:click={() => (viewerTab = "xml")}>XML</button>
          <button class:active={viewerTab === "summary"} on:click={() => (viewerTab = "summary")}>Übersicht</button>
        </div>
        {#if viewerTab === "xml"}
          <button on:click={() => $openViewerSearch()} disabled={!$selectedResult}>Search</button>
          <button on:click={() => $foldAllInViewer()} disabled={!$selectedResult}>Collapse all</button>
          <button on:click={() => $unfoldAllInViewer()} disabled={!$selectedResult}>Expand all</button>
        {/if}
      </div>
      <div class="viewer-pane" class:hidden={viewerTab !== "xml"}><CodeViewer /></div>
      {#if viewerTab === "summary"}<SummaryView />{/if}
    </section>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("right", e)}></div>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
</div>
