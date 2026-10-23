<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import { editor } from "../editor.svelte";
  import { argbCss } from "../color";
  import type { Style, StyleKind } from "../engine";

  // Figma's style list for a section (text, fill, stroke, effect), with "Create style".

  let {
    kind,
    top,
    right = 252,
    onclose,
  }: {
    kind: StyleKind;
    top: number;
    right?: number;
    onclose: () => void;
  } = $props();

  let search = $state("");
  let creating = $state<string | null>(null);
  let el = $state<HTMLDivElement>();
  const y = $derived(Math.max(8, Math.min(top - 20, window.innerHeight - 400)));
  const type = $derived(kind === "text" ? "text" : kind === "effect" ? "effect" : "color");
  const list = $derived(
    editor.styles.list.filter((s) => s.type === type && s.name.toLowerCase().includes(search.trim().toLowerCase())),
  );

  function preview(s: Style): string {
    if (s.type === "color") return argbCss(s.color);
    if (s.type === "text") return `${s.size}/${s.lineHeight.unit === "auto" ? "auto" : s.lineHeight.value}`;
    return "";
  }

  function pick(s: Style) {
    editor.linkStyle(kind, s.id);
    onclose();
  }

  function create() {
    if (creating?.trim()) editor.createStyle(kind, creating.trim());
    onclose();
  }

  $effect(() => {
    const close = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node) && !(e.target as HTMLElement).closest("[data-style-trigger]")) onclose();
    };
    window.addEventListener("pointerdown", close, true);
    return () => window.removeEventListener("pointerdown", close, true);
  });

  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") onclose();
  };
</script>

<div class="picker" bind:this={el} style="top:{y}px; right:{right}px" role="dialog" aria-label="Styles">
  <header>
    <span class="title">{kind === "text" ? "Text styles" : kind === "effect" ? "Effect styles" : "Color styles"}</span>
    <button class="icon-btn" title="Create style" onclick={() => (creating = creating === null ? "" : null)}><Plus size={14} strokeWidth={1.8} /></button>
    <button class="icon-btn" title="Close" onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>
  {#if creating !== null}
    <div class="create">
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={creating} placeholder={kind === "text" ? "Heading/H1" : "Brand/Primary"} autofocus onkeydown={(e) => { stop(e); if (e.key === "Enter") create(); }} />
      <button class="primary" onclick={create}>Create</button>
    </div>
  {:else}
    <!-- svelte-ignore a11y_autofocus -->
    <input class="search" placeholder="Search" bind:value={search} autofocus onkeydown={stop} />
  {/if}
  <div class="list">
    {#each list as s (s.id)}
      <button class="item" onclick={() => pick(s)}>
        {#if s.type === "color"}
          <span class="swatch" style="background:{preview(s)}"></span>
        {:else if s.type === "text"}
          <span class="ag" style="font-weight:{s.weight}">Ag</span>
        {/if}
        <span class="name">{s.name}</span>
        {#if s.type === "text"}<span class="faint meta">{preview(s)}</span>{/if}
      </button>
    {:else}
      <p class="faint empty">No styles yet. Click + to save the selection's {kind === "text" ? "typography" : kind} as a style.</p>
    {/each}
  </div>
</div>

<style>
  .picker {
    position: fixed;
    z-index: 70;
    width: 240px;
    max-height: 380px;
    display: flex;
    flex-direction: column;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 40px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    flex: 1;
    font-weight: 600;
  }
  input {
    height: 26px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  input:focus {
    border-color: var(--brand-strong);
  }
  .search {
    margin: 8px 12px 4px;
  }
  .create {
    display: flex;
    gap: 6px;
    margin: 8px 12px 4px;
  }
  .create input {
    flex: 1;
    min-width: 0;
  }
  .primary {
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  .list {
    overflow-y: auto;
    padding: 4px 4px 8px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 30px;
    padding: 0 8px;
    border-radius: var(--radius);
    text-align: left;
  }
  .item:hover {
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  .swatch {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    flex: none;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.15);
  }
  .ag {
    width: 22px;
    flex: none;
  }
  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    font-size: 11px;
  }
  .empty {
    padding: 8px;
    line-height: 1.5;
  }
</style>
