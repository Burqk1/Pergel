<script lang="ts">
  import Plus from "@lucide/svelte/icons/plus";
  import Minus from "@lucide/svelte/icons/minus";
  import { editor } from "../editor.svelte";
  import type { ComponentProp, NodeInfo, PropKind, PropValue } from "../engine";

  // Figma's component properties: define them on a main component or set, link layers inside
  // it to them, and set them per instance.

  let { n }: { n: NodeInfo } = $props();

  const list = $derived(n.componentProps ?? []);
  const mains = $derived(editor.engine?.components() ?? []);
  const KIND_LABEL: Record<PropKind, string> = { boolean: "Boolean", text: "Text", "instance-swap": "Instance swap" };
  let adding = $state(false);

  function defs(): ComponentProp[] {
    return list.map(({ id, name, kind, default: d }) => ({ id, name, kind, default: d }));
  }

  function saveDefs(next: ComponentProp[]) {
    editor.patch({ componentProps: next }, n.propsOwner ?? n.id);
  }

  function add(kind: PropKind) {
    adding = false;
    const count = list.filter((p) => p.kind === kind).length + 1;
    const name = kind === "boolean" ? `Show ${count > 1 ? `item ${count}` : "item"}` : kind === "text" ? (count > 1 ? `Text ${count}` : "Text") : count > 1 ? `Icon ${count}` : "Icon";
    const fallback: PropValue = kind === "boolean" ? true : kind === "text" ? "" : (mains[0]?.id ?? 0);
    const next = [...defs(), { id: "", name, kind, default: fallback }];
    saveDefs(next);
    return next;
  }

  /** Link mode: create a property from this layer and link it right away. */
  function createAndLink(kind: PropKind) {
    const defaultValue: PropValue = kind === "boolean" ? n.visible : kind === "text" ? (n.text?.content ?? "") : (n.main ?? 0);
    const base = kind === "boolean" ? `Show ${n.name}` : kind === "text" ? n.name : n.name;
    saveDefs([...defs(), { id: "", name: base, kind, default: defaultValue }]);
    // The owner now has the new property; find its id and link it.
    editor.refreshSelection();
    const fresh = editor.selection.find((s) => s.id === n.id)?.componentProps ?? [];
    const made = [...fresh].reverse().find((p) => p.kind === kind && p.name === base);
    if (made) link(kind, made.id);
  }

  function link(kind: PropKind, id: string) {
    const refs = { ...(n.propRefs ?? {}) };
    const key = kind === "boolean" ? "visible" : kind === "text" ? "text" : "swap";
    if (id) refs[key] = id;
    else delete refs[key];
    editor.patch({ propRefs: refs }, n.id);
  }

  function setValue(id: string, value: PropValue | null) {
    editor.patch({ propValues: { [id]: value } }, n.id);
  }

  const linked = (kind: PropKind) => (kind === "boolean" ? n.propRefs?.visible : kind === "text" ? n.propRefs?.text : n.propRefs?.swap) ?? "";
  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };
</script>

{#if n.propsMode === "define"}
  <div class="section-head">
    <span class="section-title">Properties</span>
    <span class="anchor">
      <button class="icon-btn" title="Create component property" onclick={() => (adding = !adding)}><Plus size={14} strokeWidth={1.7} /></button>
      {#if adding}
        <div class="menu">
          {#each ["boolean", "text", "instance-swap"] as const as k (k)}
            <button onclick={() => add(k)}>{KIND_LABEL[k]}</button>
          {/each}
        </div>
      {/if}
    </span>
  </div>
  {#each list as p (p.id)}
    <div class="prop">
      <input class="name" value={p.name} title="Property name" onkeydown={stop}
        onchange={(e) => saveDefs(defs().map((d) => (d.id === p.id ? { ...d, name: (e.target as HTMLInputElement).value || d.name } : d)))} />
      <span class="kind faint">{KIND_LABEL[p.kind]}</span>
      {#if p.kind === "boolean"}
        <input type="checkbox" title="Default" checked={p.default === true} onchange={(e) => saveDefs(defs().map((d) => (d.id === p.id ? { ...d, default: (e.target as HTMLInputElement).checked } : d)))} />
      {:else if p.kind === "text"}
        <input class="value" title="Default" value={String(p.default)} onkeydown={stop} onchange={(e) => saveDefs(defs().map((d) => (d.id === p.id ? { ...d, default: (e.target as HTMLInputElement).value } : d)))} />
      {:else}
        <select class="value" title="Default component" value={String(p.default)} onchange={(e) => saveDefs(defs().map((d) => (d.id === p.id ? { ...d, default: Number((e.target as HTMLSelectElement).value) } : d)))}>
          {#each mains as m (m.id)}<option value={String(m.id)}>{m.name}</option>{/each}
        </select>
      {/if}
      <button class="icon-btn" title="Remove property" onclick={() => saveDefs(defs().filter((d) => d.id !== p.id))}><Minus size={14} strokeWidth={1.7} /></button>
    </div>
  {:else}
    <p class="hint faint">Add boolean, text or instance-swap properties, then link layers inside the component to them.</p>
  {/each}
{:else if n.propsMode === "values" && list.length}
  <div class="section-head"><span class="section-title">Properties</span></div>
  {#each list as p (p.id)}
    <div class="prop">
      <span class="name-label">{p.name}</span>
      {#if p.kind === "boolean"}
        <input type="checkbox" checked={p.value === true} onchange={(e) => setValue(p.id, (e.target as HTMLInputElement).checked)} />
      {:else if p.kind === "text"}
        <input class="value" value={String(p.value ?? "")} onkeydown={stop} onchange={(e) => setValue(p.id, (e.target as HTMLInputElement).value)} />
      {:else}
        <select class="value" value={String(p.value)} onchange={(e) => setValue(p.id, Number((e.target as HTMLSelectElement).value))}>
          {#each mains as m (m.id)}<option value={String(m.id)}>{m.name}</option>{/each}
        </select>
      {/if}
      {#if p.value !== p.default}
        <button class="icon-btn reset" title="Back to the default" onclick={() => setValue(p.id, null)}>↺</button>
      {/if}
    </div>
  {/each}
{:else if n.propsMode === "link"}
  {@const kinds = ([["boolean", "Visibility"]] as [PropKind, string][]).concat(n.text ? [["text", "Text"]] : [], n.component === "instance" ? [["instance-swap", "Swap"]] : [])}
  <div class="section-head"><span class="section-title">Component property</span></div>
  {#each kinds as [kind, label] (kind)}
    <div class="prop">
      <span class="name-label">{label}</span>
      <select class="value" value={linked(kind)}
        onchange={(e) => {
          const v = (e.target as HTMLSelectElement).value;
          if (v === "__new") createAndLink(kind);
          else link(kind, v);
        }}>
        <option value="">Not linked</option>
        {#each list.filter((p) => p.kind === kind) as p (p.id)}<option value={p.id}>{p.name}</option>{/each}
        <option value="__new">Create property…</option>
      </select>
    </div>
  {/each}
{/if}

<style>
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
  }
  .anchor {
    position: relative;
  }
  .menu {
    position: absolute;
    right: 0;
    top: 26px;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 140px;
    padding: 4px;
    border-radius: 8px;
    background: var(--menu-bg, #1e1e1e);
    box-shadow: var(--shadow-menu);
  }
  .menu button {
    height: 26px;
    padding: 0 8px;
    border-radius: 4px;
    text-align: left;
  }
  .menu button:hover {
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  .prop {
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr) 24px;
    align-items: center;
    gap: 6px;
    padding: 2px 8px 2px 16px;
  }
  .prop:has(.name-label) {
    grid-template-columns: 84px minmax(0, 1fr) 24px;
  }
  .name,
  .value {
    height: 26px;
    min-width: 0;
    padding: 0 6px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  .name:focus,
  .value:focus {
    border-color: var(--brand-strong);
  }
  .kind {
    font-size: 10px;
  }
  .name-label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .hint {
    margin: 0;
    padding: 0 16px 8px;
    font-size: 11px;
  }
  .reset {
    font-size: 13px;
  }
</style>
