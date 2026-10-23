<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash-2";
  import Palette from "@lucide/svelte/icons/palette";
  import Hash from "@lucide/svelte/icons/hash";
  import TypeIcon from "@lucide/svelte/icons/type";
  import ToggleLeft from "@lucide/svelte/icons/toggle-left";
  import ColorPicker from "./ColorPicker.svelte";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";
  import { argbAlpha, argbCss, argbHex, packArgb } from "../color";
  import type { VarCollection, VarKind, VarValue, Variable, Variables } from "../engine";

  // Figma's local variables editor: collections on the left, a table of variables with one
  // column per mode. Every edit is applied right away (bound layers follow).

  let draft = $state<Variables>(structuredClone($state.snapshot(editor.variables)));
  let current = $state<string | null>(draft.collections[0]?.id ?? null);
  let createOpen = $state(false);
  let picker = $state<{ variable: string; mode: string; top: number; right: number } | null>(null);
  let search = $state("");

  const collection = $derived(draft.collections.find((c) => c.id === current) ?? null);
  const rows = $derived(
    draft.variables.filter((v) => v.collection === current && v.name.toLowerCase().includes(search.trim().toLowerCase())),
  );

  function commit() {
    editor.setVariables($state.snapshot(draft) as Variables);
  }

  function newId(prefix: string) {
    const used = new Set([
      ...draft.collections.flatMap((c) => [c.id, ...c.modes.map((m) => m.id)]),
      ...draft.variables.map((v) => v.id),
    ]);
    let n = 1;
    while (used.has(`${prefix}${n}`)) n++;
    return `${prefix}${n}`;
  }

  function addCollection() {
    const id = newId("c");
    const mode = `${id}m1`;
    const name = `Collection ${draft.collections.length + 1}`;
    draft.collections.push({ id, name, modes: [{ id: mode, name: "Mode 1" }], active: mode });
    current = id;
    commit();
  }

  function addMode(c: VarCollection) {
    let n = c.modes.length + 1;
    while (c.modes.some((m) => m.id === `${c.id}m${n}`)) n++;
    const id = `${c.id}m${n}`;
    const from = c.modes[c.modes.length - 1]?.id;
    c.modes.push({ id, name: `Mode ${c.modes.length + 1}` });
    // A new mode starts as a copy of the last one.
    for (const v of draft.variables) if (v.collection === c.id && from && v.values[from]) v.values[id] = structuredClone($state.snapshot(v.values[from]));
    commit();
  }

  function removeMode(c: VarCollection, id: string) {
    if (c.modes.length <= 1) return;
    c.modes = c.modes.filter((m) => m.id !== id);
    if (c.active === id) c.active = c.modes[0].id;
    for (const v of draft.variables) if (v.collection === c.id) delete v.values[id];
    commit();
  }

  const DEFAULTS: Record<VarKind, VarValue> = {
    color: { kind: "color", value: 0xff000000 },
    number: { kind: "number", value: 0 },
    string: { kind: "string", value: "" },
    boolean: { kind: "boolean", value: true },
  };

  function addVariable(type: VarKind) {
    if (!collection) addCollection();
    const c = draft.collections.find((x) => x.id === current);
    if (!c) return;
    const base = { color: "color", number: "number", string: "string", boolean: "boolean" }[type];
    let n = 1;
    while (draft.variables.some((v) => v.collection === c.id && v.name === `${base}/${n}`)) n++;
    const values: Record<string, VarValue> = {};
    for (const m of c.modes) values[m.id] = structuredClone(DEFAULTS[type]);
    draft.variables.push({ id: newId("v"), name: `${base}/${n}`, collection: c.id, type, values });
    commit();
  }

  function removeVariable(v: Variable) {
    draft.variables = draft.variables.filter((x) => x.id !== v.id);
    commit();
  }

  function removeCollection(c: VarCollection) {
    draft.variables = draft.variables.filter((v) => v.collection !== c.id);
    draft.collections = draft.collections.filter((x) => x.id !== c.id);
    current = draft.collections[0]?.id ?? null;
    commit();
  }

  function setValue(v: Variable, mode: string, value: VarValue) {
    v.values[mode] = value;
    commit();
  }

  function aliasName(id: string) {
    return draft.variables.find((v) => v.id === id)?.name ?? "missing";
  }

  function openColor(e: MouseEvent, v: Variable, mode: string) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    editor.beginLive();
    picker = { variable: v.id, mode, top: r.top, right: Math.max(8, window.innerWidth - r.left + 8) };
  }

  function closePicker() {
    picker = null;
    editor.endLive();
  }

  const pickedVar = $derived(picker ? draft.variables.find((v) => v.id === picker?.variable) : undefined);
  const pickedValue = $derived(pickedVar && picker ? pickedVar.values[picker.mode] : undefined);

  const createItems = (): MenuItem[] => [
    { label: "Color", action: () => addVariable("color") },
    { label: "Number", action: () => addVariable("number") },
    { label: "String", action: () => addVariable("string") },
    { label: "Boolean", action: () => addVariable("boolean") },
  ];

  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
  };

  $effect(() => {
    const esc = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !picker) editor.variablesOpen = false;
    };
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  });
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && (editor.variablesOpen = false)}>
  <div class="dialog" role="dialog" aria-label="Local variables">
    <aside>
      <div class="side-head">
        <span class="title">Collections</span>
        <button class="icon-btn" title="Create collection" onclick={addCollection}><Plus size={14} strokeWidth={1.8} /></button>
      </div>
      {#each draft.collections as c (c.id)}
        <button class="collection" class:on={c.id === current} onclick={() => (current = c.id)}>
          <span class="name">{c.name}</span>
          <span class="faint count">{draft.variables.filter((v) => v.collection === c.id).length}</span>
        </button>
      {:else}
        <p class="faint pad">No collections yet.</p>
      {/each}
    </aside>
    <section class="main">
      <header>
        {#if collection}
          <input class="collection-name" value={collection.name} onkeydown={stop}
            onblur={(e) => { const v = (e.target as HTMLInputElement).value.trim(); if (v && collection) { collection.name = v; commit(); } }} />
          <label class="active">
            <span class="faint">Showing</span>
            <select value={collection.active} onchange={(e) => { if (collection) { collection.active = (e.target as HTMLSelectElement).value; commit(); } }}>
              {#each collection.modes as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
            </select>
          </label>
          <input class="search" placeholder="Search" bind:value={search} onkeydown={stop} />
          <button class="icon-btn" title="Delete collection" onclick={() => collection && removeCollection(collection)}><Trash size={14} strokeWidth={1.7} /></button>
        {:else}
          <span class="title">Local variables</span>
        {/if}
        <span class="spacer"></span>
        <button class="icon-btn" title="Close" onclick={() => (editor.variablesOpen = false)}><X size={14} strokeWidth={1.8} /></button>
      </header>

      {#if collection}
        <div class="table" style="--modes:{collection.modes.length}">
          <div class="row head">
            <span class="cell name-cell">Name</span>
            {#each collection.modes as m (m.id)}
              <span class="cell mode">
                <input value={m.name} onkeydown={stop} onblur={(e) => { const v = (e.target as HTMLInputElement).value.trim(); if (v) { m.name = v; commit(); } }} />
                {#if collection.modes.length > 1}
                  <button class="icon-btn tiny" title="Delete mode" onclick={() => collection && removeMode(collection, m.id)}><X size={11} strokeWidth={2} /></button>
                {/if}
              </span>
            {/each}
            <button class="icon-btn" title="Add mode" onclick={() => collection && addMode(collection)}><Plus size={14} strokeWidth={1.8} /></button>
          </div>
          {#each rows as v (v.id)}
            <div class="row">
              <span class="cell name-cell">
                <span class="type">
                  {#if v.type === "color"}<Palette size={13} strokeWidth={1.7} />
                  {:else if v.type === "number"}<Hash size={13} strokeWidth={1.7} />
                  {:else if v.type === "string"}<TypeIcon size={13} strokeWidth={1.7} />
                  {:else}<ToggleLeft size={13} strokeWidth={1.7} />{/if}
                </span>
                <input value={v.name} title="Use / for groups (color/brand/primary)" onkeydown={stop}
                  onblur={(e) => { const n = (e.target as HTMLInputElement).value.trim(); if (n) { v.name = n; commit(); } }} />
              </span>
              {#each collection.modes as m (m.id)}
                {@const val = v.values[m.id]}
                <span class="cell">
                  {#if !val}
                    <button class="faint" onclick={() => setValue(v, m.id, structuredClone(DEFAULTS[v.type]))}>Set value</button>
                  {:else if val.kind === "alias"}
                    <span class="alias" title="Alias">→ {aliasName(val.value)}</span>
                  {:else if val.kind === "color"}
                    <button class="swatch" data-picker-trigger aria-label="Edit colour" style="background:{argbCss(val.value)}" onclick={(e) => openColor(e, v, m.id)}></button>
                    <input class="hex" value={argbHex(val.value).slice(1)} onkeydown={stop}
                      onblur={(e) => { const h = (e.target as HTMLInputElement).value.replace("#", ""); if (/^[0-9a-f]{6}$/i.test(h)) setValue(v, m.id, { kind: "color", value: packArgb(h, argbAlpha(val.value)) }); }} />
                    <span class="faint pct">{argbAlpha(val.value)}%</span>
                  {:else if val.kind === "number"}
                    <input value={val.value} onkeydown={stop}
                      onblur={(e) => { const n = Number((e.target as HTMLInputElement).value); if (Number.isFinite(n)) setValue(v, m.id, { kind: "number", value: n }); }} />
                  {:else if val.kind === "string"}
                    <input value={val.value} onkeydown={stop} onblur={(e) => setValue(v, m.id, { kind: "string", value: (e.target as HTMLInputElement).value })} />
                  {:else}
                    <label class="bool"><input type="checkbox" checked={val.value} onchange={(e) => setValue(v, m.id, { kind: "boolean", value: (e.target as HTMLInputElement).checked })} />{val.value ? "True" : "False"}</label>
                  {/if}
                </span>
              {/each}
              <button class="icon-btn row-del" title="Delete variable" onclick={() => removeVariable(v)}><Trash size={13} strokeWidth={1.7} /></button>
            </div>
          {/each}
        </div>
      {:else}
        <div class="empty">
          <p>Variables store reusable values — colours, numbers, strings and booleans — with modes such as Light and Dark.</p>
          <p class="faint">Bind them to fills, strokes, radii, spacing and text; design to code turns them into CSS custom properties.</p>
        </div>
      {/if}

      <footer>
        <div class="anchor">
          <button class="create" data-menu-trigger onclick={() => (createOpen = !createOpen)}><Plus size={14} strokeWidth={1.8} />Create variable</button>
          <Menu items={createOpen ? createItems() : []} bind:open={createOpen} placement="above" />
        </div>
      </footer>
    </section>
  </div>
</div>

{#if picker && pickedValue?.kind === "color"}
  <ColorPicker
    hex={argbHex(pickedValue.value)}
    alpha={argbAlpha(pickedValue.value)}
    top={picker.top}
    right={picker.right}
    onchange={(hex, a) => {
      if (pickedVar && picker) {
        pickedVar.values[picker.mode] = { kind: "color", value: packArgb(hex, a) };
        editor.engine?.setVariables($state.snapshot(draft) as Variables);
        editor.refreshAll();
      }
    }}
    onclose={closePicker}
  />
{/if}

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
  }
  .dialog {
    display: flex;
    width: min(1000px, calc(100vw - 80px));
    height: min(600px, calc(100vh - 80px));
    background: var(--bg, #2c2c2c);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
    overflow: hidden;
  }
  aside {
    width: 220px;
    flex: none;
    border-right: 1px solid var(--border);
    padding: 0 8px;
    overflow-y: auto;
  }
  .side-head,
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
  }
  .side-head {
    justify-content: space-between;
    padding-left: 8px;
  }
  header {
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-weight: 600;
  }
  .collection {
    display: flex;
    align-items: center;
    width: 100%;
    height: 32px;
    padding: 0 8px;
    border-radius: var(--radius);
    text-align: left;
  }
  .collection.on {
    background: var(--bg-secondary);
  }
  .collection .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .spacer {
    flex: 1;
  }
  input {
    min-width: 0;
    height: 26px;
    padding: 0 6px;
    background: transparent;
    border: 1px solid transparent;
    border-radius: var(--radius);
    color: var(--text);
    outline: none;
  }
  input:hover {
    border-color: var(--border);
  }
  input:focus {
    border-color: var(--brand-strong);
    background: var(--bg-secondary);
  }
  .collection-name {
    font-weight: 600;
    width: 180px;
  }
  .search {
    width: 160px;
    background: var(--bg-secondary);
  }
  .active {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  select {
    height: 26px;
    background: var(--bg-secondary);
    color: var(--text);
    border: none;
    border-radius: var(--radius);
    padding: 0 6px;
  }
  .table {
    flex: 1;
    overflow: auto;
  }
  .row {
    display: grid;
    grid-template-columns: minmax(200px, 1.2fr) repeat(var(--modes), minmax(160px, 1fr)) 32px;
    align-items: center;
    min-height: 36px;
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--border);
  }
  .row.head {
    position: sticky;
    top: 0;
    background: var(--bg, #2c2c2c);
    color: var(--text-secondary);
    z-index: 1;
  }
  .cell {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
    padding-right: 8px;
  }
  .cell input {
    flex: 1;
  }
  .mode input {
    font-weight: 550;
  }
  .type {
    display: flex;
    color: var(--text-secondary);
  }
  .swatch {
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 3px;
    box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.15);
  }
  .hex {
    text-transform: uppercase;
    font-variant-numeric: tabular-nums;
    max-width: 90px;
  }
  .pct {
    font-variant-numeric: tabular-nums;
  }
  .alias {
    padding: 2px 6px;
    border-radius: 4px;
    background: var(--bg-secondary);
  }
  .bool {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .tiny {
    width: 18px;
    height: 18px;
  }
  .row-del {
    opacity: 0;
  }
  .row:hover .row-del {
    opacity: 1;
  }
  .empty {
    flex: 1;
    padding: 32px;
    line-height: 1.6;
    max-width: 520px;
  }
  .pad {
    padding: 0 8px;
  }
  footer {
    display: flex;
    padding: 8px 16px;
    border-top: 1px solid var(--border);
  }
  .anchor {
    position: relative;
  }
  .create {
    display: flex;
    align-items: center;
    gap: 6px;
    height: 30px;
    padding: 0 10px;
    border-radius: var(--radius);
  }
  .create:hover {
    background: var(--bg-secondary);
  }
</style>
