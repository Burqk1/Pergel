<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";
  import { exportLayer, parseSize, sizeLabel } from "../exporter";
  import type { ExportFormat, ExportSetting, NodeInfo } from "../engine";

  let { n }: { n: NodeInfo } = $props();

  const FORMATS: ExportFormat[] = ["png", "jpg", "svg", "pdf"];
  let formatMenu = $state<number | null>(null);
  let busy = $state(false);

  const rows = $derived(n.exports);
  const set = (list: ExportSetting[]) => editor.patch({ exports: list }, n.id);

  function add() {
    // Figma adds 1x first, then the next scale up.
    const used = new Set(rows.map((r) => r.scale));
    const scale = [1, 2, 3, 4, 0.5].find((s) => !used.has(s)) ?? 1;
    set([...rows, { scale, width: null, height: null, format: "png", suffix: scale === 1 ? "" : `@${scale}x` }]);
  }

  function update(i: number, change: Partial<ExportSetting>) {
    set(rows.map((r, k) => (k === i ? { ...r, ...change } : r)));
  }

  function formatItems(i: number): MenuItem[] {
    return FORMATS.map((f) => ({ label: f.toUpperCase(), checked: rows[i]?.format === f, action: () => update(i, { format: f }) }));
  }

  async function run() {
    if (busy) return;
    busy = true;
    try {
      const list = rows.length ? rows : [{ scale: 1, width: null, height: null, format: "png" as const, suffix: "" }];
      let done = 0;
      for (const e of list) if (await exportLayer(n, e)) done++;
      if (done) editor.notify(`Exported ${done} file${done === 1 ? "" : "s"}`);
    } catch (e) {
      editor.notify(`Export failed: ${e}`);
    } finally {
      busy = false;
    }
  }
</script>

<div class="section-head">
  <span class="section-title">Export</span>
  <button class="icon-btn" title="Add export settings" onclick={add}><Plus size={14} strokeWidth={1.7} /></button>
</div>
{#each rows as r, i (i)}
  <div class="row">
    <input
      class="size"
      value={sizeLabel(r)}
      title="Scale (2x) or fixed width/height (512w, 300h)"
      spellcheck="false"
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
      }}
      onblur={(e) => {
        const v = parseSize((e.target as HTMLInputElement).value);
        if (v) update(i, v);
      }}
    />
    <input
      class="suffix"
      value={r.suffix}
      placeholder="Suffix"
      spellcheck="false"
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
      }}
      onblur={(e) => update(i, { suffix: (e.target as HTMLInputElement).value })}
    />
    <div class="anchor">
      <button class="select" data-menu-trigger onclick={() => (formatMenu = formatMenu === i ? null : i)}>
        {r.format.toUpperCase()}<ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={formatMenu === i ? formatItems(i) : []} open={formatMenu === i} align="right" onclose={() => (formatMenu = null)} />
    </div>
    <button class="icon-btn" title="Remove" onclick={() => set(rows.filter((_, k) => k !== i))}><Minus size={14} strokeWidth={1.7} /></button>
  </div>
{/each}
<div class="pad">
  <button class="export" disabled={busy} onclick={run}>{busy ? "Exporting…" : `Export ${n.name}`}</button>
</div>

<style>
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .row {
    display: grid;
    grid-template-columns: 56px 1fr 64px 24px;
    gap: 6px;
    align-items: center;
    padding: 0 8px 6px 16px;
  }
  input {
    height: 24px;
    min-width: 0;
    padding: 0 6px;
    background: var(--bg-secondary);
    border: 1px solid transparent;
    border-radius: var(--radius);
    outline: none;
  }
  input:focus {
    border-color: var(--brand-strong);
  }
  .anchor {
    position: relative;
  }
  .select {
    display: flex;
    align-items: center;
    justify-content: space-between;
    width: 100%;
    height: 24px;
    padding: 0 6px 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .pad {
    padding: 2px 16px 8px;
  }
  .export {
    width: 100%;
    height: 32px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    font-weight: 550;
  }
  .export:hover:not(:disabled) {
    background: var(--bg-secondary);
  }
</style>
