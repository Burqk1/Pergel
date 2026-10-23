<script lang="ts">
  import ArrowDown from "@lucide/svelte/icons/arrow-down";
  import ArrowRight from "@lucide/svelte/icons/arrow-right";
  import WrapIcon from "@lucide/svelte/icons/corner-down-left";
  import GridIcon from "@lucide/svelte/icons/grid-2x2";
  import GapH from "@lucide/svelte/icons/between-horizontal-start";
  import GapV from "@lucide/svelte/icons/between-vertical-start";
  import Columns from "@lucide/svelte/icons/columns-3";
  import Rows from "@lucide/svelte/icons/rows-3";
  import Padding from "@lucide/svelte/icons/square-dashed-mouse-pointer";
  import NumberField from "./NumberField.svelte";
  import { editor } from "../editor.svelte";
  import type { Align, AutoLayout, Flow, Justify, Patch } from "../engine";

  let { layout }: { layout: AutoLayout } = $props();
  let separate = $state(false);

  const set = (p: Patch) => editor.patch(p);
  const POS: ("start" | "center" | "end")[] = ["start", "center", "end"];
  const horizontal = $derived(layout.flow === "horizontal" || layout.flow === "wrap" || layout.flow === "grid");
  const spaced = $derived(layout.justify === "space-between");

  /** Cell (row, column) of the alignment box → justify (along the flow) and align (across). */
  function choose(row: number, col: number) {
    const [main, cross] = horizontal ? [col, row] : [row, col];
    const justify: Justify = spaced ? "space-between" : POS[main];
    set({ justify, align: POS[cross] as Align });
  }

  function active(row: number, col: number) {
    const [main, cross] = horizontal ? [col, row] : [row, col];
    const j = layout.justify === "space-between" ? -1 : POS.indexOf(layout.justify as "start");
    const a = layout.align === "baseline" ? 0 : POS.indexOf(layout.align as "start");
    return (j === -1 || j === main) && a === cross;
  }

  const flows: [Flow, string, typeof ArrowDown][] = [
    ["vertical", "Vertical layout", ArrowDown],
    ["horizontal", "Horizontal layout", ArrowRight],
    ["wrap", "Wrap", WrapIcon],
    ["grid", "Grid", GridIcon],
  ];

  const [top, right, bottom, left] = $derived(layout.padding);
</script>

<div class="flows">
  <div class="seg">
    {#each flows as [flow, title, Icon] (flow)}
      <button class="icon-btn" class:on={layout.flow === flow} {title} onclick={() => set({ flow })}>
        <Icon size={14} strokeWidth={1.7} />
      </button>
    {/each}
  </div>
</div>

<div class="body">
  <div class="align-box" role="group" aria-label="Alignment">
    {#each [0, 1, 2] as row (row)}
      {#each [0, 1, 2] as col (col)}
        <button class="cell" class:active={active(row, col)} aria-label="Align" onclick={() => choose(row, col)}>
          <span class="dot"></span>
        </button>
      {/each}
    {/each}
  </div>
  <div class="spacing">
    <div class="gap-row">
      <NumberField
        min={0}
        title={layout.flow === "grid" ? "Column gap" : "Gap between items"}
        value={layout.gap}
        mixed={spaced}
        onCommit={(v) => set({ gap: v, justify: spaced ? "start" : layout.justify })}
      >
        {#snippet label()}{#if horizontal}<GapH size={12} strokeWidth={1.8} />{:else}<GapV size={12} strokeWidth={1.8} />{/if}{/snippet}
      </NumberField>
      <button
        class="auto"
        class:on={spaced}
        title="Auto spacing (space between)"
        onclick={() => set({ justify: spaced ? "start" : "space-between" })}>Auto</button
      >
    </div>
    {#if layout.flow === "wrap" || layout.flow === "grid"}
      <NumberField min={0} title="Gap between rows" value={layout.counterGap} onCommit={(v) => set({ counterGap: v })}>
        {#snippet label()}<GapV size={12} strokeWidth={1.8} />{/snippet}
      </NumberField>
    {/if}
    {#if layout.flow === "grid"}
      <div class="pair">
        <NumberField min={1} max={64} title="Columns" value={layout.columns} onCommit={(v) => set({ columns: Math.round(v) })}>
          {#snippet label()}<Columns size={12} strokeWidth={1.8} />{/snippet}
        </NumberField>
        <NumberField min={1} max={64} title="Rows" value={layout.rows} onCommit={(v) => set({ rows: Math.round(v) })}>
          {#snippet label()}<Rows size={12} strokeWidth={1.8} />{/snippet}
        </NumberField>
      </div>
    {/if}
  </div>
</div>

<div class="padding">
  {#if separate}
    <NumberField label="T" min={0} title="Top padding" value={top} onCommit={(v) => set({ padding: [v, right, bottom, left] })} />
    <NumberField label="R" min={0} title="Right padding" value={right} onCommit={(v) => set({ padding: [top, v, bottom, left] })} />
    <NumberField label="B" min={0} title="Bottom padding" value={bottom} onCommit={(v) => set({ padding: [top, right, v, left] })} />
    <NumberField label="L" min={0} title="Left padding" value={left} onCommit={(v) => set({ padding: [top, right, bottom, v] })} />
  {:else}
    <NumberField
      label="↔"
      min={0}
      title="Horizontal padding"
      value={left}
      mixed={left !== right}
      onCommit={(v) => set({ padding: [top, v, bottom, v] })}
    />
    <NumberField
      label="↕"
      min={0}
      title="Vertical padding"
      value={top}
      mixed={top !== bottom}
      onCommit={(v) => set({ padding: [v, right, v, left] })}
    />
  {/if}
  <button class="icon-btn" class:on={separate} title="Individual padding" onclick={() => (separate = !separate)}>
    <Padding size={14} strokeWidth={1.7} />
  </button>
</div>

<style>
  .flows {
    padding: 0 16px 8px;
  }
  .seg {
    display: inline-flex;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .body {
    display: grid;
    grid-template-columns: 72px 1fr;
    gap: 8px;
    padding: 0 16px 8px;
  }
  .align-box {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    grid-template-rows: repeat(3, 1fr);
    width: 72px;
    height: 72px;
    padding: 4px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .cell {
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 3px;
  }
  .cell:hover .dot {
    background: var(--text-secondary);
  }
  .dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text-tertiary);
  }
  .cell.active .dot {
    width: 12px;
    height: 4px;
    border-radius: 1px;
    background: var(--brand-strong);
  }
  .spacing {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }
  .gap-row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px;
  }
  .auto {
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-size: 11px;
  }
  .auto:hover {
    background: var(--bg-secondary);
  }
  .auto.on {
    background: var(--bg-selected);
    color: var(--text);
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 4px;
  }
  .padding {
    display: grid;
    grid-template-columns: 1fr 1fr 24px;
    gap: 8px;
    align-items: center;
    padding: 0 8px 8px 16px;
  }
</style>
