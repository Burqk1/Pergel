<script lang="ts">
  import ColorPicker from "./ColorPicker.svelte";
  import { editor } from "../editor.svelte";

  // Figma's "Selection colors": every fill/stroke colour inside the selection; changing one
  // recolours it everywhere in the selection.

  const colors = $derived.by(() => {
    void editor.selection;
    return editor.engine?.selectionColors() ?? [];
  });
  let more = $state(false);
  let editing = $state<{ index: number; top: number; color: string; opacity: number } | null>(null);
  const shown = $derived(more ? colors : colors.slice(0, 6));

  function open(e: MouseEvent, index: number) {
    if (editing) return close();
    const c = colors[index];
    editor.beginLive();
    editing = { index, top: (e.currentTarget as HTMLElement).getBoundingClientRect().top, color: c.color, opacity: c.opacity };
  }

  function close() {
    editing = null;
    editor.endLive();
  }

  function change(hex: string, alpha: number) {
    const cur = editing;
    const engine = editor.engine;
    if (!cur || !engine) return;
    engine.replaceColor(cur.color, cur.opacity, hex, alpha);
    // Keep following the colour as it changes.
    cur.color = hex;
    cur.opacity = alpha;
    editor.refreshSelection();
    editor.invalidate();
  }
</script>

{#if colors.length > 1 || editor.selection.length > 1 || editor.selection.some((n) => n.children > 0)}
  <div class="divider"></div>
  <div class="section-head"><span class="section-title">Selection colors</span></div>
  {#each shown as c, i (c.color + c.opacity)}
    <div class="row">
      <button class="swatch" data-picker-trigger aria-label="Change {c.color}" style="background:{c.color}; opacity:{Math.max(c.opacity, 15) / 100}" onclick={(e) => open(e, i)}></button>
      <span class="hex">{c.color.replace("#", "")}</span>
      <span class="faint pct">{c.opacity}%</span>
    </div>
  {/each}
  {#if colors.length > 6}
    <button class="more" onclick={() => (more = !more)}>{more ? "Show fewer" : `See all ${colors.length} colors`}</button>
  {/if}
  {#if editing}
    <ColorPicker
      hex={editing.color}
      alpha={editing.opacity}
      top={editing.top}
      onchange={change}
      onclose={close}
    />
  {/if}
{/if}

<style>
  .divider {
    height: 1px;
    margin: 8px 0 4px;
    background: var(--border);
  }
  .section-head {
    display: flex;
    align-items: center;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 28px;
    padding: 0 16px;
  }
  .swatch {
    width: 16px;
    height: 16px;
    border-radius: 3px;
    flex: none;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }
  .hex {
    flex: 1;
    font-variant-numeric: tabular-nums;
  }
  .pct {
    font-variant-numeric: tabular-nums;
  }
  .more {
    margin: 2px 16px 6px;
    color: var(--text-brand);
  }
</style>
