<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Sliders from "@lucide/svelte/icons/sliders-horizontal";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import X from "@lucide/svelte/icons/x";
  import Grid from "@lucide/svelte/icons/grid-3x3";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import NumberField from "./NumberField.svelte";
  import ColorPicker from "./ColorPicker.svelte";
  import { editor } from "../editor.svelte";
  import type { EffectInfo, EffectType } from "../engine";

  const TYPES: [EffectType, string][] = [
    ["drop-shadow", "Drop shadow"],
    ["inner-shadow", "Inner shadow"],
    ["layer-blur", "Layer blur"],
    ["background-blur", "Background blur"],
  ];
  const DEFAULTS: Record<EffectType, EffectInfo> = {
    "drop-shadow": { type: "drop-shadow", color: "#000000", opacity: 25, x: 0, y: 4, blur: 4, spread: 0, visible: true },
    "inner-shadow": { type: "inner-shadow", color: "#000000", opacity: 25, x: 0, y: 4, blur: 4, spread: 0, visible: true },
    "layer-blur": { type: "layer-blur", color: "", opacity: 0, x: 0, y: 0, blur: 4, spread: 0, visible: true },
    "background-blur": { type: "background-blur", color: "", opacity: 0, x: 0, y: 0, blur: 4, spread: 0, visible: true },
  };

  const effects = $derived(editor.common((n) => n.effects));
  let typeMenu = $state<number | null>(null);
  let settings = $state<{ index: number; top: number } | null>(null);
  let picker = $state<number | null>(null);

  const isShadow = (e: EffectInfo) => e.type === "drop-shadow" || e.type === "inner-shadow";
  const label = (t: EffectType) => TYPES.find((x) => x[0] === t)?.[1] ?? t;

  function set(list: EffectInfo[]) {
    editor.patch({ effects: list });
  }

  function change(i: number, change: Partial<EffectInfo>, live = false) {
    if (!effects) return;
    const list = effects.map((e, k) => (k === i ? { ...e, ...change } : e));
    if (live) editor.livePatch({ effects: list });
    else set(list);
  }

  function summary(e: EffectInfo) {
    if (isShadow(e)) return `${e.opacity}% · ${e.x}, ${e.y} · ${e.blur}`;
    return `Uniform · ${e.blur}`;
  }

  function typeItems(i: number): MenuItem[] {
    return TYPES.map(([t, name]) => ({
      label: name,
      checked: effects?.[i]?.type === t,
      action: () => {
        const cur = effects?.[i];
        if (!cur) return;
        // Keep shared values when switching between shadows (or between blurs).
        const next = { ...DEFAULTS[t], ...(isShadow(cur) === isShadow(DEFAULTS[t]) ? cur : {}), type: t };
        change(i, next);
      },
    }));
  }

  function openSettings(e: MouseEvent, i: number) {
    if (settings?.index === i) return (settings = null);
    settings = { index: i, top: (e.currentTarget as HTMLElement).getBoundingClientRect().top };
  }

  $effect(() => {
    if (!settings) return;
    const close = (ev: PointerEvent) => {
      const t = ev.target as HTMLElement;
      if (!t.closest(".effect-settings") && !t.closest("[data-effect-trigger]") && !t.closest(".picker")) settings = null;
    };
    window.addEventListener("pointerdown", close, true);
    return () => window.removeEventListener("pointerdown", close, true);
  });
</script>

<div class="section-head">
  <span class="section-title">Effects</span>
  <span class="head-actions">
    <button class="icon-btn" title="Effect styles are coming soon" onclick={() => editor.notify("Effect styles are coming soon")}><Grid size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" title="Add effect" onclick={() => set([...(effects ?? []), DEFAULTS["drop-shadow"]])}>
      <Plus size={14} strokeWidth={1.7} />
    </button>
  </span>
</div>

{#if effects === null}
  <p class="hint faint">Mixed — click + to replace</p>
{:else}
  {#each effects as e, i (i)}
    <div class="row" class:off={!e.visible}>
      <button class="icon-btn" data-effect-trigger class:on={settings?.index === i} title="Effect settings" onclick={(ev) => openSettings(ev, i)}>
        <Sliders size={14} strokeWidth={1.7} />
      </button>
      <div class="anchor grow">
        <button class="select" data-menu-trigger onclick={() => (typeMenu = typeMenu === i ? null : i)}>
          <span class="name">{label(e.type)}</span>
          <span class="faint small">{summary(e)}</span>
          <ChevronDown size={10} strokeWidth={2} />
        </button>
        <Menu items={typeMenu === i ? typeItems(i) : []} open={typeMenu === i} onclose={() => (typeMenu = null)} />
      </div>
      <button class="icon-btn" title={e.visible ? "Hide effect" : "Show effect"} onclick={() => change(i, { visible: !e.visible })}>
        {#if e.visible}<Eye size={14} strokeWidth={1.7} />{:else}<EyeOff size={14} strokeWidth={1.7} />{/if}
      </button>
      <button class="icon-btn" title="Remove effect" onclick={() => set(effects.filter((_, k) => k !== i))}>
        <Minus size={14} strokeWidth={1.7} />
      </button>
    </div>
  {/each}
{/if}

{#if settings && effects?.[settings.index]}
  {@const i = settings.index}
  {@const e = effects[i]}
  <div class="effect-settings" style="top:{Math.min(settings.top - 20, window.innerHeight - 240)}px" role="dialog" aria-label="Effect settings">
    <header>
      <span class="title">{label(e.type)}</span>
      <button class="icon-btn" title="Close" onclick={() => (settings = null)}><X size={14} strokeWidth={1.8} /></button>
    </header>
    {#if isShadow(e)}
      <div class="grid">
        <NumberField label="X" value={e.x} onCommit={(v) => change(i, { x: v })} />
        <NumberField label="Y" value={e.y} onCommit={(v) => change(i, { y: v })} />
        <NumberField label="Blur" min={0} value={e.blur} onCommit={(v) => change(i, { blur: v })} />
        <NumberField label="Spread" value={e.spread} onCommit={(v) => change(i, { spread: v })} />
      </div>
      <div class="color">
        <button
          class="swatch"
          data-picker-trigger
          aria-label="Shadow colour"
          style="background:{e.color}; opacity:{Math.max(e.opacity, 15) / 100}"
          onclick={() => {
            if (picker === i) {
              picker = null;
              editor.endLive();
            } else {
              editor.beginLive();
              picker = i;
            }
          }}
        ></button>
        <span class="hex">{e.color.replace("#", "")}</span>
        <div class="pct">
          <NumberField label="" suffix="%" min={0} max={100} value={e.opacity} onCommit={(v) => change(i, { opacity: v })} />
        </div>
      </div>
      {#if picker === i}
        <ColorPicker
          hex={e.color}
          alpha={e.opacity}
          top={settings.top}
          right={500}
          onchange={(hex, a) => change(i, { color: hex, opacity: a }, true)}
          onclose={() => {
            picker = null;
            editor.endLive();
          }}
        />
      {/if}
    {:else}
      <div class="grid one">
        <NumberField label="Blur" min={0} value={e.blur} onCommit={(v) => change(i, { blur: v })} />
      </div>
    {/if}
  </div>
{/if}

<style>
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .head-actions {
    display: flex;
    gap: 2px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 32px;
    padding: 0 8px 0 12px;
  }
  .row.off .name {
    opacity: 0.5;
  }
  .anchor {
    position: relative;
    min-width: 0;
  }
  .grow {
    flex: 1;
  }
  .select {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    height: 24px;
    padding: 0 6px 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    text-align: left;
  }
  .name {
    white-space: nowrap;
  }
  .small {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 10px;
  }
  .hint {
    padding: 0 16px;
    margin: 4px 0;
  }
  .effect-settings {
    position: fixed;
    right: 252px;
    z-index: 65;
    width: 240px;
    padding-bottom: 12px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 10px;
  }
  .title {
    font-weight: 600;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 0 12px;
  }
  .grid.one {
    grid-template-columns: 1fr;
  }
  .grid :global(.field .label) {
    width: auto;
    padding: 0 6px;
  }
  .color {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 0;
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
    width: 64px;
  }
</style>
