<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import AlignStart from "@lucide/svelte/icons/text-align-start";
  import AlignCenter from "@lucide/svelte/icons/text-align-center";
  import AlignEnd from "@lucide/svelte/icons/text-align-end";
  import AlignJustify from "@lucide/svelte/icons/text-align-justify";
  import AlignTop from "@lucide/svelte/icons/arrow-up-to-line";
  import AlignMiddle from "@lucide/svelte/icons/fold-vertical";
  import AlignBottom from "@lucide/svelte/icons/arrow-down-to-line";
  import AutoWidth from "@lucide/svelte/icons/arrow-left-right";
  import AutoHeight from "@lucide/svelte/icons/arrow-up-down";
  import FixedSize from "@lucide/svelte/icons/maximize";
  import LineHeightIcon from "@lucide/svelte/icons/move-vertical";
  import LetterSpacingIcon from "@lucide/svelte/icons/move-horizontal";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import NumberField from "./NumberField.svelte";
  import UnitField from "./UnitField.svelte";
  import { editor } from "../editor.svelte";
  import type { LineHeight, NodeInfo, TextStyle } from "../engine";
  import Grid from "@lucide/svelte/icons/grid-3x3";
  import Unlink2 from "@lucide/svelte/icons/unlink-2";
  import StylePicker from "./StylePicker.svelte";

  let { n }: { n: NodeInfo } = $props();
  let stylePicker = $state<number | null>(null);
  const linked = $derived(n.styles?.text ? (editor.styles.list.find((s) => s.id === n.styles?.text) ?? null) : null);

  const t = $derived(n.text!);
  const family = $derived(editor.fonts.find((f) => f.name === t.family));
  let open = $state<"family" | "weight" | "size" | "settings" | null>(null);

  const SIZES = [10, 11, 12, 13, 14, 15, 16, 20, 24, 32, 36, 40, 48, 64, 96, 128];
  const WEIGHTS: [number, string][] = [
    [100, "Thin"],
    [200, "Extra Light"],
    [300, "Light"],
    [400, "Regular"],
    [500, "Medium"],
    [600, "Semi Bold"],
    [700, "Bold"],
    [800, "Extra Bold"],
    [900, "Black"],
  ];

  const set = (change: Partial<TextStyle>) => editor.setTextStyle(change);
  const toggle = (menu: NonNullable<typeof open>) => (open = open === menu ? null : menu);

  function familyItems(): MenuItem[] {
    const items: MenuItem[] = editor.fonts.map((f) => ({
      label: f.name,
      checked: f.name === t.family,
      action: () => {
        const weight = Math.min(f.weights[1], Math.max(f.weights[0], t.weight));
        set({ family: f.name, weight, italic: t.italic && f.italic });
      },
    }));
    if (!family) items.unshift({ label: `${t.family} (missing)`, checked: true, disabled: true, action: () => {} });
    const loaded = new Set(editor.fonts.map((f) => f.name.toLowerCase()));
    const more = editor.systemFamilies.filter((f) => !loaded.has(f.name.toLowerCase()));
    if (more.length) {
      items.push("-", { label: "Installed on this computer", heading: true, action: () => {} });
      for (const f of more) items.push({ label: f.name, action: () => void editor.useFamily(f.name, t.weight, t.italic) });
    }
    return items;
  }

  function weightItems(): MenuItem[] {
    const [lo, hi] = family?.weights ?? [400, 400];
    const available = WEIGHTS.filter(([w]) => w >= lo && w <= hi);
    const list = available.length ? available : [[t.weight, t.weightName] as [number, string]];
    const items: MenuItem[] = [];
    for (const italic of [false, true]) {
      if (italic ? family && !family.italic : family && !family.upright) continue;
      if (italic && items.length) items.push("-");
      for (const [w, name] of list) {
        const label = italic ? (name === "Regular" ? "Italic" : `${name} Italic`) : name;
        items.push({
          label,
          checked: t.weight === w && t.italic === italic,
          action: () => set({ weight: w, italic }),
        });
      }
    }
    return items;
  }

  const sizeItems = (): MenuItem[] =>
    SIZES.map((s) => ({ label: `${s}`, checked: s === t.size, action: () => set({ size: s }) }));

  function settingsItems(): MenuItem[] {
    const decoration = (d: TextStyle["decoration"], label: string, shortcut?: string): MenuItem => ({
      label,
      shortcut,
      checked: t.decoration === d,
      action: () => set({ decoration: d }),
    });
    const textCase = (c: TextStyle["case"], label: string): MenuItem => ({
      label,
      checked: t.case === c,
      action: () => set({ case: c }),
    });
    const list = (l: NonNullable<TextStyle["list"]>, label: string): MenuItem => ({
      label,
      checked: (t.list ?? "none") === l,
      action: () => set({ list: l }),
    });
    // OpenType features toggle on and off.
    const feature = (tag: string, value: number, label: string): MenuItem => ({
      label,
      checked: t.features?.[tag] === value,
      action: () => {
        const next = { ...(t.features ?? {}) };
        if (next[tag] === value) delete next[tag];
        else next[tag] = value;
        set({ features: next });
      },
    });
    return [
      decoration("none", "No decoration"),
      decoration("underline", "Underline", "Ctrl+U"),
      decoration("strikethrough", "Strikethrough", "Ctrl+Shift+X"),
      "-",
      textCase("original", "As typed"),
      textCase("upper", "Uppercase"),
      textCase("lower", "Lowercase"),
      textCase("title", "Title Case"),
      "-",
      list("none", "No list"),
      list("bullet", "Bulleted list"),
      list("numbered", "Numbered list"),
      "-",
      feature("tnum", 1, "Tabular numbers"),
      feature("liga", 0, "No ligatures"),
      feature("smcp", 1, "Small caps"),
      feature("zero", 1, "Slashed zero"),
    ];
  }

  const lineHeightLabel = (l: LineHeight) =>
    l.unit === "auto" ? "Auto" : l.unit === "percent" ? `${round(l.value)}%` : `${round(l.value)}`;
  const round = (v: number) => Math.round(v * 100) / 100;

  function parseLineHeight(raw: string): LineHeight | null {
    const s = raw.trim().toLowerCase().replace(",", ".");
    if (s === "" || s === "auto") return { unit: "auto" };
    const v = parseFloat(s);
    if (!Number.isFinite(v) || v < 0) return null;
    return s.endsWith("%") ? { unit: "percent", value: v } : { unit: "px", value: v };
  }

  function parseLetterSpacing(raw: string): number | null {
    const s = raw.trim().toLowerCase().replace(",", ".");
    const v = parseFloat(s);
    if (!Number.isFinite(v)) return null;
    return s.endsWith("px") ? (v / t.size) * 100 : v;
  }
</script>

<div class="section-head">
  <span class="section-title">Typography</span>
  <button class="icon-btn" data-style-trigger title="Text styles" onclick={(e) => (stylePicker = stylePicker ? null : (e.currentTarget as HTMLElement).getBoundingClientRect().top)}>
    <Grid size={14} strokeWidth={1.7} />
  </button>
</div>
{#if linked}
  <div class="style-row">
    <span class="ag" style="font-weight:{linked.type === 'text' ? linked.weight : 400}">Ag</span>
    <button class="style-chip" data-style-trigger title="Change style" onclick={(e) => (stylePicker = (e.currentTarget as HTMLElement).getBoundingClientRect().top)}>{linked.name}</button>
    <button class="icon-btn" title="Detach style" onclick={() => editor.linkStyle("text", null)}><Unlink2 size={14} strokeWidth={1.7} /></button>
  </div>
{/if}
{#if stylePicker !== null}
  <StylePicker kind="text" top={stylePicker} onclose={() => (stylePicker = null)} />
{/if}

<div class="rows">
  <div class="anchor">
    <button class="select wide" data-menu-trigger onclick={() => toggle("family")}>
      <span class="value" class:missing={!family}>{t.family}</span>
      <ChevronDown size={10} strokeWidth={2} />
    </button>
    <Menu items={open === "family" ? familyItems() : []} open={open === "family"} onclose={() => (open = null)} />
  </div>

  <div class="grid">
    <div class="anchor">
      <button class="select" data-menu-trigger onclick={() => toggle("weight")}>
        <span class="value">{t.weightName}</span>
        <ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={open === "weight" ? weightItems() : []} open={open === "weight"} onclose={() => (open = null)} />
    </div>
    <div class="anchor size">
      <NumberField label="" value={t.size} min={1} max={1000} title="Font size" onCommit={(v) => set({ size: v })} />
      <button class="size-menu" data-menu-trigger aria-label="Font size presets" onclick={() => toggle("size")}>
        <ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={open === "size" ? sizeItems() : []} open={open === "size"} align="right" onclose={() => (open = null)} />
    </div>
  </div>

  <div class="grid">
    <UnitField
      value={lineHeightLabel(t.lineHeight)}
      title="Line height (Auto, 24 or 150%)"
      onCommit={(raw) => {
        const v = parseLineHeight(raw);
        if (v) set({ lineHeight: v });
      }}
    >
      {#snippet label()}<LineHeightIcon size={12} strokeWidth={1.8} />{/snippet}
    </UnitField>
    <UnitField
      value={`${round(t.letterSpacing)}%`}
      title="Letter spacing (% of size, or px)"
      onCommit={(raw) => {
        const v = parseLetterSpacing(raw);
        if (v !== null) set({ letterSpacing: v });
      }}
    >
      {#snippet label()}<LetterSpacingIcon size={12} strokeWidth={1.8} />{/snippet}
    </UnitField>
  </div>

  <div class="toolbar">
    <div class="seg">
      <button class="icon-btn" class:on={t.align === "left"} title="Align left" onclick={() => set({ align: "left" })}>
        <AlignStart size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.align === "center"} title="Align center" onclick={() => set({ align: "center" })}>
        <AlignCenter size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.align === "right"} title="Align right" onclick={() => set({ align: "right" })}>
        <AlignEnd size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.align === "justify"} title="Justify" onclick={() => set({ align: "justify" })}>
        <AlignJustify size={14} strokeWidth={1.7} />
      </button>
    </div>
    <div class="seg">
      <button class="icon-btn" class:on={t.vertical === "top"} title="Align top" onclick={() => set({ vertical: "top" })}>
        <AlignTop size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.vertical === "middle"} title="Align middle" onclick={() => set({ vertical: "middle" })}>
        <AlignMiddle size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.vertical === "bottom"} title="Align bottom" onclick={() => set({ vertical: "bottom" })}>
        <AlignBottom size={14} strokeWidth={1.7} />
      </button>
    </div>
    <div class="anchor">
      <button class="icon-btn" data-menu-trigger class:on={open === "settings"} title="Type settings" onclick={() => toggle("settings")}>
        <Ellipsis size={14} strokeWidth={1.7} />
      </button>
      <Menu items={open === "settings" ? settingsItems() : []} open={open === "settings"} align="right" onclose={() => (open = null)} />
    </div>
  </div>

  <div class="toolbar">
    <span class="sub">Resizing</span>
    <div class="seg">
      <button class="icon-btn" class:on={t.resize === "auto-width"} title="Auto width" onclick={() => set({ resize: "auto-width" })}>
        <AutoWidth size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.resize === "auto-height"} title="Auto height" onclick={() => set({ resize: "auto-height" })}>
        <AutoHeight size={14} strokeWidth={1.7} />
      </button>
      <button class="icon-btn" class:on={t.resize === "fixed"} title="Fixed size" onclick={() => set({ resize: "fixed" })}>
        <FixedSize size={14} strokeWidth={1.7} />
      </button>
    </div>
  </div>
</div>

<style>
  .style-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 8px 6px 16px;
  }
  .ag {
    width: 18px;
    flex: none;
  }
  .style-chip {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 8px;
    border-radius: 4px;
    background: var(--bg-secondary);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .rows {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 16px 8px;
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .anchor {
    position: relative;
    min-width: 0;
  }
  .select {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 4px;
    width: 100%;
    height: 24px;
    padding: 0 6px 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    outline: 1px solid transparent;
    outline-offset: -1px;
    text-align: left;
  }
  .select:hover {
    outline-color: var(--border);
  }
  .value {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .value.missing {
    color: #ffb46b;
  }
  .size {
    display: flex;
  }
  .size :global(.field) {
    flex: 1;
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .size :global(.field .label) {
    width: 8px;
  }
  .size-menu {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 24px;
    border-radius: 0 var(--radius) var(--radius) 0;
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }
  .size-menu:hover {
    color: var(--text);
  }
  .toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .seg {
    display: flex;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .sub {
    color: var(--text-secondary);
  }
</style>
