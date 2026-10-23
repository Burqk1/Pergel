<script lang="ts">
  import LiveShare from "./LiveShare.svelte";
  import { live } from "../live.svelte";
  import Play from "@lucide/svelte/icons/play";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plus from "@lucide/svelte/icons/plus";
  import Eye from "@lucide/svelte/icons/eye";
  import Copy from "@lucide/svelte/icons/copy";
  import NumberField from "./NumberField.svelte";
  import DesignPanel from "./DesignPanel.svelte";
  import Interactions from "./Interactions.svelte";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";
  import { css, tailwind } from "../code";

  let zoomOpen = $state(false);
  let codeTab = $state<"css" | "tailwind">("css");

  const single = $derived(editor.selection.length === 1 ? editor.selection[0] : null);
  const zoomLabel = $derived(`${Math.round(editor.view.zoom * 100)}%`);

  const zoomMenu = (): MenuItem[] => [
    { label: "Zoom in", shortcut: "Ctrl++", action: () => editor.zoomBy(2) },
    { label: "Zoom out", shortcut: "Ctrl+-", action: () => editor.zoomBy(0.5) },
    { label: "Zoom to fit", shortcut: "Shift+1", action: () => editor.fit() },
    { label: "Zoom to selection", shortcut: "Shift+2", disabled: !editor.selection.length, action: () => editor.fitSelection() },
    { label: "Zoom to 50%", action: () => editor.zoomTo(0.5) },
    { label: "Zoom to 100%", shortcut: "Shift+0", action: () => editor.zoomTo(1) },
    { label: "Zoom to 200%", action: () => editor.zoomTo(2) },
    "-",
    { label: "Performance stats", checked: editor.showStats, action: () => (editor.showStats = !editor.showStats) },
  ];


  async function copy(text: string) {
    await navigator.clipboard.writeText(text);
    editor.notify("Copied to clipboard");
  }

  const soon = (what: string) => () => editor.notify(`${what} is coming soon`);
  import { argbCss } from "../color";
  import Minus from "@lucide/svelte/icons/minus";
  // Connections show on the canvas while the Prototype tab is open.
  $effect(() => {
    editor.engine?.showConnections(editor.rightTab === "prototype");
    editor.invalidate();
  });
  let sharing = $state(false);
</script>

{#snippet section(title: string, add?: () => void, extra?: import("svelte").Snippet)}
  <div class="section-head">
    <span class="section-title">{title}</span>
    <span class="head-actions">
      {#if extra}{@render extra()}{/if}
      {#if add}
        <button class="icon-btn" title="Add {title.toLowerCase()}" onclick={add}><Plus size={14} strokeWidth={1.7} /></button>
      {/if}
    </span>
  </div>
{/snippet}

<aside class="right">
  <div class="top">
    <span class="avatar" title="You">B</span>
    {#each Object.values(live.peers) as p (p.id)}
      <span class="avatar peer" title={p.name} style="background:{p.color}">{p.name.slice(0, 1).toUpperCase()}</span>
    {/each}
    <span class="spacer"></span>
    <button class="present" title="Present (Ctrl+Alt+Enter)" onclick={() => editor.present()}>
      <Play size={13} strokeWidth={1.8} /><ChevronDown size={10} strokeWidth={2} />
    </button>
    <button class="share" class:live={live.status === "on"} onclick={() => (sharing = !sharing)}>{live.status === "on" ? "Live" : "Share"}</button>
    {#if sharing}<LiveShare onclose={() => (sharing = false)} />{/if}
  </div>
  <div class="tabs">
    <button class:active={editor.rightTab === "design"} onclick={() => (editor.rightTab = "design")}>Design</button>
    <button class:active={editor.rightTab === "prototype"} onclick={() => (editor.rightTab = "prototype")}>Prototype</button>
    <span class="spacer"></span>
    <div class="zoom">
      <button class="zoom-btn" data-menu-trigger onclick={() => (zoomOpen = !zoomOpen)}>
        {zoomLabel}<ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={zoomMenu()} bind:open={zoomOpen} align="right" />
    </div>
  </div>

  <div class="body">
    {#if editor.rightTab === "prototype"}
      {#if single}
        <Interactions n={single} />
      {:else}
        {@render section("Interactions")}
        <p class="hint faint">Select a layer to add interactions. Press ▷ to play the prototype.</p>
      {/if}
    {:else if !editor.selection.length}
      {@render section("Page")}
      <div class="row">
        <span class="swatch" style="background:#1E1E1E"></span>
        <span class="hex">1E1E1E</span>
        <span class="pct muted">100%</span>
        <button class="icon-btn" title="Hide"><Eye size={14} strokeWidth={1.7} /></button>
      </div>
      <div class="divider"></div>
      {@render section("Variables")}
      <button class="wide-btn" onclick={() => (editor.variablesOpen = true)}>Open variables</button>
      <div class="divider"></div>
      {@render section("Styles", () => editor.notify("Select a layer, then use the style buttons in Typography or Fill to save a style"))}
      {#each ["text", "color", "effect"] as type (type)}
        {@const list = editor.styles.list.filter((s) => s.type === type)}
        {#if list.length}
          <div class="style-group faint">{type === "text" ? "Text styles" : type === "color" ? "Color styles" : "Effect styles"}</div>
          {#each list as s (s.id)}
            <div class="style-item">
              {#if s.type === "color"}<span class="swatch" style="background:{argbCss(s.color)}"></span>
              {:else if s.type === "text"}<span class="ag" style="font-weight:{s.weight}">Ag</span>{/if}
              <input
                class="style-name"
                value={s.name}
                onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
                onblur={(e) => {
                  const name = (e.target as HTMLInputElement).value.trim();
                  if (name && name !== s.name) editor.setStyles({ list: editor.styles.list.map((x) => (x.id === s.id ? { ...x, name } : x)) });
                }}
              />
              {#if s.type === "text"}<span class="faint meta">{s.size}</span>{/if}
              <button class="icon-btn" title="Delete style" onclick={() => editor.setStyles({ list: editor.styles.list.filter((x) => x.id !== s.id) })}><Minus size={13} strokeWidth={1.7} /></button>
            </div>
          {/each}
        {/if}
      {/each}
      <div class="divider"></div>
      {@render section("Export", soon("Export"))}
    {:else}
      <DesignPanel />

      {#if editor.devMode && single}
        {@const n = single}
        <div class="divider"></div>
        <div class="section-head">
          <span class="section-title">Code</span>
          <span class="head-actions">
            <button class="code-tab" class:active={codeTab === "css"} onclick={() => (codeTab = "css")}>CSS</button>
            <button class="code-tab" class:active={codeTab === "tailwind"} onclick={() => (codeTab = "tailwind")}>Tailwind</button>
            <button class="icon-btn" title="Copy" onclick={() => copy(codeTab === "css" ? css(n) : tailwind(n))}>
              <Copy size={13} strokeWidth={1.7} />
            </button>
          </span>
        </div>
        <pre class="code">{codeTab === "css" ? css(n) : tailwind(n)}</pre>
      {/if}
    {/if}
  </div>
</aside>

<style>
  .style-group {
    padding: 4px 16px 2px;
    font-size: 11px;
  }
  .style-item {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 8px 0 16px;
  }
  .style-item .ag {
    width: 18px;
    flex: none;
  }
  .style-name {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 4px;
    margin-left: -4px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text);
    outline: none;
  }
  .style-name:hover {
    border-color: var(--border);
  }
  .style-name:focus {
    border-color: var(--brand-strong);
  }
  .meta {
    font-size: 11px;
  }
  .right {
    display: flex;
    flex-direction: column;
    width: 240px;
    flex: none;
    background: var(--bg);
    border-left: 1px solid var(--border);
    min-height: 0;
  }
  .top,
  .tabs {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 8px;
  }
  .top {
    height: 48px;
    border-bottom: 1px solid var(--border);
  }
  .tabs {
    height: 40px;
    border-bottom: 1px solid var(--border);
  }
  .spacer {
    flex: 1;
  }
  .avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 50%;
    background: var(--brand-strong);
    font-weight: 600;
  }
  .present {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
  }
  .present:hover {
    background: var(--bg-secondary);
  }
  .avatar.peer {
    margin-left: -6px;
    box-shadow: 0 0 0 2px var(--panel-bg, #2c2c2c);
    color: #fff;
    font-size: 11px;
  }
  .share.live {
    background: #14ae5c;
  }
  .top {
    position: relative;
  }
  .share {
    height: 24px;
    padding: 0 12px;
    border-radius: var(--radius);
    background: var(--brand);
    color: #fff;
    font-weight: 550;
  }
  .share:hover {
    background: var(--brand-strong);
  }
  .tabs > button {
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-weight: 550;
  }
  .tabs > button.active {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .zoom {
    position: relative;
  }
  .zoom-btn {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }
  .zoom-btn:hover {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    padding-bottom: 16px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .divider {
    height: 1px;
    margin: 8px 0 4px;
    background: var(--border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .swatch {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }
  .hex {
    flex: 1;
    min-width: 0;
    font-variant-numeric: tabular-nums;
  }
  .pct {
    width: 40px;
    text-align: right;
  }
  .wide-btn {
    width: calc(100% - 32px);
    margin: 0 16px;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    font-weight: 550;
  }
  .wide-btn:hover {
    background: var(--bg-secondary);
  }
  .hint {
    padding: 0 16px;
    margin: 4px 0;
  }
  .code-tab {
    height: 20px;
    padding: 0 6px;
    border-radius: 4px;
    color: var(--text-secondary);
  }
  .code-tab.active {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .code {
    margin: 0 16px;
    padding: 10px;
    background: var(--canvas);
    border-radius: var(--radius);
    font: 11px/16px "Cascadia Code", "Consolas", monospace;
    color: var(--text-brand);
    white-space: pre-wrap;
    word-break: break-all;
    user-select: text;
  }
</style>
