<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import RotateCw from "@lucide/svelte/icons/rotate-cw";
  import RotateCwSquare from "@lucide/svelte/icons/rotate-cw-square";
  import FlipH from "@lucide/svelte/icons/triangles-centerline-dashed-horizontal";
  import FlipV from "@lucide/svelte/icons/triangles-centerline-dashed-vertical";
  import Link from "@lucide/svelte/icons/link-2";
  import Unlink from "@lucide/svelte/icons/unlink-2";
  import Scan from "@lucide/svelte/icons/scan";
  import Corners from "@lucide/svelte/icons/square-round-corner";
  import Sun from "@lucide/svelte/icons/sun-medium";
  import BlendIcon from "@lucide/svelte/icons/blend";
  import Grid from "@lucide/svelte/icons/grid-3x3";
  import AlignLeft from "@lucide/svelte/icons/align-start-vertical";
  import AlignCenterH from "@lucide/svelte/icons/align-center-vertical";
  import AlignRight from "@lucide/svelte/icons/align-end-vertical";
  import AlignTop from "@lucide/svelte/icons/align-start-horizontal";
  import AlignCenterV from "@lucide/svelte/icons/align-center-horizontal";
  import AlignBottom from "@lucide/svelte/icons/align-end-horizontal";
  import DistributeH from "@lucide/svelte/icons/align-horizontal-space-between";
  import DistributeV from "@lucide/svelte/icons/align-vertical-space-between";
  import ComponentProps from "./ComponentProps.svelte";
  import LayoutGrid from "@lucide/svelte/icons/layout-grid";
  import PackIcon from "@lucide/svelte/icons/layout-dashboard";
  import SquareSquare from "@lucide/svelte/icons/square-square";
  import NumberField from "./NumberField.svelte";
  import AutoLayoutControls from "./AutoLayoutControls.svelte";
  import ColorPicker from "./ColorPicker.svelte";
  import RangeSlider from "./RangeSlider.svelte";
  import Effects from "./Effects.svelte";
  import SelectionColors from "./SelectionColors.svelte";
  import ExportSection from "./ExportSection.svelte";
  import Pin from "@lucide/svelte/icons/pin";
  import ComponentIcon from "@lucide/svelte/icons/component";
  import Diamond from "@lucide/svelte/icons/diamond";
  import Target from "@lucide/svelte/icons/target";
  import RotateCcw from "@lucide/svelte/icons/rotate-ccw";
  import Typography from "./Typography.svelte";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";
  import type { BindProp, Blend, EndPoint, Constraint, NodeInfo, Paint, Patch, Sizing, StrokeAlign, VarKind } from "../engine";

  const H_CONSTRAINTS: [Constraint, string][] = [["min", "Left"], ["max", "Right"], ["stretch", "Left and right"], ["center", "Center"], ["scale", "Scale"]];
  const V_CONSTRAINTS: [Constraint, string][] = [["min", "Top"], ["max", "Bottom"], ["stretch", "Top and bottom"], ["center", "Center"], ["scale", "Scale"]];
  /** Constraints apply to layers in a frame without auto layout (or absolute in one). */
  const constrained = $derived(
    editor.selection.length > 0 &&
      editor.selection.every((n) => {
        const p = editor.layers.find((l) => l.id === n.parent);
        return p?.kind === "Frame" && (n.parentFlow === null || n.absolute);
      }),
  );
  const ch = $derived(editor.common((n) => n.constraints?.[0] ?? "min"));
  const cv = $derived(editor.common((n) => n.constraints?.[1] ?? "min"));
  import VariablePicker from "./VariablePicker.svelte";
  import StylePicker from "./StylePicker.svelte";
  import LayoutGrids from "./LayoutGrids.svelte";
  import Palette from "@lucide/svelte/icons/palette";
  import MoveHorizontal from "@lucide/svelte/icons/move-horizontal";
  import WrapText from "@lucide/svelte/icons/wrap-text";
  import Square from "@lucide/svelte/icons/square";
  import Unlink2 from "@lucide/svelte/icons/unlink-2";

  let varPicker = $state<{ prop: BindProp; kind: VarKind; top: number } | null>(null);
  let fillStyles = $state<number | null>(null);
  const fillStyle = $derived.by(() => {
    const id = editor.common((n) => n.styles?.fill ?? null);
    return id ? (editor.styles.list.find((s) => s.id === id) ?? null) : null;
  });

  function openVarPicker(e: MouseEvent, prop: BindProp, kind: VarKind) {
    if (varPicker) return (varPicker = null);
    varPicker = { prop, kind, top: (e.currentTarget as HTMLElement).getBoundingClientRect().top };
  }

  /** The variable bound to `prop` on every selected layer, if they share one. */
  function boundVar(prop: BindProp) {
    const id = editor.common((n) => n.bindings?.[prop] ?? null);
    return id ? (editor.variables.variables.find((v) => v.id === id) ?? null) : null;
  }
  import { argbCss } from "../color";

  /** A swatch for a gradient or image fill. */
  function paintCss(p: Paint): string {
    if (p.type === "image") {
      const url = editor.imageUrl(p.hash);
      return url ? `center / cover no-repeat url('${url}')` : "#d9d9d9";
    }
    const stops = [...p.stops].sort((x, y) => x.at - y.at).map((s) => `${argbCss(s.color)} ${(s.at * 100).toFixed(1)}%`);
    return `linear-gradient(90deg, ${stops.join(", ")})`;
  }

  function paintLabel(p: Paint): string {
    if (p.type === "image") return "Image";
    return { linear: "Linear", radial: "Radial", angular: "Angular", diamond: "Diamond" }[p.kind];
  }

  let picker = $state<{ key: string; top: number; swatches: string[] } | null>(null);

  function openPicker(e: MouseEvent, key: string) {
    if (picker) return closePicker();
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    editor.beginLive();
    picker = { key, top: r.top, swatches: editor.pageColors() };
  }

  function closePicker() {
    if (!picker) return;
    picker = null;
    editor.endLive();
  }

  let open = $state<"blend" | "preset" | "strokeAlign" | "sizingH" | "sizingV" | null>(null);
  const toggle = (m: NonNullable<typeof open>) => (open = open === m ? null : m);

  const sel = $derived(editor.selection);
  const one = $derived(sel.length === 1 ? sel[0] : null);
  const set = (p: Patch) => editor.patch(p);
  const soon = (what: string) => () => editor.notify(`${what} is coming soon`);

  /** Shared value of the selection, or null when the layers differ. */
  const common = <T,>(get: (n: NodeInfo) => T) => editor.common(get);
  const num = (get: (n: NodeInfo) => number) => {
    const v = common(get);
    return { value: v ?? 0, mixed: v === null };
  };

  const shapes = $derived(sel.every((n) => n.kind === "Frame" || n.kind === "Rectangle"));
  const paintable = $derived(sel.some((n) => n.kind !== "Group"));
  const corners = $derived(common((n) => n.corners !== null));
  const locked = $derived(common((n) => n.lockRatio) === true);
  const opacity = $derived(num((n) => n.opacity));
  const strokeAlign = $derived(common((n) => n.stroke?.align ?? null));

  const BLENDS: [Blend, string][] = [
    ["pass-through", "Pass through"],
    ["normal", "Normal"],
    ["darken", "Darken"],
    ["multiply", "Multiply"],
    ["color-burn", "Color burn"],
    ["lighten", "Lighten"],
    ["screen", "Screen"],
    ["color-dodge", "Color dodge"],
    ["overlay", "Overlay"],
    ["soft-light", "Soft light"],
    ["hard-light", "Hard light"],
    ["difference", "Difference"],
    ["exclusion", "Exclusion"],
    ["hue", "Hue"],
    ["saturation", "Saturation"],
    ["color", "Color"],
    ["luminosity", "Luminosity"],
  ];
  const blendSeparators = new Set<Blend>(["normal", "color-burn", "color-dodge", "hard-light", "exclusion"]);

  function blendItems(): MenuItem[] {
    const current = common((n) => n.blend);
    const containers = sel.every((n) => n.kind === "Frame" || n.kind === "Group");
    const items: MenuItem[] = [];
    for (const [b, label] of BLENDS) {
      if (b === "pass-through" && !containers) continue;
      items.push({ label, checked: current === b, action: () => set({ blend: b }) });
      if (blendSeparators.has(b)) items.push("-");
    }
    return items;
  }

  // Figma's frame presets (a selection of the common ones).
  const PRESETS: [string, [string, number, number][]][] = [
    [
      "Phone",
      [
        ["iPhone 16", 393, 852],
        ["iPhone 16 Pro Max", 440, 956],
        ["iPhone SE", 320, 568],
        ["Android Compact", 412, 917],
        ["Android Medium", 700, 840],
      ],
    ],
    [
      "Tablet",
      [
        ["iPad mini 8.3", 744, 1133],
        ["iPad Pro 11\"", 834, 1194],
        ["iPad Pro 12.9\"", 1024, 1366],
        ["Android Expanded", 1280, 800],
      ],
    ],
    [
      "Desktop",
      [
        ["MacBook Air", 1280, 832],
        ["MacBook Pro 14\"", 1512, 982],
        ["Desktop", 1440, 1024],
        ["Wireframe", 1440, 1024],
        ["TV", 1280, 720],
      ],
    ],
    [
      "Presentation & social",
      [
        ["Slide 16:9", 1920, 1080],
        ["Slide 4:3", 1024, 768],
        ["Instagram post", 1080, 1350],
        ["Instagram story", 1080, 1920],
        ["Twitter post", 1200, 675],
      ],
    ],
  ];

  function presetItems(): MenuItem[] {
    const items: MenuItem[] = [];
    for (const [group, list] of PRESETS) {
      if (items.length) items.push("-");
      items.push({ label: group, heading: true });
      for (const [name, w, h] of list) {
        items.push({ label: name, shortcut: `${w}×${h}`, action: () => set({ w, h }) });
      }
    }
    return items;
  }

  const strokeAligns: [StrokeAlign, string][] = [
    ["inside", "Inside"],
    ["center", "Center"],
    ["outside", "Outside"],
  ];

  function strokeAlignItems(): MenuItem[] {
    const current = common((n) => n.stroke?.align ?? null);
    return strokeAligns.map(([a, label]) => ({ label, checked: current === a, action: () => set({ strokeAlign: a }) }));
  }

  function hex(raw: string) {
    const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(raw.trim());
    return m ? `#${(m[1].length === 3 ? m[1].replace(/./g, (c) => c + c) : m[1]).toUpperCase()}` : null;
  }

  function commitHex(raw: string, key: "fill" | "strokeColor") {
    const v = hex(raw);
    if (v) set({ [key]: v });
  }

  const SIZING_LABEL: Record<Sizing, string> = { fixed: "Fixed", hug: "Hug", fill: "Fill" };
  const canHug = $derived(sel.every((n) => n.layout !== null || n.kind === "Text"));
  const canFill = $derived(sel.every((n) => n.parentFlow !== null && !n.absolute));
  const inFlow = $derived(sel.some((n) => n.parentFlow !== null));
  const absolute = $derived(common((n) => n.absolute) === true);

  function sizingItems(axis: 0 | 1): MenuItem[] {
    const current = common((n) => n.sizing[axis]);
    const key = axis === 0 ? "sizingH" : "sizingV";
    const dim = axis === 0 ? "width" : "height";
    return [
      { label: `Fixed ${dim}`, checked: current === "fixed", action: () => set({ [key]: "fixed" }) },
      { label: "Hug contents", checked: current === "hug", disabled: !canHug, action: () => set({ [key]: "hug" }) },
      { label: "Fill container", checked: current === "fill", disabled: !canFill, action: () => set({ [key]: "fill" }) },
    ];
  }

  const label = (blend: Blend | null) => (blend ? (BLENDS.find((b) => b[0] === blend)?.[1] ?? blend) : "Mixed");
  /** Scale tool: the box point the selection scales from (0–8, 4 = centre). */
  let scaleAnchor = $state(0);
  const END_POINTS: [EndPoint, string][] = [
    ["none", "None"],
    ["line-arrow", "Line arrow"],
    ["triangle-arrow", "Triangle arrow"],
    ["reversed-triangle", "Reversed triangle"],
    ["circle", "Circle arrow"],
    ["diamond", "Diamond arrow"],
  ];
</script>

{#snippet head(title: string, actions?: import("svelte").Snippet)}
  <div class="section-head">
    <span class="section-title">{title}</span>
    <span class="head-actions">{#if actions}{@render actions()}{/if}</span>
  </div>
{/snippet}

{#snippet alignButtons()}
  {@const disabled = sel.length === 1 && (one?.parent ?? -1) < 0}
  <div class="align-row">
    <button class="icon-btn" {disabled} title="Align left (Alt+A)" onclick={() => editor.align("left")}><AlignLeft size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" {disabled} title="Align horizontal centers (Alt+H)" onclick={() => editor.align("centerX")}><AlignCenterH size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" {disabled} title="Align right (Alt+D)" onclick={() => editor.align("right")}><AlignRight size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" {disabled} title="Align top (Alt+W)" onclick={() => editor.align("top")}><AlignTop size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" {disabled} title="Align vertical centers (Alt+V)" onclick={() => editor.align("centerY")}><AlignCenterV size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" {disabled} title="Align bottom (Alt+S)" onclick={() => editor.align("bottom")}><AlignBottom size={14} strokeWidth={1.7} /></button>
  </div>
  {#if sel.length > 2}
    <div class="align-row distribute">
      <button class="icon-btn" title="Distribute horizontal spacing (Alt+Shift+H)" onclick={() => editor.align("distributeX")}><DistributeH size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Distribute vertical spacing (Alt+Shift+V)" onclick={() => editor.align("distributeY")}><DistributeV size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Tidy up (Ctrl+Alt+Shift+T)" onclick={() => editor.align("tidy")}><LayoutGrid size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Pack" onclick={() => editor.align("pack")}><PackIcon size={14} strokeWidth={1.7} /></button>
    </div>
  {/if}
{/snippet}

<div class="kind-row">
  {#if one?.component === "main"}
    <span class="kind-label purple"><ComponentIcon size={14} strokeWidth={1.8} />Component</span>
  {:else if one?.component === "instance"}
    <span class="kind-label purple" title="Instance of {one.mainName ?? 'a deleted component'}">
      <Diamond size={14} strokeWidth={1.8} />{one.mainName ?? "Instance"}
    </span>
    <span class="spacer"></span>
    <button class="icon-btn" title="Go to main component" disabled={!one.main} onclick={() => editor.goToMain()}>
      <Target size={14} strokeWidth={1.7} />
    </button>
    <button class="icon-btn" title="Reset all changes" disabled={!one.overrides && !sel.some((n) => n.overrides)} onclick={() => editor.resetInstance()}>
      <RotateCcw size={14} strokeWidth={1.7} />
    </button>
    <button class="icon-btn" title="Detach instance (Ctrl+Alt+B)" onclick={() => editor.detachInstance()}>
      <Unlink size={14} strokeWidth={1.7} />
    </button>
  {:else if one?.inInstance}
    <span class="kind-label purple">{one.kind}</span>
  {:else if one?.variantSet}
    <span class="kind-label purple"><ComponentIcon size={14} strokeWidth={1.8} />Component set</span>
  {:else if one?.kind === "Frame"}
    <button class="icon-btn ready-btn" class:on={one.ready} title={one.ready ? "Ready for dev — click to clear" : "Mark as ready for dev"} onclick={() => set({ ready: !one.ready })}>&lt;/&gt;</button>
    <div class="anchor">
      <button class="kind" data-menu-trigger title="Frame presets" onclick={() => toggle("preset")}>
        {one.section ? "Section" : "Frame"}<ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={open === "preset" ? presetItems() : []} open={open === "preset"} onclose={() => (open = null)} />
    </div>
  {:else if one}
    <span class="kind-label">{one.kind}</span>
  {:else}
    <span class="kind-label">{sel.length} layers</span>
  {/if}
</div>

{#snippet positionActions()}
  {#if inFlow}
    <button
      class="icon-btn"
      class:on={absolute}
      title={absolute ? "Use auto layout position" : "Absolute position (ignore auto layout)"}
      onclick={() => set({ absolute: !absolute })}
    >
      <Pin size={14} strokeWidth={1.7} />
    </button>
  {/if}
{/snippet}
{#if editor.tool === "scale" && sel.length}
  {@render head("Scale")}
  <div class="scale-row">
    <div class="anchor-grid" role="radiogroup" aria-label="Scale from">
      {#each [0, 1, 2, 3, 4, 5, 6, 7, 8] as a (a)}
        <button class="dot" class:on={scaleAnchor === a} role="radio" aria-checked={scaleAnchor === a} aria-label="Anchor {a}" onclick={() => (scaleAnchor = a)}></button>
      {/each}
    </div>
    <NumberField label="×" suffix="%" min={1} value={100} title="Scale the selection and everything in it" onCommit={(v) => editor.run((e) => e.scale(v / 100, scaleAnchor))} />
  </div>
  <div class="divider"></div>
{/if}
{#if one?.variants?.length}
  <div class="variants">
    {#each one.variants as p (p.name)}
      <label class="variant">
        <span class="faint">{p.name}</span>
        <select value={p.current ?? ""} onchange={(e) => editor.setVariant(p.name, (e.target as HTMLSelectElement).value)}>
          {#each p.values as v (v)}<option value={v}>{v}</option>{/each}
        </select>
      </label>
    {/each}
  </div>
{/if}
{#if one?.propsMode}
  <ComponentProps n={one} />
{/if}
{#if one && (one.component === "main" || one.variantSet)}
  <div class="code-connect">
    <div class="sub">Code Connect</div>
    <input
      class="cc-input"
      placeholder="Component in code (e.g. Button)"
      value={one.code?.component ?? ""}
      onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
      onblur={(e) => {
        const component = (e.target as HTMLInputElement).value.trim();
        if (component !== (one.code?.component ?? "")) editor.setCodeConnect(one.id, { ...(one.code ?? { import: "", source: "", props: {}, text: {} }), component });
      }}
    />
    <input
      class="cc-input mono"
      placeholder={'import { Button } from "@/components/ui/button"'}
      value={one.code?.import ?? ""}
      onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
      onblur={(e) => {
        const imp = (e.target as HTMLInputElement).value.trim();
        if (one.code && imp !== one.code.import) editor.setCodeConnect(one.id, { ...one.code, import: imp });
      }}
    />
  </div>
{/if}
{#if sel.length > 0 && sel.every((n) => n.component === "main") && !one?.variantSet}
  <div class="pad-row">
    <button class="wide-btn" onclick={() => editor.combineVariants()}>{sel.length > 1 ? "Combine as variants" : "Add variant"}</button>
  </div>
{/if}
<div class="sub first">Alignment</div>
{@render alignButtons()}
<div class="sub">Position</div>
<div class="grid-lock">
  <NumberField label="X" {...num((n) => n.x)} onCommit={(v) => set({ x: v })} />
  <NumberField label="Y" {...num((n) => n.y)} onCommit={(v) => set({ y: v })} />
  {@render positionActions()}
</div>
{#if constrained}
  <div class="sub">Constraints</div>
  <div class="grid2 constraints">
    <select class="c-select" title="Horizontal constraint" value={ch ?? ""} onchange={(e) => set({ constraintH: (e.target as HTMLSelectElement).value as Constraint })}>
      {#if ch === null}<option value="" disabled>Mixed</option>{/if}
      {#each H_CONSTRAINTS as [v, label] (v)}<option value={v}>{label}</option>{/each}
    </select>
    <select class="c-select" title="Vertical constraint" value={cv ?? ""} onchange={(e) => set({ constraintV: (e.target as HTMLSelectElement).value as Constraint })}>
      {#if cv === null}<option value="" disabled>Mixed</option>{/if}
      {#each V_CONSTRAINTS as [v, label] (v)}<option value={v}>{label}</option>{/each}
    </select>
  </div>
{/if}
<div class="sub">Rotation</div>
<div class="grid2">
  <NumberField suffix="°" title="Rotation" {...num((n) => n.rotation)} onCommit={(v) => set({ rotation: v })}>
    {#snippet label()}<RotateCw size={12} strokeWidth={1.8} />{/snippet}
  </NumberField>
  <div class="buttons">
    <button class="icon-btn" title="Rotate 90° right" onclick={() => editor.rotateBy(-90)}><RotateCwSquare size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" title="Flip horizontal (Shift+H)" onclick={() => editor.flip(true)}><FlipH size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" title="Flip vertical (Shift+V)" onclick={() => editor.flip(false)}><FlipV size={14} strokeWidth={1.7} /></button>
  </div>
</div>

<div class="divider"></div>
{#snippet layoutActions()}
  {#if one?.layout}
    <button class="icon-btn" title="Remove auto layout (Alt+Shift+A)" onclick={() => editor.autoLayout(false)}>
      <Minus size={14} strokeWidth={1.7} />
    </button>
  {:else}
    <button class="icon-btn" title="Add auto layout (Shift+A)" onclick={() => editor.autoLayout(true)}>
      <Plus size={14} strokeWidth={1.7} />
    </button>
  {/if}
{/snippet}
{@render head(one?.layout ? "Auto layout" : "Layout", layoutActions)}
{#if one?.layout}
  <AutoLayoutControls layout={one.layout} />
{/if}
{#if sel.length && sel.every((n) => n.text)}
  {@const resize = common((n) => n.text?.resize ?? null)}
  <div class="sub">Resizing</div>
  <div class="resizing" role="radiogroup" aria-label="Text resizing">
    <button class="seg" class:on={resize === "auto-width"} role="radio" aria-checked={resize === "auto-width"} title="Auto width" onclick={() => editor.setTextStyle({ resize: "auto-width" })}>
      <MoveHorizontal size={14} strokeWidth={1.7} />
    </button>
    <button class="seg" class:on={resize === "auto-height"} role="radio" aria-checked={resize === "auto-height"} title="Auto height" onclick={() => editor.setTextStyle({ resize: "auto-height" })}>
      <WrapText size={14} strokeWidth={1.7} />
    </button>
    <button class="seg" class:on={resize === "fixed"} role="radio" aria-checked={resize === "fixed"} title="Fixed size" onclick={() => editor.setTextStyle({ resize: "fixed" })}>
      <Square size={13} strokeWidth={1.7} />
    </button>
  </div>
{/if}
<div class="sub">Dimensions</div>
<div class="grid-lock">
  <NumberField label="W" min={0} {...num((n) => n.w)} onCommit={(v) => set({ w: v })} />
  <NumberField label="H" min={0} {...num((n) => n.h)} onCommit={(v) => set({ h: v })} />
  <button
    class="icon-btn"
    class:on={locked}
    title={locked ? "Unlock aspect ratio" : "Lock aspect ratio"}
    onclick={() => set({ lockRatio: !locked })}
  >
    {#if locked}<Link size={14} strokeWidth={1.7} />{:else}<Unlink size={14} strokeWidth={1.7} />{/if}
  </button>
</div>
<div class="grid-lock sizing">
  {#each [0, 1] as axis (axis)}
    {@const value = common((n) => n.sizing[axis])}
    {@const key = axis === 0 ? "sizingH" : "sizingV"}
    <div class="anchor">
      <button class="select" data-menu-trigger title={axis === 0 ? "Horizontal resizing" : "Vertical resizing"} onclick={() => toggle(key)}>
        <span class="faint">{axis === 0 ? "W" : "H"}</span>
        <span>{value ? SIZING_LABEL[value] : "Mixed"}</span>
        <ChevronDown size={10} strokeWidth={2} />
      </button>
      <Menu items={open === key ? sizingItems(axis as 0 | 1) : []} open={open === key} onclose={() => (open = null)} />
    </div>
  {/each}
</div>
{#if one?.kind === "Frame"}
  <label class="check-row">
    <input type="checkbox" checked={one.clip} onchange={(e) => editor.toggleFlag(one.id, "clip", (e.target as HTMLInputElement).checked)} />
    Clip content
  </label>
{/if}

<div class="divider"></div>
{#snippet appearanceActions()}
  {@const visible = common((n) => n.visible) !== false}
  <button class="icon-btn" title={visible ? "Hide (Ctrl+Shift+H)" : "Show (Ctrl+Shift+H)"} onclick={() => editor.toggleFlag(-1, "visible")}>
    {#if visible}<Eye size={14} strokeWidth={1.7} />{:else}<EyeOff size={14} strokeWidth={1.7} />{/if}
  </button>
{/snippet}
{@render head("Appearance", appearanceActions)}
<div class="sub">Opacity</div>
<div class="slider-row">
  <NumberField suffix="%" min={0} max={100} title="Layer opacity (0–9 keys)" {...opacity} onCommit={(v) => set({ opacity: v })}>
    {#snippet label()}<Sun size={12} strokeWidth={1.8} />{/snippet}
  </NumberField>
  <RangeSlider
    value={opacity.value}
    title="Layer opacity"
    onstart={() => editor.beginLive()}
    oninput={(v) => editor.livePatch({ opacity: v })}
    onend={() => editor.endLive()}
  />
</div>
{#if shapes}
  {@const radius = num((n) => n.radius)}
  <div class="sub">Corner radius</div>
  <div class="slider-row corner">
    <NumberField min={0} title="Corner radius" {...radius} mixed={corners !== false || radius.mixed} onCommit={(v) => set({ radius: v })}>
      {#snippet label()}<Scan size={12} strokeWidth={1.8} />{/snippet}
    </NumberField>
    <RangeSlider
      value={radius.value}
      max={Math.max(1, Math.round(Math.min(...sel.map((n) => Math.min(n.w, n.h) / 2))))}
      title="Corner radius"
      onstart={() => editor.beginLive()}
      oninput={(v) => editor.livePatch({ radius: v })}
      onend={() => editor.endLive()}
    />
    <button
      class="icon-btn"
      class:on={corners === true}
      title="Independent corners"
      onclick={() => (corners === true ? set({ corners: [] }) : set({ corners: [0, 1, 2, 3].map(() => one?.radius ?? 0) }))}
    >
      <Corners size={14} strokeWidth={1.7} />
    </button>
  </div>
{/if}
{#if shapes && corners === true}
  <div class="grid2">
    {#each ["Top left", "Top right", "Bottom right", "Bottom left"] as title, i (title)}
      <NumberField
        label={["↖", "↗", "↘", "↙"][i]}
        min={0}
        {title}
        {...num((n) => n.corners?.[i] ?? n.radius)}
        onCommit={(v) => {
          for (const n of sel) {
            const c = [...(n.corners ?? [n.radius, n.radius, n.radius, n.radius])];
            c[i] = v;
            editor.patch({ corners: c }, n.id);
          }
        }}
      />
    {/each}
  </div>
{/if}

<div class="sub">Blend mode</div>
<div class="blend anchor">
  <button class="select" data-menu-trigger onclick={() => toggle("blend")}>
    <BlendIcon size={13} strokeWidth={1.7} />
    <span class="grow">{label(common((n) => n.blend))}</span>
    <ChevronDown size={10} strokeWidth={2} />
  </button>
  <Menu items={open === "blend" ? blendItems() : []} open={open === "blend"} onclose={() => (open = null)} />
</div>

{#if one?.text}
  <div class="divider"></div>
  <Typography n={one} />
{/if}

{#if paintable}
  <div class="divider"></div>
  {@const fillOn = common((n) => n.fillOn)}
  {#snippet fillActions()}
    <button class="icon-btn" data-style-trigger title="Color styles" onclick={(e) => (fillStyles = fillStyles ? null : (e.currentTarget as HTMLElement).getBoundingClientRect().top)}><Palette size={14} strokeWidth={1.7} /></button>
    <button class="icon-btn" data-var-trigger title="Apply variable" onclick={(e) => openVarPicker(e, "fill", "color")}><Grid size={14} strokeWidth={1.7} /></button>
    {#if fillOn !== true}
      <button class="icon-btn" title="Add fill" onclick={() => set({ fillOn: true })}><Plus size={14} strokeWidth={1.7} /></button>
    {:else if one}
      <button class="icon-btn" title="Add another fill" onclick={() => set({ extraFills: [...one.extraFills, { color: "#000000", opacity: 20, paint: null, visible: true }] })}><Plus size={14} strokeWidth={1.7} /></button>
    {/if}
  {/snippet}
  {@render head("Fill", fillActions)}
  {#if fillOn === null}
    <p class="hint faint">Mixed — click + to replace</p>
  {:else if fillOn && fillStyle}
    {@const color = common((n) => n.fill)}
    <div class="row">
      <span class="swatch" style="background:{color ?? '#000'}"></span>
      <button class="var-chip" data-style-trigger title="Change style" onclick={(e) => (fillStyles = (e.currentTarget as HTMLElement).getBoundingClientRect().top)}>{fillStyle.name}</button>
      <button class="icon-btn" title="Detach style" onclick={() => editor.linkStyle("fill", null)}><Unlink2 size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Remove fill (Alt+/)" onclick={() => set({ fillOn: false })}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
  {:else if fillOn && boundVar("fill")}
    {@const v = boundVar("fill")}
    {@const color = common((n) => n.fill)}
    <div class="row">
      <span class="swatch" style="background:{color ?? '#000'}"></span>
      <button class="var-chip" data-var-trigger title="Change variable" onclick={(e) => openVarPicker(e, "fill", "color")}>{v?.name}</button>
      <button class="icon-btn" title="Detach variable" onclick={() => editor.bindVariable("fill", null)}><Unlink2 size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Remove fill (Alt+/)" onclick={() => set({ fillOn: false })}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
  {:else if fillOn}
    {#if one?.extraFills.length}
      {@const extras = one.extraFills}
      {@const save = (i: number, change: Partial<(typeof extras)[number]>, live = false) => {
        const next = extras.map((f, k) => (k === i ? { ...f, ...change } : f));
        if (live) editor.livePatch({ extraFills: next });
        else set({ extraFills: next });
      }}
      <!-- Extra fills, top first as in Figma. -->
      {#each [...extras.keys()].reverse() as i (i)}
        {@const f = extras[i]}
        <div class="row" class:faded={!f.visible}>
          <button
            class="swatch"
            data-picker-trigger
            title="Fill {i + 2}"
            aria-label="Fill {i + 2}"
            style="background:{f.paint ? paintCss(f.paint) : f.color}; opacity:{f.paint ? 1 : Math.max(f.opacity, 15) / 100}"
            onclick={(e) => openPicker(e, `extra-${i}`)}
          ></button>
          {#if picker?.key === `extra-${i}`}
            <ColorPicker
              hex={f.color}
              alpha={f.opacity}
              top={picker.top}
              swatches={picker.swatches}
              paint={f.paint}
              onchange={(hex, a) => save(i, { color: hex, opacity: a, paint: null }, true)}
              onpaint={(p) => save(i, { paint: p }, true)}
              onclose={closePicker}
            />
          {/if}
          {#if f.paint}
            <button class="paint-label" data-picker-trigger onclick={(e) => openPicker(e, `extra-${i}`)}>{paintLabel(f.paint)}</button>
            <div class="pct-field"></div>
          {:else}
            <input
              class="hex-input"
              value={f.color.replace("#", "")}
              spellcheck="false"
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
              }}
              onblur={(e) => {
                const v = (e.target as HTMLInputElement).value.replace("#", "").trim();
                if (/^[0-9a-f]{6}$/i.test(v)) save(i, { color: `#${v.toUpperCase()}` });
              }}
            />
            <div class="pct-field">
              <NumberField label="" suffix="%" min={0} max={100} value={f.opacity} onCommit={(v) => save(i, { opacity: v })} />
            </div>
          {/if}
          <button class="icon-btn" title={f.visible ? "Hide fill" : "Show fill"} onclick={() => save(i, { visible: !f.visible })}>
            {#if f.visible}<Eye size={14} strokeWidth={1.7} />{:else}<EyeOff size={14} strokeWidth={1.7} />{/if}
          </button>
          <button class="icon-btn" title="Remove fill" onclick={() => set({ extraFills: extras.filter((_, k) => k !== i) })}><Minus size={14} strokeWidth={1.7} /></button>
        </div>
      {/each}
    {/if}
    {@const color = common((n) => n.fill)}
    {@const alpha = num((n) => n.fillOpacity)}
    {@const paintKey = common((n) => JSON.stringify(n.paint))}
    {@const paint = paintKey === null ? undefined : (JSON.parse(paintKey) as Paint | null)}
    <div class="row">
      <button
        class="swatch"
        data-picker-trigger
        title="Fill"
        aria-label="Fill"
        style="background:{paint ? paintCss(paint) : (color ?? 'conic-gradient(#f44, #4f4, #44f, #f44)')}; opacity:{paint ? 1 : Math.max(alpha.value, 15) / 100}"
        onclick={(e) => openPicker(e, "fill")}
      ></button>
      {#if picker?.key === "fill"}
        <ColorPicker
          hex={color ?? "#000000"}
          alpha={alpha.value}
          top={picker.top}
          swatches={picker.swatches}
          paint={paint ?? null}
          imageUrl={paint?.type === "image" ? editor.imageUrl(paint.hash) : null}
          onchange={(hex, a) => editor.livePatch({ fill: hex, fillOpacity: a })}
          onpaint={(p) => editor.livePatch({ paint: p })}
          onimage={() => editor.replaceImageDialog()}
          onclose={closePicker}
        />
      {/if}
      {#if paint}
        <button class="paint-label" data-picker-trigger onclick={(e) => openPicker(e, "fill")}>{paintLabel(paint)}</button>
        <div class="pct-field">
          {#if paint.type === "image"}
            <NumberField label="" suffix="%" min={0} max={100} value={Math.round(paint.opacity * 100)} onCommit={(v) => set({ paint: { ...paint, opacity: v / 100 } })} />
          {/if}
        </div>
      {:else}
      <input
        class="hex-input"
        value={color ? color.replace("#", "") : "Mixed"}
        spellcheck="false"
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
        onblur={(e) => commitHex((e.target as HTMLInputElement).value, "fill")}
      />
      <div class="pct-field">
        <NumberField label="" suffix="%" min={0} max={100} {...alpha} onCommit={(v) => set({ fillOpacity: v })} />
      </div>
      {/if}
      <button class="icon-btn" title="Hide fill" onclick={() => set({ fillOn: false })}><Eye size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Remove fill (Alt+/)" onclick={() => set({ fillOn: false })}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
  {/if}

  <div class="divider"></div>
  {@const hasStroke = common((n) => n.stroke !== null)}
  {#snippet strokeActions()}
    {#if hasStroke === true}
      <button class="icon-btn" data-var-trigger title="Apply variable" onclick={(e) => openVarPicker(e, "stroke", "color")}><Grid size={14} strokeWidth={1.7} /></button>
    {/if}
    {#if hasStroke !== true}
      <button class="icon-btn" title="Add stroke" onclick={() => set({ strokeOn: true })}><Plus size={14} strokeWidth={1.7} /></button>
    {/if}
  {/snippet}
  {@render head("Stroke", strokeActions)}
  {#if hasStroke === null}
    <p class="hint faint">Mixed — click + to replace</p>
  {:else if hasStroke}
    {@const color = common((n) => n.stroke?.color ?? null)}
    <div class="row">
      <button
        class="swatch"
        data-picker-trigger
        title="Stroke colour"
        aria-label="Stroke colour"
        style="background:{color ?? 'conic-gradient(#f44, #4f4, #44f, #f44)'}"
        onclick={(e) => openPicker(e, "stroke")}
      ></button>
      {#if picker?.key === "stroke"}
        <ColorPicker
          hex={color ?? "#000000"}
          alpha={common((n) => n.stroke?.opacity ?? 100) ?? 100}
          top={picker.top}
          swatches={picker.swatches}
          onchange={(hex, a) => editor.livePatch({ strokeColor: hex, strokeOpacity: a })}
          onclose={closePicker}
        />
      {/if}
      <input
        class="hex-input"
        value={color ? color.replace("#", "") : "Mixed"}
        spellcheck="false"
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        }}
        onblur={(e) => commitHex((e.target as HTMLInputElement).value, "strokeColor")}
      />
      <div class="pct-field">
        <NumberField label="" suffix="%" min={0} max={100} {...num((n) => n.stroke?.opacity ?? 100)} onCommit={(v) => set({ strokeOpacity: v })} />
      </div>
      <button class="icon-btn" title="Remove stroke (Shift+/)" onclick={() => set({ strokeOn: false })}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
    <div class="grid2">
      <div class="anchor">
        <button class="select" data-menu-trigger onclick={() => toggle("strokeAlign")}>
          <span>{strokeAlign ? strokeAligns.find((a) => a[0] === strokeAlign)?.[1] : "Mixed"}</span>
          <ChevronDown size={10} strokeWidth={2} />
        </button>
        <Menu items={open === "strokeAlign" ? strokeAlignItems() : []} open={open === "strokeAlign"} onclose={() => (open = null)} />
      </div>
      <div class="weight-row">
        {#if one?.sides}
          <span class="faint mixed-weight">Mixed</span>
        {:else}
          <NumberField label="≡" min={0} title="Stroke weight" {...num((n) => n.stroke?.width ?? 0)} onCommit={(v) => set({ strokeWidth: v })} />
        {/if}
        {#if one && (one.kind === "Rectangle" || one.kind === "Frame")}
          <button
            class="icon-btn"
            class:on={!!one.sides}
            title={one.sides ? "All sides" : "Individual strokes"}
            onclick={() => set({ strokeSides: one.sides ? [] : [0, 1, 2, 3].map(() => one.stroke?.width ?? 1) })}
          ><SquareSquare size={14} strokeWidth={1.7} /></button>
        {/if}
      </div>
    </div>
    {#if one?.sides}
      {@const sides = one.sides}
      <div class="sides-grid">
        {#each ["Top", "Right", "Bottom", "Left"] as label, i (label)}
          <NumberField label={label[0]} min={0} title="{label} stroke weight" value={sides[i]} onCommit={(v) => set({ strokeSides: sides.map((x, k) => (k === i ? v : x)) })} />
        {/each}
      </div>
    {/if}
    {@const dash = common((n) => (n.dash ? n.dash.join(",") : ""))}
    <div class="grid2 dash-row">
      <label class="dash-toggle">
        <input type="checkbox" checked={!!dash} onchange={(e) => set({ dash: (e.target as HTMLInputElement).checked ? [4, 4] : [] })} />
        <span>Dashed</span>
      </label>
      {#if dash}
        <input
          class="dash-input"
          title="Dash, gap"
          value={dash.replace(",", ", ")}
          onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") (e.target as HTMLInputElement).blur(); }}
          onblur={(e) => {
            const v = (e.target as HTMLInputElement).value.split(/[ ,]+/).map(Number).filter((x) => Number.isFinite(x) && x > 0);
            if (v.length) set({ dash: v.slice(0, 2) });
          }}
        />
      {/if}
    </div>
    {#if one?.endPoints}
      {@const points = one.endPoints}
      <div class="grid2 end-points">
        {#each ["start", "end"] as side, i (side)}
          <label class="end-point" title="{side === 'start' ? 'Start' : 'End'} point">
            <span class="faint">{side === "start" ? "Start" : "End"}</span>
            <select
              value={points[i]}
              onchange={(e) => {
                const v = (e.target as HTMLSelectElement).value as EndPoint;
                set(side === "start" ? { startPoint: v } : { endPoint: v });
              }}
            >
              {#each END_POINTS as [value, label] (value)}<option {value}>{label}</option>{/each}
            </select>
          </label>
        {/each}
      </div>
    {/if}
  {/if}
{/if}


<div class="divider"></div>
<Effects />
<SelectionColors />
{#if one?.kind === "Frame"}
  <div class="divider"></div>
  <LayoutGrids n={one} />
{/if}
{#if one && (one.note || editor.annotating)}
  <div class="divider"></div>
  <div class="section-head"><span class="section-title">Annotation</span></div>
  <div class="note-box">
    <!-- svelte-ignore a11y_autofocus -->
    <textarea
      placeholder="A note for developers and agents, e.g. “Opens the checkout sheet”"
      value={one.note ?? ""}
      autofocus={editor.annotating}
      rows="3"
      onkeydown={(e) => { e.stopPropagation(); if (e.key === "Escape") (e.target as HTMLTextAreaElement).blur(); }}
      onblur={(e) => {
        editor.annotating = false;
        const v = (e.target as HTMLTextAreaElement).value;
        if (v.trim() !== (one.note ?? "")) editor.patch({ note: v }, one.id);
      }}
    ></textarea>
  </div>
{/if}
{#if one}
  <div class="divider"></div>
  <ExportSection n={one} />
{/if}

{#if fillStyles !== null}
  <StylePicker kind="fill" top={fillStyles} onclose={() => (fillStyles = null)} />
{/if}
{#if varPicker}
  <VariablePicker
    kind={varPicker.kind}
    top={varPicker.top}
    onpick={(id) => {
      if (varPicker) editor.bindVariable(varPicker.prop, id);
      varPicker = null;
    }}
    onclose={() => (varPicker = null)}
  />
{/if}

<style>
  .row.faded > :not(.icon-btn) {
    opacity: 0.45;
  }
  .kind-row {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 16px;
  }
  .kind,
  .kind-label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-weight: 550;
    height: 24px;
  }
  .kind {
    margin-left: -6px;
    padding: 0 6px;
    border-radius: var(--radius);
  }
  .kind:hover {
    background: var(--bg-secondary);
  }
  .kind-label.purple {
    color: var(--component, #9747ff);
    gap: 6px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .spacer {
    flex: 1;
  }
  .anchor {
    position: relative;
  }
  .scale-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px 8px;
  }
  .anchor-grid {
    display: grid;
    grid-template-columns: repeat(3, 10px);
    gap: 4px;
    padding: 4px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .anchor-grid .dot {
    width: 10px;
    height: 10px;
    border-radius: 2px;
    background: var(--text-tertiary, #666);
    opacity: 0.4;
  }
  .anchor-grid .dot.on {
    background: var(--brand-strong, #0d99ff);
    opacity: 1;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .head-actions {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .align-row {
    display: flex;
    justify-content: space-between;
    padding: 0 12px 8px;
  }
  .align-row.distribute {
    justify-content: flex-start;
    gap: 4px;
  }
  .align-row .icon-btn:disabled {
    opacity: 0.4;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 0 16px 8px;
  }
  .grid-lock {
    display: grid;
    grid-template-columns: 1fr 1fr 24px;
    gap: 8px;
    align-items: center;
    padding: 0 8px 8px 16px;
  }
  .sizing .select {
    gap: 6px;
    justify-content: flex-start;
  }
  .sizing .select :global(svg) {
    margin-left: auto;
  }
  .buttons {
    display: flex;
    gap: 2px;
  }
  .check-row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 16px;
    cursor: default;
  }
  .check-row input {
    accent-color: var(--brand);
    margin: 0;
  }
  .divider {
    height: 1px;
    margin: 8px 0 4px;
    background: var(--border);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px 0 16px;
  }
  .swatch {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.1);
  }
  .sub {
    padding: 0 16px 4px;
    color: var(--text-secondary);
    font-size: 11px;
  }
  .slider-row {
    display: grid;
    grid-template-columns: 88px 1fr;
    gap: 10px;
    align-items: center;
    padding: 0 16px 8px;
  }
  .slider-row.corner {
    grid-template-columns: 88px 1fr 24px;
    padding-right: 8px;
  }
  .blend {
    padding: 0 16px 8px;
  }
  .grow {
    flex: 1;
    text-align: left;
  }
  .sub.first {
    padding-top: 4px;
  }
  .blend .select {
    gap: 8px;
    justify-content: flex-start;
  }
  .variants {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 16px 8px;
  }
  .variant {
    display: grid;
    grid-template-columns: 80px 1fr;
    align-items: center;
    gap: 8px;
  }
  .variant select {
    height: 26px;
    padding: 0 6px;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
  }
  .code-connect {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 0 16px 8px;
  }
  .code-connect .sub {
    padding: 0;
  }
  .cc-input {
    height: 26px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  .cc-input:focus {
    border-color: var(--brand-strong);
  }
  .mono {
    font-family: ui-monospace, monospace;
    font-size: 11px;
  }
  .pad-row {
    padding: 2px 16px 8px;
  }
  .wide-btn {
    width: 100%;
    height: 28px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    font-weight: 550;
  }
  .wide-btn:hover {
    background: var(--bg-secondary);
  }
  .ready-btn {
    order: 2;
    margin-left: auto;
    font-size: 10px;
    font-weight: 700;
  }
  .ready-btn.on {
    background: #14ae5c;
    color: #fff;
  }
  .resizing {
    display: flex;
    gap: 2px;
    margin: 0 16px 8px;
    padding: 2px;
    width: max-content;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .seg {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 40px;
    height: 24px;
    border-radius: 4px;
    color: var(--text-secondary);
  }
  .seg.on {
    background: var(--bg);
    color: var(--text);
    box-shadow: 0 0 0 1px var(--border);
  }
  .note-box {
    padding: 0 16px 8px;
  }
  .note-box textarea {
    width: 100%;
    resize: vertical;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    font: inherit;
    outline: none;
    line-height: 1.4;
  }
  .note-box textarea:focus {
    border-color: var(--brand-strong);
  }
  .dash-row {
    margin-top: 4px;
  }
  .weight-row {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
  }
  .weight-row > :global(:first-child) {
    flex: 1;
    min-width: 0;
  }
  .mixed-weight {
    padding-left: 8px;
  }
  .weight-row .icon-btn.on {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .sides-grid {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 4px;
    padding: 0 16px 8px;
  }
  .end-points {
    margin-top: 4px;
  }
  .end-point {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }
  .end-point select {
    flex: 1;
    min-width: 0;
    height: 24px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
    border: 1px solid transparent;
  }
  .dash-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dash-input {
    height: 24px;
    min-width: 0;
    padding: 0 6px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  .dash-input:focus {
    border-color: var(--brand-strong);
  }
  .c-select {
    width: 100%;
    height: 24px;
    padding: 0 6px;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
  }
  .var-chip {
    flex: 1;
    min-width: 0;
    height: 22px;
    padding: 0 8px;
    border-radius: 4px;
    background: var(--bg-secondary);
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .paint-label {
    flex: 1;
    min-width: 0;
    height: 24px;
    text-align: left;
    padding: 0 4px;
    margin-left: -4px;
    border-radius: var(--radius);
  }
  .paint-label:hover {
    background: var(--bg-secondary);
  }
  .hex-input {
    flex: 1;
    min-width: 0;
    height: 24px;
    padding: 0 4px;
    margin-left: -4px;
    background: transparent;
    border: 0;
    border-radius: var(--radius);
    outline: 1px solid transparent;
    outline-offset: -1px;
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
  }
  .hex-input:hover {
    outline-color: var(--border);
  }
  .hex-input:focus {
    outline-color: var(--brand-strong);
  }
  .pct-field {
    width: 58px;
    flex: none;
  }
  .pct-field :global(.label) {
    width: 6px;
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
  .hint {
    padding: 0 16px;
    margin: 4px 0;
  }
</style>
