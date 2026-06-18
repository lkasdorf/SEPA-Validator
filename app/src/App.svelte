<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";

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
    <section class="viewer"><CodeViewer /></section>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="gutter" role="separator" aria-orientation="vertical" on:mousedown={(e) => startDrag("right", e)}></div>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
</div>
