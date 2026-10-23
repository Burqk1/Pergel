<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { attachFrame, closeFigmaPlugin, figmaPlugin } from "../figmaPlugin.svelte";

  // A Figma plugin's UI (figma.showUI): its HTML in a sandboxed iframe, as in Figma.

  let el = $state<HTMLIFrameElement | null>(null);
  $effect(() => {
    attachFrame(el);
    return () => attachFrame(null);
  });
</script>

{#if figmaPlugin.open}
  <div class="plugin-ui" class:hidden={!figmaPlugin.visible} role="dialog" aria-label={figmaPlugin.name}>
    <header>
      <span class="name">{figmaPlugin.name}</span>
      <button class="icon-btn" title="Close plugin" onclick={closeFigmaPlugin}><X size={14} strokeWidth={1.8} /></button>
    </header>
    <iframe
      bind:this={el}
      title={figmaPlugin.name}
      sandbox="allow-scripts allow-forms allow-popups allow-modals"
      srcdoc={figmaPlugin.html}
      style="width:{figmaPlugin.width}px; height:{figmaPlugin.height}px"
    ></iframe>
  </div>
{/if}

<style>
  .plugin-ui {
    position: fixed;
    top: 64px;
    right: 260px;
    z-index: 140;
    display: flex;
    flex-direction: column;
    border-radius: 12px;
    overflow: hidden;
    background: #fff;
    box-shadow: var(--shadow-menu);
  }
  .plugin-ui.hidden {
    visibility: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 36px;
    padding: 0 6px 0 12px;
    background: var(--menu-bg, #1e1e1e);
  }
  .name {
    font-weight: 600;
  }
  iframe {
    border: 0;
    display: block;
    background: #fff;
  }
</style>
