<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";
  import { argbCss } from "../color";
  import type { VarKind } from "../engine";

  // Picks a variable to bind (Figma's "Apply variable" list), filtered by type.

  let {
    kind,
    top,
    right = 252,
    onpick,
    onclose,
  }: {
    kind: VarKind;
    top: number;
    right?: number;
    onpick: (id: string) => void;
    onclose: () => void;
  } = $props();

  let search = $state("");
  let el = $state<HTMLDivElement>();
  const y = $derived(Math.max(8, Math.min(top - 20, window.innerHeight - 380)));

  const groups = $derived(
    editor.variables.collections
      .map((c) => ({
        c,
        vars: editor.variables.variables.filter(
          (v) => v.collection === c.id && v.type === kind && v.name.toLowerCase().includes(search.trim().toLowerCase()),
        ),
      }))
      .filter((g) => g.vars.length),
  );

  function preview(id: string): string {
    const v = editor.resolveVariable(id);
    if (!v) return "";
    if (v.kind === "color") return argbCss(v.value);
    return String(v.value);
  }

  $effect(() => {
    const close = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node) && !(e.target as HTMLElement).closest("[data-var-trigger]")) onclose();
    };
    window.addEventListener("pointerdown", close, true);
    return () => window.removeEventListener("pointerdown", close, true);
  });
</script>

<div class="picker" bind:this={el} style="top:{y}px; right:{right}px" role="dialog" aria-label="Variables">
  <header>
    <span class="title">Variables</span>
    <button class="icon-btn" title="Close" onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>
  <!-- svelte-ignore a11y_autofocus -->
  <input class="search" placeholder="Search" bind:value={search} autofocus onkeydown={(e) => { e.stopPropagation(); if (e.key === "Escape") onclose(); }} />
  <div class="list">
    {#each groups as g (g.c.id)}
      <div class="group faint">{g.c.name}</div>
      {#each g.vars as v (v.id)}
        <button class="item" onclick={() => onpick(v.id)}>
          {#if kind === "color"}
            <span class="swatch" style="background:{preview(v.id)}"></span>
          {:else}
            <span class="value faint">{preview(v.id)}</span>
          {/if}
          <span class="name">{v.name}</span>
        </button>
      {/each}
    {:else}
      <p class="faint empty">No {kind} variables. Create them in Variables (right panel, nothing selected).</p>
    {/each}
  </div>
  <footer>
    <button class="open" onclick={() => { onclose(); editor.variablesOpen = true; }}>Open variables…</button>
  </footer>
</div>

<style>
  .picker {
    position: fixed;
    z-index: 70;
    width: 240px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
    display: flex;
    flex-direction: column;
    max-height: 360px;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-weight: 600;
  }
  .search {
    margin: 8px 12px 4px;
    height: 26px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  .search:focus {
    border-color: var(--brand-strong);
  }
  .list {
    overflow-y: auto;
    padding: 4px 4px 8px;
  }
  .group {
    padding: 6px 8px 2px;
    font-size: 11px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    height: 28px;
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
  .value {
    min-width: 28px;
    font-variant-numeric: tabular-nums;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .empty {
    padding: 8px;
    line-height: 1.5;
  }
  footer {
    border-top: 1px solid var(--border);
    padding: 6px 4px;
  }
  .open {
    width: 100%;
    height: 28px;
    padding: 0 8px;
    text-align: left;
    border-radius: var(--radius);
  }
  .open:hover {
    background: var(--bg-secondary);
  }
</style>
