<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Pipette from "@lucide/svelte/icons/pipette";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import { untrack } from "svelte";
  import { argbAlpha, argbCss, argbHex, hexToRgb, hslToRgb, hsvToRgb, packArgb, parseHex, rgbToHex, rgbToHsl, rgbToHsv, type Hsv } from "../color";
  import type { Gradient, GradientKind, ImageAdjustKey, ImageMode, Paint } from "../engine";

  // Figma's colour picker (Solid paint): saturation/brightness square, hue and opacity sliders,
  // a value row in Hex / RGB / HSL / HSB, the eyedropper and the document's colours.

  let {
    hex,
    alpha,
    swatches = [],
    top,
    right = 252,
    onchange,
    onclose,
    paint = undefined,
    onpaint = undefined,
    imageUrl = null,
    onimage = undefined,
  }: {
    hex: string;
    /** 0–100 */
    alpha: number;
    swatches?: string[];
    /** Screen y to line the picker up with its swatch. */
    top: number;
    /** Distance from the window's right edge (left of the properties panel by default). */
    right?: number;
    onchange: (hex: string, alpha: number) => void;
    onclose: () => void;
    /** Fill pickers: the current gradient or image (null for a solid colour). */
    paint?: Paint | null;
    /** Enables the Gradient and Image types. */
    onpaint?: (paint: Paint | null) => void;
    /** Preview of the image fill. */
    imageUrl?: string | null;
    /** Asks for an image file (Image type). */
    onimage?: () => void;
  } = $props();

  const GRADIENTS: [GradientKind, string][] = [
    ["linear", "Linear"],
    ["radial", "Radial"],
    ["angular", "Angular"],
    ["diamond", "Diamond"],
  ];
  const ADJUST: [ImageAdjustKey, string][] = [
    ["exposure", "Exposure"],
    ["contrast", "Contrast"],
    ["saturation", "Saturation"],
    ["temperature", "Temperature"],
    ["tint", "Tint"],
    ["highlights", "Highlights"],
    ["shadows", "Shadows"],
  ];
  const MODES: [ImageMode, string][] = [
    ["fill", "Fill"],
    ["fit", "Fit"],
    ["crop", "Crop"],
    ["tile", "Tile"],
  ];
  const grad = $derived(paint?.type === "gradient" ? paint : null);
  const image = $derived(paint?.type === "image" ? paint : null);
  let stopIndex = $state(0);
  let bar = $state<HTMLDivElement>();

  /** The gradient as CSS, left to right, for the stop bar and swatches. */
  function stopsCss(g: Gradient) {
    const stops = [...g.stops].sort((x, y) => x.at - y.at);
    return `linear-gradient(90deg, ${stops.map((s) => `${argbCss(s.color)} ${(s.at * 100).toFixed(1)}%`).join(", ")})`;
  }

  function selectStop(i: number) {
    const g = grad;
    if (!g?.stops[i]) return;
    stopIndex = i;
    hsv = rgbToHsv(hexToRgb(argbHex(g.stops[i].color)));
    a = argbAlpha(g.stops[i].color);
  }

  function setGradient(change: Partial<Gradient>) {
    if (grad && onpaint) onpaint({ ...grad, ...change });
  }

  function toGradient(kind: GradientKind) {
    if (!onpaint) return;
    if (grad) return setGradient({ kind, ...(kind === "linear" || grad.kind === "linear" ? handles(kind) : {}) });
    // From a solid colour: that colour fading to transparent, as Figma does.
    const from = packArgb(current, a);
    const to = packArgb(current, 0);
    onpaint({ type: "gradient", kind, stops: [{ at: 0, color: from }, { at: 1, color: to }], ...handles(kind), width: 1 });
    stopIndex = 0;
  }

  function handles(kind: GradientKind): Pick<Gradient, "from" | "to"> {
    return kind === "linear" ? { from: [0.5, 0], to: [0.5, 1] } : { from: [0.5, 0.5], to: [0.5, 1] };
  }

  /** CSS-style angle of a linear gradient (0 = up, 90 = right). */
  const angle = $derived.by(() => {
    if (!grad) return 0;
    const dx = grad.to[0] - grad.from[0];
    const dy = grad.to[1] - grad.from[1];
    return Math.round(((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360);
  });

  function setAngle(deg: number) {
    if (!grad) return;
    const r = (deg * Math.PI) / 180;
    const cx = (grad.from[0] + grad.to[0]) / 2;
    const cy = (grad.from[1] + grad.to[1]) / 2;
    const len = Math.hypot(grad.to[0] - grad.from[0], grad.to[1] - grad.from[1]) / 2 || 0.5;
    const dx = Math.sin(r) * len;
    const dy = -Math.cos(r) * len;
    setGradient({ from: [cx - dx, cy - dy], to: [cx + dx, cy + dy] });
  }

  function flipStops() {
    if (grad) setGradient({ stops: grad.stops.map((s) => ({ ...s, at: 1 - s.at })) });
  }

  function barDown(e: PointerEvent) {
    const g = grad;
    if (!g || !bar) return;
    const r = bar.getBoundingClientRect();
    const at = Math.min(1, Math.max(0, (e.clientX - r.left) / r.width));
    let i = g.stops.findIndex((s) => Math.abs(s.at - at) * r.width < 7);
    let stops = g.stops;
    if (i < 0) {
      // A click between stops adds one there, in the colour of the gradient at that point.
      const sorted = [...stops].sort((x, y) => x.at - y.at);
      const next = sorted.find((s) => s.at >= at) ?? sorted[sorted.length - 1];
      stops = [...stops, { at, color: next.color }];
      i = stops.length - 1;
      onpaint?.({ ...g, stops });
    }
    selectStop(i);
    stopIndex = i;
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      const cur = grad;
      if (!cur) return;
      const x = Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width));
      onpaint?.({ ...cur, stops: cur.stops.map((s, k) => (k === stopIndex ? { ...s, at: Math.round(x * 1000) / 1000 } : s)) });
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", () => target.removeEventListener("pointermove", move), { once: true });
  }

  function removeStop() {
    if (!grad || grad.stops.length <= 2) return;
    const stops = grad.stops.filter((_, k) => k !== stopIndex);
    onpaint?.({ ...grad, stops });
    selectStop(0);
  }

  let hsv = $state<Hsv>(untrack(() => rgbToHsv(hexToRgb(hex))));
  let a = $state(untrack(() => alpha));
  let format = $state<"hex" | "rgb" | "hsl" | "hsb">("hex");
  let formatOpen = $state(false);
  let el = $state<HTMLDivElement>();

  const current = $derived(rgbToHex(hsvToRgb(hsv)));
  const pure = $derived(rgbToHex(hsvToRgb({ h: hsv.h, s: 1, v: 1 })));
  const y = $derived(Math.max(8, Math.min(top - 40, window.innerHeight - 460)));

  function emit() {
    if (grad && onpaint) {
      const color = packArgb(current, a);
      onpaint({ ...grad, stops: grad.stops.map((s, k) => (k === stopIndex ? { ...s, color } : s)) });
      return;
    }
    onchange(current, Math.round(a));
  }

  // Editing a gradient: the colour controls show the selected stop.
  $effect(() => {
    if (grad && grad.stops[stopIndex]) {
      const c = grad.stops[stopIndex].color;
      untrack(() => {
        if (packArgb(current, a) !== c) {
          hsv = rgbToHsv(hexToRgb(argbHex(c)));
          a = argbAlpha(c);
        }
      });
    }
  });

  function drag(e: PointerEvent, apply: (fx: number, fy: number) => void) {
    const target = e.currentTarget as HTMLElement;
    const r = target.getBoundingClientRect();
    const move = (ev: PointerEvent) => {
      apply(
        Math.min(1, Math.max(0, (ev.clientX - r.left) / r.width)),
        Math.min(1, Math.max(0, (ev.clientY - r.top) / r.height)),
      );
      emit();
    };
    move(e);
    target.setPointerCapture(e.pointerId);
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", () => target.removeEventListener("pointermove", move), { once: true });
  }

  function setHex(raw: string) {
    const h = parseHex(raw);
    if (!h) return;
    hsv = rgbToHsv(hexToRgb(h));
    emit();
  }

  function values(): number[] {
    const rgb = hsvToRgb(hsv);
    if (format === "rgb") return rgb.map((v) => Math.round(v * 255));
    if (format === "hsl") {
      const [h, s, l] = rgbToHsl(rgb);
      return [Math.round(h), Math.round(s * 100), Math.round(l * 100)];
    }
    return [Math.round(hsv.h), Math.round(hsv.s * 100), Math.round(hsv.v * 100)];
  }

  function setValue(i: number, raw: string) {
    const v = Number(raw);
    if (!Number.isFinite(v)) return;
    const vals = values();
    vals[i] = v;
    if (format === "rgb") hsv = rgbToHsv(vals.map((x) => Math.min(255, Math.max(0, x)) / 255) as [number, number, number]);
    else if (format === "hsl") hsv = rgbToHsv(hslToRgb([((vals[0] % 360) + 360) % 360, vals[1] / 100, vals[2] / 100]));
    else hsv = { h: ((vals[0] % 360) + 360) % 360, s: Math.min(1, Math.max(0, vals[1] / 100)), v: Math.min(1, Math.max(0, vals[2] / 100)) };
    emit();
  }

  async function sample() {
    const Dropper = (window as unknown as { EyeDropper?: new () => { open(): Promise<{ sRGBHex: string }> } }).EyeDropper;
    if (!Dropper) return;
    try {
      const { sRGBHex } = await new Dropper().open();
      setHex(sRGBHex);
    } catch {
      // cancelled
    }
  }

  $effect(() => {
    const close = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node) && !(e.target as HTMLElement).closest("[data-picker-trigger]")) onclose();
    };
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onclose();
      }
    };
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", esc, true);
    return () => {
      window.removeEventListener("pointerdown", close, true);
      window.removeEventListener("keydown", esc, true);
    };
  });

  const stop = (e: KeyboardEvent) => e.stopPropagation();
</script>

<div class="picker" bind:this={el} style="top:{y}px; right:{right}px" role="dialog" aria-label="Color picker">
  <header>
    <span class="tab active">Custom</span>
    <span class="tab muted" title="Libraries are coming soon">Libraries</span>
    <span class="spacer"></span>
    <button class="icon-btn" title="Close" onclick={onclose}><X size={14} strokeWidth={1.8} /></button>
  </header>
  <div class="types">
    {#if onpaint}
      <button class="type" class:on={!paint} onclick={() => onpaint?.(null)}>Solid</button>
      <button class="type" class:on={!!grad} onclick={() => toGradient(grad?.kind ?? "linear")}>Gradient</button>
      <button class="type" class:on={!!image} onclick={() => onimage?.()}>Image</button>
    {:else}
      <span class="type on">Solid</span>
    {/if}
  </div>

  {#if image}
    <div class="image-preview" style={imageUrl ? `background-image:url('${imageUrl}')` : ""}>
      <button class="choose" onclick={() => onimage?.()}>Choose image…</button>
    </div>
    <div class="row-controls">
      <select class="select" value={image.mode} onchange={(e) => onpaint?.({ ...image, mode: (e.target as HTMLSelectElement).value as ImageMode })}>
        {#each MODES as [m, label] (m)}<option value={m}>{label}</option>{/each}
      </select>
      <span class="muted small">{image.width}×{image.height}</span>
      <input class="num pct" value={`${Math.round(image.opacity * 100)}%`} title="Image opacity"
        onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
        onblur={(e) => { const v = parseFloat((e.target as HTMLInputElement).value); if (Number.isFinite(v)) onpaint?.({ ...image, opacity: Math.min(100, Math.max(0, v)) / 100 }); }} />
    </div>
    <div class="adjust">
      {#each ADJUST as [key, label] (key)}
        {@const v = Math.round((image.adjust?.[key] ?? 0) * 100)}
        <label class="adjust-row">
          <span class="muted small">{label}</span>
          <input
            type="range"
            min="-100"
            max="100"
            value={v}
            title="{label} {v}"
            ondblclick={() => onpaint?.({ ...image, adjust: { ...image.adjust, [key]: 0 } })}
            onchange={(e) => onpaint?.({ ...image, adjust: { ...image.adjust, [key]: Number((e.target as HTMLInputElement).value) / 100 } })}
          />
          <span class="muted small adjust-value">{v}</span>
        </label>
      {/each}
    </div>
    {#if image.mode === "tile"}
      <div class="row-controls">
        <span class="muted small">Scale</span>
        <input class="num" value={`${Math.round(image.scale * 100)}%`}
          onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onblur={(e) => { const v = parseFloat((e.target as HTMLInputElement).value); if (Number.isFinite(v) && v > 0) onpaint?.({ ...image, scale: v / 100 }); }} />
      </div>
    {/if}
  {:else}
  {#if grad}
    <div class="row-controls">
      <select class="select" value={grad.kind} onchange={(e) => toGradient((e.target as HTMLSelectElement).value as GradientKind)}>
        {#each GRADIENTS as [k, label] (k)}<option value={k}>{label}</option>{/each}
      </select>
      {#if grad.kind === "linear"}
        <input class="num" value={`${angle}°`} title="Angle"
          onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onblur={(e) => { const v = parseFloat((e.target as HTMLInputElement).value); if (Number.isFinite(v)) setAngle(v); }} />
      {/if}
      <button class="icon-btn flip" title="Flip gradient" onclick={flipStops}>⇄</button>
      <button class="icon-btn flip" title="Remove stop" disabled={grad.stops.length <= 2} onclick={removeStop}>−</button>
    </div>
    <div class="stops" bind:this={bar} role="slider" tabindex="0" aria-label="Gradient stops" aria-valuenow={stopIndex}
      onpointerdown={barDown}
      onkeydown={(e) => { if (e.key === "Delete" || e.key === "Backspace") { e.stopPropagation(); removeStop(); } }}>
      <div class="stops-fill" style="background:{stopsCss(grad)}"></div>
      {#each grad.stops as s, i (i)}
        <span class="stop" class:on={i === stopIndex} style="left:{s.at * 100}%; background:{argbCss(s.color)}"></span>
      {/each}
    </div>
  {/if}

  <div class="sv" style="background-color:{pure}" role="slider" tabindex="-1" aria-label="Saturation and brightness" aria-valuenow={Math.round(hsv.s * 100)}
    onpointerdown={(e) => drag(e, (fx, fy) => (hsv = { ...hsv, s: fx, v: 1 - fy }))}>
    <div class="white"></div>
    <div class="black"></div>
    <span class="knob" style="left:{hsv.s * 100}%; top:{(1 - hsv.v) * 100}%; background:{current}"></span>
  </div>

  <div class="sliders">
    <button class="icon-btn" title="Sample color" onclick={sample}><Pipette size={14} strokeWidth={1.8} /></button>
    <div class="bars">
      <div class="bar hue" role="slider" tabindex="-1" aria-label="Hue" aria-valuenow={Math.round(hsv.h)}
        onpointerdown={(e) => drag(e, (fx) => (hsv = { ...hsv, h: Math.min(359.9, fx * 360) }))}>
        <span class="thumb" style="left:{(hsv.h / 360) * 100}%"></span>
      </div>
      <div class="bar alpha" role="slider" tabindex="-1" aria-label="Opacity" aria-valuenow={Math.round(a)}
        onpointerdown={(e) => drag(e, (fx) => (a = fx * 100))}>
        <div class="alpha-fill" style="background:linear-gradient(to right, transparent, {current})"></div>
        <span class="thumb" style="left:{a}%"></span>
      </div>
    </div>
  </div>

  <div class="values">
    <div class="anchor">
      <button class="format" onclick={() => (formatOpen = !formatOpen)}>{format === "hex" ? "Hex" : format.toUpperCase()}<ChevronDown size={10} strokeWidth={2} /></button>
      {#if formatOpen}
        <div class="formats">
          {#each ["hex", "rgb", "hsl", "hsb"] as f (f)}
            <button class:on={format === f} onclick={() => { format = f as typeof format; formatOpen = false; }}>{f === "hex" ? "Hex" : f.toUpperCase()}</button>
          {/each}
        </div>
      {/if}
    </div>
    {#if format === "hex"}
      <input class="wide" value={current.slice(1)} spellcheck="false" onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
        onblur={(e) => setHex((e.target as HTMLInputElement).value)} />
    {:else}
      {#each values() as v, i (i)}
        <input class="num" value={v} onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onblur={(e) => setValue(i, (e.target as HTMLInputElement).value)} />
      {/each}
    {/if}
    <input class="num pct" value={`${Math.round(a)}%`} onkeydown={(e) => { stop(e); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
      onblur={(e) => { const v = parseFloat((e.target as HTMLInputElement).value); if (Number.isFinite(v)) { a = Math.min(100, Math.max(0, v)); emit(); } }} />
  </div>

  {#if swatches.length && !image}
    <div class="swatches-head muted">On this page</div>
    <div class="swatches">
      {#each swatches as s (s)}
        <button class="swatch" style="background:{s}" title={s} aria-label={s} onclick={() => setHex(s)}></button>
      {/each}
    </div>
  {/if}
  {/if}
</div>

<style>
  .picker {
    position: fixed;
    z-index: 70;
    width: 240px;
    padding-bottom: 12px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 40px;
    padding: 0 8px 0 12px;
    border-bottom: 1px solid var(--border);
  }
  .tab {
    padding: 0 6px;
    height: 24px;
    line-height: 24px;
    border-radius: var(--radius);
    font-weight: 550;
  }
  .tab.active {
    background: var(--bg-secondary);
  }
  .spacer {
    flex: 1;
  }
  .types {
    display: flex;
    gap: 4px;
    padding: 8px 12px;
  }
  .type {
    padding: 0 8px;
    height: 22px;
    line-height: 22px;
    border-radius: var(--radius);
    color: var(--text-tertiary);
  }
  button.type:hover {
    color: var(--text);
  }
  .adjust {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px 12px 8px;
  }
  .adjust-row {
    display: grid;
    grid-template-columns: 76px 1fr 28px;
    align-items: center;
    gap: 8px;
  }
  .adjust-row input {
    width: 100%;
    accent-color: var(--brand-strong, #0d99ff);
  }
  .adjust-value {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
  .image-preview {
    position: relative;
    height: 160px;
    margin: 0 12px 8px;
    border-radius: 4px;
    background: repeating-conic-gradient(#3a3a3a 0 25%, #2c2c2c 0 50%) 0 0 / 16px 16px;
    background-size: cover;
    background-position: center;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .choose {
    padding: 0 10px;
    height: 26px;
    border-radius: var(--radius);
    background: rgba(0, 0, 0, 0.6);
    color: #fff;
    opacity: 0;
  }
  .image-preview:hover .choose,
  .image-preview:not([style*="url"]) .choose {
    opacity: 1;
  }
  .row-controls {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 12px 8px;
  }
  .select {
    flex: 1;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
    border: none;
  }
  .small {
    font-size: 11px;
  }
  .flip {
    font-size: 13px;
  }
  .stops {
    position: relative;
    height: 16px;
    margin: 4px 18px 12px;
    border-radius: 3px;
    background: repeating-conic-gradient(#666 0 25%, #999 0 50%) 0 0 / 8px 8px;
    cursor: copy;
    touch-action: none;
  }
  .stops-fill {
    position: absolute;
    inset: 0;
    border-radius: 3px;
  }
  .stop {
    position: absolute;
    top: -3px;
    width: 12px;
    height: 22px;
    margin-left: -6px;
    border-radius: 3px;
    box-shadow: 0 0 0 2px #fff, 0 1px 3px rgba(0, 0, 0, 0.6);
    cursor: grab;
  }
  .stop.on {
    box-shadow: 0 0 0 2px var(--brand-strong, #0d99ff), 0 1px 3px rgba(0, 0, 0, 0.6);
  }
  .type.on {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .sv {
    position: relative;
    height: 216px;
    margin: 0 12px;
    border-radius: 4px;
    overflow: hidden;
    cursor: crosshair;
    touch-action: none;
  }
  .white {
    position: absolute;
    inset: 0;
    background: linear-gradient(to right, #fff, rgba(255, 255, 255, 0));
  }
  .black {
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, #000, rgba(0, 0, 0, 0));
  }
  .knob {
    position: absolute;
    width: 14px;
    height: 14px;
    margin: -7px 0 0 -7px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.3), inset 0 0 0 1px rgba(0, 0, 0, 0.2);
    pointer-events: none;
  }
  .sliders {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 12px 12px 8px;
  }
  .bars {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  .bar {
    position: relative;
    height: 12px;
    border-radius: 6px;
    touch-action: none;
  }
  .hue {
    background: linear-gradient(to right, #f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
  }
  .alpha {
    background: repeating-conic-gradient(#808080 0 25%, #c0c0c0 0 50%) 0 0 / 8px 8px;
  }
  .alpha-fill {
    position: absolute;
    inset: 0;
    border-radius: 6px;
  }
  .thumb {
    position: absolute;
    top: 50%;
    width: 14px;
    height: 14px;
    margin: -7px 0 0 -7px;
    border: 2px solid #fff;
    border-radius: 50%;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.35);
    pointer-events: none;
  }
  .values {
    display: flex;
    gap: 4px;
    padding: 4px 12px 8px;
  }
  .anchor {
    position: relative;
  }
  .format {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 24px;
    padding: 0 6px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .formats {
    position: absolute;
    top: 28px;
    left: 0;
    z-index: 2;
    display: flex;
    flex-direction: column;
    padding: 4px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 8px;
    box-shadow: var(--shadow-menu);
  }
  .formats button {
    padding: 0 12px;
    height: 24px;
    text-align: left;
    border-radius: var(--radius);
  }
  .formats button:hover,
  .formats button.on {
    background: var(--brand);
  }
  .values input {
    height: 24px;
    min-width: 0;
    padding: 0 6px;
    background: var(--bg-secondary);
    border: 1px solid transparent;
    border-radius: var(--radius);
    outline: none;
    font-variant-numeric: tabular-nums;
  }
  .values input:focus {
    border-color: var(--brand-strong);
  }
  .wide {
    flex: 1;
    text-transform: uppercase;
  }
  .num {
    width: 38px;
    flex: 1;
  }
  .pct {
    width: 46px;
    flex: none;
  }
  .swatches-head {
    padding: 8px 12px 6px;
    border-top: 1px solid var(--border);
  }
  .swatches {
    display: grid;
    grid-template-columns: repeat(9, 1fr);
    gap: 6px;
    padding: 0 12px;
  }
  .swatch {
    aspect-ratio: 1;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.12);
  }
</style>
