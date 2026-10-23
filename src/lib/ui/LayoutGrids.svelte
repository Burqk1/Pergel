<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import { editor } from "../editor.svelte";
  import type { LayoutGrid, NodeInfo } from "../engine";

  // Figma's "Layout guide" section: columns, rows and grids on a frame.

  let { n }: { n: NodeInfo } = $props();

  const DEFAULT: LayoutGrid = { kind: "columns", count: 12, gutter: 20, margin: 0, size: 10, align: "stretch", color: 0x1aff0000, visible: true };

  function set(list: LayoutGrid[]) {
    editor.patch({ grids: list }, n.id);
  }

  function change(i: number, c: Partial<LayoutGrid>) {
    set(n.grids.map((g, k) => (k === i ? { ...g, ...c } : g)));
  }

  const num = (v: string, fallback: number) => {
    const x = Number(v);
    return Number.isFinite(x) ? x : fallback;
  };
  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };
</script>

<div class="section-head">
  <span class="section-title">Layout guide</span>
  <button class="icon-btn" title="Add layout guide" onclick={() => set([...n.grids, DEFAULT])}><Plus size={14} strokeWidth={1.7} /></button>
</div>
{#each n.grids as g, i (i)}
  <div class="grid-row" class:off={!g.visible}>
    <select value={g.kind} onchange={(e) => change(i, { kind: (e.target as HTMLSelectElement).value as LayoutGrid["kind"] })}>
      <option value="columns">Columns</option>
      <option value="rows">Rows</option>
      <option value="grid">Grid</option>
    </select>
    {#if g.kind === "grid"}
      <input title="Size" value={g.size} onkeydown={stop} onblur={(e) => change(i, { size: Math.max(1, num((e.target as HTMLInputElement).value, g.size)) })} />
    {:else}
      <input title="Count" value={g.count} onkeydown={stop} onblur={(e) => change(i, { count: Math.max(1, Math.round(num((e.target as HTMLInputElement).value, g.count))) })} />
    {/if}
    <button class="icon-btn" title={g.visible ? "Hide" : "Show"} onclick={() => change(i, { visible: !g.visible })}>
      {#if g.visible}<Eye size={14} strokeWidth={1.7} />{:else}<EyeOff size={14} strokeWidth={1.7} />{/if}
    </button>
    <button class="icon-btn" title="Remove" onclick={() => set(n.grids.filter((_, k) => k !== i))}><Minus size={14} strokeWidth={1.7} /></button>
  </div>
  {#if g.kind !== "grid"}
    <div class="grid-detail">
      <label><span class="faint">Gutter</span><input value={g.gutter} onkeydown={stop} onblur={(e) => change(i, { gutter: Math.max(0, num((e.target as HTMLInputElement).value, g.gutter)) })} /></label>
      <label><span class="faint">Margin</span><input value={g.margin} onkeydown={stop} onblur={(e) => change(i, { margin: Math.max(0, num((e.target as HTMLInputElement).value, g.margin)) })} /></label>
      <label>
        <span class="faint">Type</span>
        <select value={g.align} onchange={(e) => change(i, { align: (e.target as HTMLSelectElement).value as LayoutGrid["align"] })}>
          <option value="stretch">Stretch</option>
          <option value="start">{g.kind === "columns" ? "Left" : "Top"}</option>
          <option value="center">Center</option>
          <option value="end">{g.kind === "columns" ? "Right" : "Bottom"}</option>
        </select>
      </label>
      {#if g.align !== "stretch"}
        <label><span class="faint">Width</span><input value={g.size} onkeydown={stop} onblur={(e) => change(i, { size: Math.max(1, num((e.target as HTMLInputElement).value, g.size)) })} /></label>
      {/if}
    </div>
  {/if}
{/each}

<style>
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .grid-row {
    display: grid;
    grid-template-columns: 1fr 56px 24px 24px;
    gap: 4px;
    align-items: center;
    padding: 0 8px 4px 16px;
  }
  .grid-row.off select,
  .grid-row.off input {
    opacity: 0.5;
  }
  .grid-detail {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px 8px;
    padding: 0 16px 8px;
  }
  label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  label span {
    width: 44px;
    font-size: 11px;
  }
  select,
  input {
    min-width: 0;
    width: 100%;
    height: 24px;
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
    outline: none;
  }
  input:focus {
    border-color: var(--brand-strong);
  }
</style>
