<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import { editor } from "../editor.svelte";
  import type { Action, NodeInfo, OverlayPosition, Reaction, Scroll, Transition, VarValue } from "../engine";

  // Figma's Prototype tab: the selected layer's interactions.

  let { n }: { n: NodeInfo } = $props();

  const screens = $derived(editor.engine?.screens() ?? []);
  /** "Change to": the other variants of the layer's component set. */
  const variants = $derived(editor.engine?.variantsOf(n.id) ?? []);
  const TRANSITIONS: [Transition, string][] = [
    ["instant", "Instant"],
    ["dissolve", "Dissolve"],
    ["smart-animate", "Smart animate"],
    ["slide-left", "Slide in ←"],
    ["slide-right", "Slide in →"],
    ["slide-up", "Slide in ↑"],
    ["slide-down", "Slide in ↓"],
    ["push", "Push"],
  ];

  function save(list: Reaction[]) {
    editor.engine?.setReactions(n.id, list);
    editor.refreshAll();
  }

  function add() {
    const own = editor.engine?.screenOf(n.id) ?? -1;
    const target = screens.find((s) => s.id !== own) ?? screens[0];
    if (!target) return editor.notify("Add a second frame to navigate to");
    save([...n.reactions, { trigger: { type: "on-click" }, action: { type: "navigate", to: target.id }, transition: "dissolve", duration: 300 }]);
  }

  function change(i: number, r: Reaction) {
    save(n.reactions.map((x, k) => (k === i ? r : x)));
  }

  function setTrigger(i: number, r: Reaction, v: string) {
    const trigger = v === "after-delay" ? { type: "after-delay" as const, ms: 800 } : { type: v as "on-click" | "on-hover" };
    change(i, { ...r, trigger });
  }

  function setAction(i: number, r: Reaction, v: string) {
    if (v === "back") return change(i, { ...r, action: { type: "back" } });
    if (v === "open-url") return change(i, { ...r, action: { type: "open-url", url: "https://" } });
    if (v.startsWith("change:")) return change(i, { ...r, action: { type: "change-to", to: Number(v.slice(7)) }, transition: "instant" });
    if (v.startsWith("overlay:")) return change(i, { ...r, action: { type: "open-overlay", to: Number(v.slice(8)), position: "center", closeOutside: true, dim: true } });
    if (v === "close-overlay") return change(i, { ...r, action: { type: "close-overlay" } });
    if (v === "set-variable") {
      const first = editor.variables.variables[0];
      if (!first) return editor.notify("Create a variable first (Variables panel)");
      return change(i, { ...r, action: { type: "set-variable", variable: first.id, value: blank(first.type) } });
    }
    change(i, { ...r, action: { type: "navigate", to: Number(v) } });
  }

  const actionValue = (r: Reaction) =>
    r.action.type === "navigate"
      ? String(r.action.to)
      : r.action.type === "change-to"
        ? `change:${r.action.to}`
        : r.action.type === "open-overlay"
          ? `overlay:${r.action.to}`
          : r.action.type;

  const POSITIONS: [OverlayPosition, string][] = [
    ["center", "Center"],
    ["top", "Top"],
    ["bottom", "Bottom"],
    ["left", "Left"],
    ["right", "Right"],
  ];

  function blank(kind: string): VarValue {
    if (kind === "number") return { kind: "number", value: 0 };
    if (kind === "boolean") return { kind: "boolean", value: true };
    if (kind === "color") return { kind: "color", value: 0xff000000 };
    return { kind: "string", value: "" };
  }

  /** A variable value typed in the panel, read as the variable's own kind. */
  function parseValue(variable: string, text: string): VarValue {
    const kind = editor.variables.variables.find((v) => v.id === variable)?.type ?? "string";
    if (kind === "number") return { kind: "number", value: Number(text) || 0 };
    if (kind === "boolean") return { kind: "boolean", value: text.trim().toLowerCase() === "true" };
    if (kind === "color") {
      const hex = text.replace("#", "").padEnd(6, "0").slice(0, 6);
      return { kind: "color", value: (0xff000000 | parseInt(hex, 16)) >>> 0 };
    }
    return { kind: "string", value: text };
  }

  function showValue(v: VarValue) {
    if (v.kind === "color") return "#" + (v.value & 0xffffff).toString(16).padStart(6, "0").toUpperCase();
    return String(v.value);
  }

  /** Conditionals and scroll-to come from agents; the panel describes them. */
  function describe(a: Action): string {
    const name = (id: number) => screens.find((s) => s.id === id)?.name ?? `layer ${id}`;
    const varName = (id: string) => editor.variables.variables.find((v) => v.id === id)?.name ?? id;
    switch (a.type) {
      case "conditional":
        return `If ${varName(a.variable)} = ${showValue(a.equals)}: ${a.then.map(describe).join(", ") || "nothing"}; else ${a.otherwise.map(describe).join(", ") || "nothing"}`;
      case "scroll-to":
        return `Scroll to ${name(a.to)}`;
      case "navigate":
        return `navigate to ${name(a.to)}`;
      case "set-variable":
        return `set ${varName(a.variable)} = ${showValue(a.value)}`;
      case "open-overlay":
        return `open overlay ${name(a.to)}`;
      default:
        return a.type.replace("-", " ");
    }
  }
</script>

{#if n.kind === "Frame" && !n.section}
  <div class="section-head">
    <span class="section-title">Scroll behavior</span>
  </div>
  <div class="line overflow">
    <span class="faint">Overflow</span>
    <select value={n.scroll} onchange={(e) => editor.patch({ scroll: (e.target as HTMLSelectElement).value as Scroll }, n.id)}>
      <option value="none">No scrolling</option>
      <option value="vertical">Vertical</option>
      <option value="horizontal">Horizontal</option>
      <option value="both">Both directions</option>
    </select>
  </div>
{/if}
<div class="section-head">
  <span class="section-title">Interactions</span>
  <button class="icon-btn" title="Add interaction" onclick={add}><Plus size={14} strokeWidth={1.7} /></button>
</div>
{#each n.reactions as r, i (i)}
  <div class="card">
    <div class="line">
      <select value={r.trigger.type} onchange={(e) => setTrigger(i, r, (e.target as HTMLSelectElement).value)}>
        <option value="on-click">On click</option>
        <option value="on-hover">While hovering</option>
        <option value="after-delay">After delay</option>
      </select>
      {#if r.trigger.type === "after-delay"}
        <input class="ms" value={r.trigger.ms} title="Delay (ms)" onkeydown={(e) => e.stopPropagation()}
          onblur={(e) => { const v = Number((e.target as HTMLInputElement).value); if (Number.isFinite(v)) change(i, { ...r, trigger: { type: "after-delay", ms: Math.max(0, v) } }); }} />
      {/if}
      <button class="icon-btn" title="Remove interaction" onclick={() => save(n.reactions.filter((_, k) => k !== i))}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
    <div class="line">
      <select value={actionValue(r)} onchange={(e) => setAction(i, r, (e.target as HTMLSelectElement).value)}>
        <optgroup label="Navigate to">
          {#each screens as s (s.id)}<option value={String(s.id)}>{s.name}</option>{/each}
        </optgroup>
        {#if variants.length > 1}
          <optgroup label="Change to">
            {#each variants as v (v.id)}<option value="change:{v.id}">{v.name}</option>{/each}
          </optgroup>
        {/if}
        <optgroup label="Open overlay">
          {#each screens as s (s.id)}<option value="overlay:{s.id}">{s.name}</option>{/each}
        </optgroup>
        <option value="close-overlay">Close overlay</option>
        <option value="set-variable">Set variable</option>
        <option value="back">Back</option>
        <option value="open-url">Open link</option>
        {#if r.action.type === "conditional" || r.action.type === "scroll-to"}
          <option value={r.action.type}>{r.action.type === "conditional" ? "Conditional" : "Scroll to"}</option>
        {/if}
      </select>
    </div>
    {#if r.action.type === "open-overlay"}
      {@const a = r.action}
      <div class="line">
        <select value={a.position} title="Overlay position" onchange={(e) => change(i, { ...r, action: { ...a, position: (e.target as HTMLSelectElement).value as OverlayPosition } })}>
          {#each POSITIONS as [v, label] (v)}<option value={v}>{label}</option>{/each}
        </select>
      </div>
      <label class="line check"><input type="checkbox" checked={a.closeOutside} onchange={(e) => change(i, { ...r, action: { ...a, closeOutside: (e.target as HTMLInputElement).checked } })} />Close when clicking outside</label>
      <label class="line check"><input type="checkbox" checked={a.dim} onchange={(e) => change(i, { ...r, action: { ...a, dim: (e.target as HTMLInputElement).checked } })} />Darken background</label>
    {:else if r.action.type === "set-variable"}
      {@const a = r.action}
      <div class="line">
        <select value={a.variable} onchange={(e) => {
          const id = (e.target as HTMLSelectElement).value;
          change(i, { ...r, action: { ...a, variable: id, value: blank(editor.variables.variables.find((v) => v.id === id)?.type ?? "string") } });
        }}>
          {#each editor.variables.variables as v (v.id)}<option value={v.id}>{v.name}</option>{/each}
        </select>
        <input class="url" value={showValue(a.value)} title="New value" onkeydown={(e) => e.stopPropagation()}
          onblur={(e) => change(i, { ...r, action: { ...a, value: parseValue(a.variable, (e.target as HTMLInputElement).value) } })} />
      </div>
    {:else if r.action.type === "conditional" || r.action.type === "scroll-to"}
      <p class="hint faint describe">{describe(r.action)}</p>
    {/if}
    {#if r.action.type === "open-url"}
      <div class="line">
        <input class="url" value={r.action.url} onkeydown={(e) => e.stopPropagation()}
          onblur={(e) => change(i, { ...r, action: { type: "open-url", url: (e.target as HTMLInputElement).value } })} />
      </div>
    {:else if r.action.type === "navigate"}
      <div class="line">
        <select value={r.transition} onchange={(e) => change(i, { ...r, transition: (e.target as HTMLSelectElement).value as Transition })}>
          {#each TRANSITIONS as [v, label] (v)}<option value={v}>{label}</option>{/each}
        </select>
        {#if r.transition !== "instant"}
          <input class="ms" value={r.duration} title="Duration (ms)" onkeydown={(e) => e.stopPropagation()}
            onblur={(e) => { const v = Number((e.target as HTMLInputElement).value); if (Number.isFinite(v)) change(i, { ...r, duration: Math.max(0, v) }); }} />
        {/if}
      </div>
    {/if}
  </div>
{:else}
  <p class="hint faint">Click + to add an interaction (e.g. on click → navigate to another frame). Press Present (▷) to try it.</p>
{/each}

<style>
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
  }
  .card {
    margin: 0 8px 8px 16px;
    padding: 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .line {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .check {
    gap: 6px;
    font-size: 11px;
  }
  .check input {
    flex: none;
    height: auto;
  }
  .describe {
    margin: 0;
    font-size: 11px;
    line-height: 1.45;
  }
  .overflow {
    padding: 0 8px 8px 16px;
  }
  .overflow .faint {
    flex: none;
    width: 64px;
  }
  select,
  input {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 6px;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text);
    outline: none;
  }
  input:focus {
    border-color: var(--brand-strong);
  }
  .ms {
    flex: none;
    width: 64px;
  }
  .hint {
    padding: 0 16px;
    line-height: 1.5;
  }
</style>
