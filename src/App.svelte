<script lang="ts">
  import LeftPanel from "./lib/ui/LeftPanel.svelte";
  import RightPanel from "./lib/ui/RightPanel.svelte";
  import CanvasView from "./lib/ui/CanvasView.svelte";
  import Toolbar from "./lib/ui/Toolbar.svelte";
  import ContextBar from "./lib/ui/ContextBar.svelte";
  import TitleBar from "./lib/ui/TitleBar.svelte";
  import Home from "./lib/ui/Home.svelte";
  import FigmaImport from "./lib/ui/FigmaImport.svelte";
  import NavRail from "./lib/ui/NavRail.svelte";
  import VariablesDialog from "./lib/ui/VariablesDialog.svelte";
  import QuickActions from "./lib/ui/QuickActions.svelte";
  import Present from "./lib/ui/Present.svelte";
  import VersionHistory from "./lib/ui/VersionHistory.svelte";
  import FindReplace from "./lib/ui/FindReplace.svelte";
  import OffsetPath from "./lib/ui/OffsetPath.svelte";
  import FigmaPluginUI from "./lib/ui/FigmaPluginUI.svelte";
  import BranchDialog from "./lib/ui/BranchDialog.svelte";
  import { workspace } from "./lib/workspace.svelte";
  import { editor } from "./lib/editor.svelte";

  // The webview's own menu (Back, Reload, Save as…) never belongs in the app; text fields keep it.
  function contextMenu(e: MouseEvent) {
    const t = e.target as HTMLElement;
    if (t.closest("input, textarea, [contenteditable]")) return;
    e.preventDefault();
  }
</script>

<svelte:window oncontextmenu={contextMenu} />

<div class="app">
  <TitleBar />
  <div class="content">
    <!-- The editor stays mounted behind Home so the canvas keeps its GPU context. -->
    <div class="editor" inert={workspace.active === "home"}>
      {#if !editor.uiHidden}<NavRail /><LeftPanel />{/if}
      <main class="center">
        <CanvasView />
        {#if !editor.uiHidden}
          <ContextBar />
          <Toolbar />
        {/if}
      </main>
      {#if !editor.uiHidden}<RightPanel />{/if}
    </div>
    {#if workspace.active === "home"}<Home />{/if}
  </div>
</div>
{#if editor.figmaImport}<FigmaImport />{/if}
{#if editor.variablesOpen}<VariablesDialog />{/if}
{#if editor.quickActions}<QuickActions />{/if}
{#if editor.presenting !== null}<Present start={editor.presenting} />{/if}
{#if editor.versionsOpen}<VersionHistory />{/if}
{#if editor.findOpen}<FindReplace />{/if}
{#if editor.offsetOpen}<OffsetPath />{/if}
<FigmaPluginUI />
{#if editor.branchOpen}<BranchDialog />{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .content {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .editor {
    display: flex;
    height: 100%;
  }
  .center {
    position: relative;
    flex: 1;
    min-width: 0;
    display: flex;
  }
</style>
