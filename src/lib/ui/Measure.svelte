<script lang="ts">
  import { editor } from "../editor.svelte";

  // Figma's Alt-hover measurement: red distances from the selection to the hovered layer
  // (to its edges when the layer contains the selection).

  let { target }: { target: { x: number; y: number; w: number; h: number } | null } = $props();

  type Seg = { x1: number; y1: number; x2: number; y2: number; label: string };

  const segs = $derived.by((): Seg[] => {
    const sel = editor.selection;
    if (!target || !sel.length) return [];
    const ax = Math.min(...sel.map((n) => n.x));
    const ay = Math.min(...sel.map((n) => n.y));
    const a = { x: ax, y: ay, r: Math.max(...sel.map((n) => n.x + n.w)), b: Math.max(...sel.map((n) => n.y + n.h)) };
    const b = { x: target.x, y: target.y, r: target.x + target.w, b: target.y + target.h };
    const cx = (a.x + a.r) / 2;
    const cy = (a.y + a.b) / 2;
    const out: Seg[] = [];
    const fmt = (v: number) => String(Math.round(v * 100) / 100);
    const inside = a.x >= b.x && a.r <= b.r && a.y >= b.y && a.b <= b.b;
    if (inside) {
      out.push({ x1: b.x, y1: cy, x2: a.x, y2: cy, label: fmt(a.x - b.x) });
      out.push({ x1: a.r, y1: cy, x2: b.r, y2: cy, label: fmt(b.r - a.r) });
      out.push({ x1: cx, y1: b.y, x2: cx, y2: a.y, label: fmt(a.y - b.y) });
      out.push({ x1: cx, y1: a.b, x2: cx, y2: b.b, label: fmt(b.b - a.b) });
    } else {
      const midY = Math.max(a.y, b.y) < Math.min(a.b, b.b) ? (Math.max(a.y, b.y) + Math.min(a.b, b.b)) / 2 : cy;
      const midX = Math.max(a.x, b.x) < Math.min(a.r, b.r) ? (Math.max(a.x, b.x) + Math.min(a.r, b.r)) / 2 : cx;
      if (a.r <= b.x) out.push({ x1: a.r, y1: midY, x2: b.x, y2: midY, label: fmt(b.x - a.r) });
      else if (b.r <= a.x) out.push({ x1: b.r, y1: midY, x2: a.x, y2: midY, label: fmt(a.x - b.r) });
      if (a.b <= b.y) out.push({ x1: midX, y1: a.b, x2: midX, y2: b.y, label: fmt(b.y - a.b) });
      else if (b.b <= a.y) out.push({ x1: midX, y1: b.b, x2: midX, y2: a.y, label: fmt(a.y - b.b) });
    }
    return out.filter((s) => s.label !== "0");
  });

  const sx = (x: number) => (x - editor.view.x) * editor.view.zoom;
  const sy = (y: number) => (y - editor.view.y) * editor.view.zoom;
</script>

{#if target && segs.length}
  <svg class="measure" aria-hidden="true">
    <rect x={sx(target.x)} y={sy(target.y)} width={target.w * editor.view.zoom} height={target.h * editor.view.zoom} fill="none" stroke="#f24822" stroke-width="1" stroke-dasharray="3 3" />
    {#each segs as s, i (i)}
      <line x1={sx(s.x1)} y1={sy(s.y1)} x2={sx(s.x2)} y2={sy(s.y2)} stroke="#f24822" stroke-width="1" />
    {/each}
  </svg>
  {#each segs as s, i (i)}
    <span class="badge" style="left:{(sx(s.x1) + sx(s.x2)) / 2}px; top:{(sy(s.y1) + sy(s.y2)) / 2}px">{s.label}</span>
  {/each}
{/if}

<style>
  .measure {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 4;
    overflow: visible;
  }
  .badge {
    position: absolute;
    z-index: 4;
    transform: translate(-50%, -50%);
    padding: 1px 4px;
    border-radius: 3px;
    background: #f24822;
    color: #fff;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }
</style>
