<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import { onMount } from "svelte";
  import { editor } from "../editor.svelte";
  import type { Action, Hotspot, OverlayPosition, Reaction, Scroll, VarValue } from "../engine";

  // Presentation view: plays the prototype. Screens are rendered by the engine; hotspots are
  // transparent buttons over them. Transitions are CSS animations.

  let { start }: { start: number } = $props();

  /** `cw`×`ch`: the content size (larger than the screen when it scrolls). */
  type Spot = Hotspot & { instance: number | null };
  type Shown = { id: number; url: string; w: number; h: number; cw: number; ch: number; scroll: Scroll; fill: string; spots: Spot[] };
  type Overlay = Shown & { position: OverlayPosition; closeOutside: boolean; dim: boolean };
  let current = $state<Shown | null>(null);
  /** Open overlays, bottom first. */
  let overlays = $state<Overlay[]>([]);
  /** Variables the prototype set ("Set variable"); they last across screens. */
  let vars: Record<string, VarValue> = {};
  let scroller = $state<HTMLDivElement>();
  let previous = $state<Shown | null>(null);
  let transition = $state<{ kind: string; duration: number } | null>(null);
  let history: number[] = [];
  let stage = $state<HTMLDivElement>();
  let size = $state({ w: window.innerWidth, h: window.innerHeight });
  let flash = $state(false);
  let timers: number[] = [];

  const screens = editor.engine?.screens() ?? [];
  const scale = $derived(current ? Math.min((size.w - 96) / current.w, (size.h - 96) / current.h, 2) : 1);

  function render(id: number): Shown | null {
    const engine = editor.engine;
    if (!engine) return null;
    const info = screens.find((s) => s.id === id);
    if (!info) return null;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    // With "Change to" variant changes applied; the whole content when the screen scrolls.
    const shot = engine.presentCapture(id, dpr);
    if (!shot) return null;
    return { id, w: info.w, h: info.h, scroll: info.scroll ?? "none", fill: info.fill ?? "#ffffff", spots: shot.spots, url: shot.url, cw: shot.area.w, ch: shot.area.h };
  }

  function show(id: number, r?: Reaction, back = false) {
    const next = render(id);
    if (!next) return;
    if (current && !back) history.push(current.id);
    // A new screen starts with every instance as designed (variables stay) and no overlays.
    if (current?.id !== id) {
      hovered.clear();
      hoverSpots = [];
      resetSwaps();
      overlays = [];
    }
    previous = r && r.transition !== "instant" ? current : null;
    transition = r && r.transition !== "instant" ? { kind: r.transition, duration: r.duration } : null;
    current = next;
    schedule();
    if (transition) setTimeout(() => ((previous = null), (transition = null)), transition.duration + 30);
  }

  /** Instances back to their own variants; the prototype's variables are kept. */
  function resetSwaps() {
    const engine = editor.engine;
    if (!engine) return;
    engine.presentReset();
    for (const [id, v] of Object.entries(vars)) engine.presentSetVar(id, v);
  }

  function schedule() {
    timers.forEach(clearTimeout);
    timers = [];
    for (const s of [...(current?.spots ?? []), ...overlays.flatMap((o) => o.spots)]) {
      for (const r of s.reactions) {
        if (r.trigger.type === "after-delay") timers.push(window.setTimeout(() => run(r, s), r.trigger.ms));
      }
    }
  }

  /** Re-renders the shown screen and its overlays in place (after a variant or variable change). */
  function refresh() {
    if (!current) return;
    const next = render(current.id);
    if (next) current = next;
    overlays = overlays.map((o) => {
      const r = render(o.id);
      return r ? { ...o, ...r } : o;
    });
    schedule();
  }

  function same(a: VarValue | undefined, b: VarValue) {
    if (!a) return false;
    if (a.kind === "number" || b.kind === "number") return Number(a.value) === Number(b.value);
    return a.kind === b.kind && a.value === b.value;
  }

  function exec(a: Action, s?: Spot, r?: Reaction) {
    const engine = editor.engine;
    if (!engine) return;
    switch (a.type) {
      case "change-to": {
        const inst = s?.instance ?? null;
        if (inst === null) return;
        engine.presentSwap(inst, a.to);
        refresh();
        break;
      }
      case "navigate":
        show(a.to, r);
        break;
      case "back":
        if (overlays.length) overlays = overlays.slice(0, -1);
        else back();
        break;
      case "open-url":
        window.open(a.url, "_blank");
        break;
      case "open-overlay": {
        const o = render(a.to);
        if (o) overlays = [...overlays, { ...o, position: a.position ?? "center", closeOutside: a.closeOutside ?? true, dim: a.dim ?? false }];
        schedule();
        break;
      }
      case "close-overlay":
        overlays = overlays.slice(0, -1);
        break;
      case "set-variable":
        vars = { ...vars, [a.variable]: a.value };
        engine.presentSetVar(a.variable, a.value);
        refresh();
        break;
      case "conditional": {
        const value = vars[a.variable] ?? engine.presentVars()[a.variable];
        for (const next of same(value, a.equals) ? a.then : a.otherwise) exec(next, s, r);
        break;
      }
      case "scroll-to": {
        const node = engine.agent("get_node", { nodeId: String(a.to) });
        const screen = current ? engine.agent("get_node", { nodeId: String(current.id) }) : null;
        const n = node.ok ? (node.result as { absX: number; absY: number }) : null;
        const sc = screen?.ok ? (screen.result as { absX: number; absY: number }) : null;
        if (n && sc && scroller) scroller.scrollTo({ left: (n.absX - sc.absX) * scale, top: (n.absY - sc.absY) * scale, behavior: "smooth" });
        break;
      }
    }
  }

  function run(r: Reaction, s?: Spot) {
    exec(r.action, s, r);
  }

  /** Where an overlay sits on the device, in scaled pixels. */
  function place(o: Overlay) {
    if (!current) return "";
    const W = current.w * scale;
    const H = current.h * scale;
    const w = o.w * scale;
    const h = o.h * scale;
    const [x, y] =
      o.position === "top" ? [(W - w) / 2, 0] : o.position === "bottom" ? [(W - w) / 2, H - h] : o.position === "left" ? [0, (H - h) / 2] : o.position === "right" ? [W - w, (H - h) / 2] : [(W - w) / 2, (H - h) / 2];
    return `left:${x}px; top:${y}px; width:${w}px; height:${h}px`;
  }

  function back() {
    const id = history.pop();
    if (id !== undefined) show(id, undefined, true);
  }

  function step(dir: number) {
    const i = screens.findIndex((s) => s.id === current?.id);
    const next = screens[(i + dir + screens.length) % screens.length];
    if (next) show(next.id);
  }

  function click(s: Spot) {
    const r = s.reactions.find((x) => x.trigger.type === "on-click");
    if (r) run(r, s);
  }

  /** Instances currently swapped by "while hovering", to restore when the pointer leaves. Their
   * hotspot stays (the hover variant may have none) so the pointer can leave it. */
  let hovered = new Map<number, number>();
  let hoverSpots = $state<Spot[]>([]);
  const spots = $derived.by(() => {
    const list = current?.spots ?? [];
    return [...list, ...hoverSpots.filter((h) => !list.some((s) => s.instance === h.instance))];
  });

  function hover(s: Spot, on: boolean) {
    const r = s.reactions.find((x) => x.trigger.type === "on-hover");
    if (r?.action.type === "change-to" && s.instance !== null) {
      if (on && !hovered.has(s.instance)) {
        hovered.set(s.instance, r.action.to);
        hoverSpots = [...hoverSpots, s];
        run(r, s);
      } else if (!on && hovered.has(s.instance)) {
        leave(s.instance);
      }
      return;
    }
    if (!on && s.instance !== null && hovered.has(s.instance)) {
      leave(s.instance);
      return;
    }
    if (!r) return;
    if (on) run(r, s);
    else back();
  }

  /** Leaving a hovered instance: back to its own variant. */
  function leave(instance: number) {
    hovered.delete(instance);
    hoverSpots = hoverSpots.filter((h) => h.instance !== instance);
    editor.engine?.presentSwap(instance, -1);
    refresh();
  }

  function missed() {
    // Figma flashes the hotspots when you click somewhere without one.
    flash = true;
    setTimeout(() => (flash = false), 350);
  }

  function key(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") editor.presenting = null;
    else if (e.key === "ArrowRight") step(1);
    else if (e.key === "ArrowLeft") step(-1);
    else if (e.key === "Backspace") back();
    else if (e.key === "r" || e.key === "R") show(start);
  }

  onMount(() => {
    editor.engine?.presentReset();
    vars = {};
    show(start);
    const resize = () => (size = { w: window.innerWidth, h: window.innerHeight });
    window.addEventListener("resize", resize);
    window.addEventListener("keydown", key, true);
    stage?.focus();
    return () => {
      timers.forEach(clearTimeout);
      editor.engine?.presentReset();
      window.removeEventListener("resize", resize);
      window.removeEventListener("keydown", key, true);
    };
  });

  const enter = (kind: string) =>
    ({
      dissolve: "fade",
      "smart-animate": "fade",
      "slide-left": "from-right",
      "slide-right": "from-left",
      "slide-up": "from-bottom",
      "slide-down": "from-top",
      push: "from-right",
    })[kind] ?? "fade";
</script>

<div class="present" bind:this={stage} tabindex="-1" role="dialog" aria-label="Presentation">
  <header>
    <span class="name">{current ? (screens.find((s) => s.id === current?.id)?.name ?? "") : ""}</span>
    <span class="spacer"></span>
    <button class="icon-btn" title="Previous screen (←)" onclick={() => step(-1)}><ChevronLeft size={16} strokeWidth={1.8} /></button>
    <span class="faint count">{screens.findIndex((s) => s.id === current?.id) + 1} / {screens.length}</span>
    <button class="icon-btn" title="Next screen (→)" onclick={() => step(1)}><ChevronRight size={16} strokeWidth={1.8} /></button>
    <button class="icon-btn" title="Restart (R)" onclick={() => show(start)}><RotateCcw size={15} strokeWidth={1.8} /></button>
    <button class="icon-btn" title="Close (Esc)" onclick={() => (editor.presenting = null)}><X size={16} strokeWidth={1.8} /></button>
  </header>
  <div class="viewport">
    {#if current}
      <div class="device" style="width:{current.w * scale}px; height:{current.h * scale}px">
        {#if previous && transition}
          <img class="screen" src={previous.url} alt="" style="animation: leave {transition.duration}ms ease forwards" />
        {/if}
        {#key current.id}
          {@const sx = current.scroll === "horizontal" || current.scroll === "both"}
          {@const sy = current.scroll === "vertical" || current.scroll === "both"}
          <div
            bind:this={scroller}
            class="screen-wrap"
            style="overflow-x:{sx ? 'auto' : 'hidden'}; overflow-y:{sy ? 'auto' : 'hidden'}; background:{current.scroll !== 'none' ? current.fill : 'transparent'};{transition ? ` animation: ${enter(transition.kind)} ${transition.duration}ms ease` : ''}"
          >
            <div class="content" style="width:{current.cw * scale}px; height:{current.ch * scale}px">
              <button class="screen-btn" aria-label="Screen" onclick={missed}>
                <img class="screen" src={current.url} alt={screens.find((s) => s.id === current?.id)?.name} draggable="false" />
              </button>
              {#each spots as s (s.instance !== null ? `i${s.instance}` : `n${s.id}`)}
                <button
                  class="spot"
                  class:flash
                  title={s.name}
                  style="left:{s.x * scale}px; top:{s.y * scale}px; width:{s.w * scale}px; height:{s.h * scale}px"
                  onclick={() => click(s)}
                  onpointerenter={() => hover(s, true)}
                  onpointerleave={() => hover(s, false)}
                ></button>
              {/each}
            </div>
          </div>
        {/key}
        {#each overlays as o, k (k)}
          <button class="backdrop" class:dim={o.dim} aria-label="Close overlay" onclick={() => o.closeOutside && (overlays = overlays.slice(0, k))}></button>
          <div class="overlay" style={place(o)}>
            <img class="screen" src={o.url} alt={screens.find((s) => s.id === o.id)?.name} draggable="false" />
            {#each o.spots as s (s.instance !== null ? `i${s.instance}` : `n${s.id}`)}
              <button
                class="spot"
                class:flash
                title={s.name}
                style="left:{s.x * scale}px; top:{s.y * scale}px; width:{s.w * scale}px; height:{s.h * scale}px"
                onclick={() => click(s)}
                onpointerenter={() => hover(s, true)}
                onpointerleave={() => hover(s, false)}
              ></button>
            {/each}
          </div>
        {/each}
      </div>
    {:else}
      <p class="faint">Nothing to present. Add a frame to the page.</p>
    {/if}
  </div>
</div>

<style>
  .present {
    position: fixed;
    inset: 0;
    z-index: 300;
    display: flex;
    flex-direction: column;
    background: #121212;
    outline: none;
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 44px;
    padding: 0 8px 0 16px;
    color: #eee;
  }
  .name {
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .count {
    min-width: 44px;
    text-align: center;
    font-variant-numeric: tabular-nums;
  }
  .viewport {
    flex: 1;
    min-height: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .device {
    position: relative;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
  }
  .screen-wrap {
    position: absolute;
    inset: 0;
    scrollbar-width: none;
  }
  .content {
    position: relative;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    padding: 0;
    background: transparent;
    cursor: default;
    animation: fade 160ms ease;
  }
  .backdrop.dim {
    background: rgba(0, 0, 0, 0.45);
  }
  .overlay {
    position: absolute;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.35);
    animation: fade 160ms ease;
  }
  .screen-btn {
    position: absolute;
    inset: 0;
    padding: 0;
    cursor: default;
  }
  .screen {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    user-select: none;
  }
  .spot {
    position: absolute;
    cursor: pointer;
    background: transparent;
    border-radius: 2px;
  }
  .spot.flash {
    background: rgba(13, 153, 255, 0.35);
    outline: 1px solid rgba(13, 153, 255, 0.9);
  }
  @keyframes -global-fade {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  @keyframes -global-leave {
    from { opacity: 1; }
    to { opacity: 0; }
  }
  @keyframes -global-from-right {
    from { transform: translateX(100%); }
    to { transform: none; }
  }
  @keyframes -global-from-left {
    from { transform: translateX(-100%); }
    to { transform: none; }
  }
  @keyframes -global-from-bottom {
    from { transform: translateY(100%); }
    to { transform: none; }
  }
  @keyframes -global-from-top {
    from { transform: translateY(-100%); }
    to { transform: none; }
  }
</style>
