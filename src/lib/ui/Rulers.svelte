<script lang="ts">
  import { editor } from "../editor.svelte";

  // Figma's rulers (Shift+R): ticks in canvas units along the top and left edges, the
  // selection's extent highlighted, and guides dragged out of them (drag a marker back onto
  // its ruler to remove it).

  const SIZE = 20;
  let { width, height }: { width: number; height: number } = $props();

  let top = $state<HTMLCanvasElement>();
  let left = $state<HTMLCanvasElement>();
  let guides = $state<{ vertical: boolean; at: number }[]>([]);
  let dragging = $state<{ index: number; vertical: boolean; at: number } | null>(null);

  $effect(() => {
    void editor.view;
    void editor.layers;
    guides = editor.engine?.guides() ?? [];
  });

  function step(zoom: number) {
    for (const s of [1, 2, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]) if (s * zoom >= 60) return s;
    return 50000;
  }

  function draw(c: HTMLCanvasElement | undefined, horizontal: boolean) {
    if (!c) return;
    const dpr = window.devicePixelRatio || 1;
    const len = horizontal ? width : height;
    c.width = Math.round((horizontal ? len : SIZE) * dpr);
    c.height = Math.round((horizontal ? SIZE : len) * dpr);
    const g = c.getContext("2d");
    if (!g) return;
    g.scale(dpr, dpr);
    const { zoom, x, y } = editor.view;
    const origin = horizontal ? x : y;
    g.fillStyle = "#2c2c2c";
    g.fillRect(0, 0, horizontal ? len : SIZE, horizontal ? SIZE : len);
    // The selection's extent.
    const sel = editor.selection;
    if (sel.length) {
      const lo = Math.min(...sel.map((n) => (horizontal ? n.x : n.y)));
      const hi = Math.max(...sel.map((n) => (horizontal ? n.x + n.w : n.y + n.h)));
      g.fillStyle = "rgba(13, 153, 255, 0.25)";
      const a = (lo - origin) * zoom;
      const b = (hi - origin) * zoom;
      if (horizontal) g.fillRect(a, 0, b - a, SIZE);
      else g.fillRect(0, a, SIZE, b - a);
    }
    const s = step(zoom);
    const minor = s / 10;
    const first = Math.floor(origin / minor) * minor;
    g.strokeStyle = "#5a5a5a";
    g.fillStyle = "#9a9a9a";
    g.font = "9px Inter, sans-serif";
    g.lineWidth = 1;
    g.beginPath();
    for (let v = first; (v - origin) * zoom < len; v += minor) {
      const p = Math.round((v - origin) * zoom) + 0.5;
      const major = Math.abs(v / s - Math.round(v / s)) < 1e-6;
      const half = Math.abs(v / (s / 2) - Math.round(v / (s / 2))) < 1e-6;
      const t = major ? 8 : half ? 5 : minor * zoom >= 6 ? 3 : 0;
      if (!t) continue;
      if (horizontal) {
        g.moveTo(p, SIZE);
        g.lineTo(p, SIZE - t);
      } else {
        g.moveTo(SIZE, p);
        g.lineTo(SIZE - t, p);
      }
      if (major) {
        const label = String(Math.round(v));
        if (horizontal) g.fillText(label, p + 3, 10);
        else {
          g.save();
          g.translate(10, p - 3);
          g.rotate(-Math.PI / 2);
          g.fillText(label, 0, 0);
          g.restore();
        }
      }
    }
    g.stroke();
    // Guide markers.
    g.fillStyle = "#f24822";
    for (const gd of guides) {
      if (gd.vertical !== horizontal) continue;
      const p = (gd.at - origin) * zoom;
      if (horizontal) g.fillRect(p - 1, 0, 2, SIZE);
      else g.fillRect(0, p - 1, SIZE, 2);
    }
  }

  $effect(() => {
    void editor.view;
    void editor.selection;
    void guides;
    void width;
    void height;
    draw(top, true);
    draw(left, false);
  });

  /** World position under a pointer along the ruler's axis. */
  function world(e: PointerEvent, vertical: boolean) {
    const host = top?.parentElement?.getBoundingClientRect();
    if (!host) return 0;
    const { zoom, x, y } = editor.view;
    return vertical ? x + (e.clientX - host.left) / zoom : y + (e.clientY - host.top) / zoom;
  }

  function save(list: { vertical: boolean; at: number }[]) {
    editor.engine?.setGuides(list);
    guides = list;
    editor.invalidate();
  }

  function start(e: PointerEvent, fromTop: boolean) {
    const tol = 4 / editor.view.zoom;
    const xw = world(e, true);
    const yw = world(e, false);
    // Markers: the top ruler shows vertical guides (by x), the left one horizontal guides (by y).
    let index = guides.findIndex((g) => (fromTop ? g.vertical && Math.abs(g.at - xw) <= tol : !g.vertical && Math.abs(g.at - yw) <= tol));
    const list = [...guides];
    if (index < 0) {
      // Dragging out of the top ruler makes a horizontal guide, out of the left one a vertical.
      const vertical = !fromTop;
      list.push({ vertical, at: Math.round(vertical ? xw : yw) });
      index = list.length - 1;
    }
    guides = list;
    dragging = { index, vertical: list[index].vertical, at: list[index].at };
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
  }

  function move(e: PointerEvent) {
    if (!dragging) return;
    const at = Math.round(world(e, dragging.vertical));
    dragging = { ...dragging, at };
    guides = guides.map((g, i) => (i === dragging?.index ? { ...g, at } : g));
    editor.engine?.setGuides(guides);
    editor.invalidate();
  }

  function end(e: PointerEvent) {
    if (!dragging) return;
    const host = top?.parentElement?.getBoundingClientRect();
    // Dropped back onto a ruler: the guide goes away.
    const onRuler = host && (e.clientY - host.top < SIZE || e.clientX - host.left < SIZE);
    const list = onRuler ? guides.filter((_, i) => i !== dragging?.index) : guides;
    dragging = null;
    save(list);
  }
</script>

<canvas
  class="ruler top"
  bind:this={top}
  style="width:{width}px; height:{SIZE}px"
  onpointerdown={(e) => start(e, true)}
  onpointermove={move}
  onpointerup={end}
></canvas>
<canvas
  class="ruler left"
  bind:this={left}
  style="width:{SIZE}px; height:{height}px"
  onpointerdown={(e) => start(e, false)}
  onpointermove={move}
  onpointerup={end}
></canvas>
<div class="corner"></div>
{#if dragging}
  <div class="tip" style={dragging.vertical ? `left:${(dragging.at - editor.view.x) * editor.view.zoom + 6}px; top:${SIZE + 6}px` : `top:${(dragging.at - editor.view.y) * editor.view.zoom + 6}px; left:${SIZE + 6}px`}>
    {dragging.vertical ? "X" : "Y"} {dragging.at}
  </div>
{/if}

<style>
  .ruler {
    position: absolute;
    z-index: 5;
    cursor: default;
  }
  .ruler.top {
    top: 0;
    left: 0;
    cursor: ns-resize;
  }
  .ruler.left {
    top: 0;
    left: 0;
    cursor: ew-resize;
  }
  .corner {
    position: absolute;
    top: 0;
    left: 0;
    width: 20px;
    height: 20px;
    z-index: 6;
    background: #2c2c2c;
    border-right: 1px solid #3a3a3a;
    border-bottom: 1px solid #3a3a3a;
  }
  .tip {
    position: absolute;
    z-index: 7;
    padding: 2px 6px;
    border-radius: 4px;
    background: #f24822;
    color: #fff;
    font-size: 11px;
    pointer-events: none;
  }
</style>
