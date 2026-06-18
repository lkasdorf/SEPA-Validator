<script lang="ts">
  import Toolbar from "./lib/Toolbar.svelte";
  import FileList from "./lib/FileList.svelte";
  import CodeViewer from "./lib/CodeViewer.svelte";
  import LogPanel from "./lib/LogPanel.svelte";
  import SummaryBar from "./lib/SummaryBar.svelte";
</script>

<div class="app">
  <Toolbar />
  <main class="body">
    <aside class="files"><FileList /></aside>
    <section class="viewer"><CodeViewer /></section>
    <section class="log"><LogPanel /></section>
  </main>
  <SummaryBar />
</div>
