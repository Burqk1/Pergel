<script lang="ts">
  import Play from "@lucide/svelte/icons/play";
  import Plus from "@lucide/svelte/icons/plus";
  import Trash from "@lucide/svelte/icons/trash-2";
  import Upload from "@lucide/svelte/icons/upload";
  import { editor } from "../editor.svelte";
  import { loadPlugins, runScript, savePlugins, type Plugin } from "../plugins";
  import { readFigmaPlugin, runFigmaPlugin } from "../figmaPlugin.svelte";
  import Figma from "@lucide/svelte/icons/puzzle";

  // Figma's Plugins: scripts against the document through the `pergel` API (the MCP tools).

  const initial = loadPlugins();
  let plugins = $state<Plugin[]>(initial);
  let current = $state<string>(initial[0]?.id ?? "");
  let running = $state(false);
  let output = $state<string[]>([]);
  const plugin = $derived(plugins.find((p) => p.id === current));

  function persist() {
    savePlugins(plugins);
  }

  function create(name = "New plugin", code = "// pergel.selection(), pergel.create([...]), pergel.update(id, {...}), pergel.notify('…')\nconst sel = await pergel.selection();\nconsole.log(sel.selection.length + ' layers selected');\n") {
    const p: Plugin = { id: `p${Date.now()}`, name, code };
    plugins = [...plugins, p];
    current = p.id;
    persist();
  }

  function remove() {
    if (!plugin || plugin.builtin) return;
    plugins = plugins.filter((p) => p.id !== plugin.id);
    current = plugins[0]?.id ?? "";
    persist();
  }

  function edit(change: Partial<Plugin>) {
    if (!plugin || plugin.builtin) return;
    plugins = plugins.map((p) => (p.id === plugin.id ? { ...p, ...change } : p));
    persist();
  }

  function importFile() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".js,.mjs,.txt";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (file) create(file.name.replace(/\.(m?js|txt)$/, ""), await file.text());
    };
    input.click();
  }

  /** A Figma plugin from its folder (manifest.json, main script, UI). */
  function importFigma() {
    const input = document.createElement("input");
    input.type = "file";
    input.multiple = true;
    (input as HTMLInputElement & { webkitdirectory: boolean }).webkitdirectory = true;
    input.onchange = async () => {
      try {
        const p = await readFigmaPlugin([...(input.files ?? [])]);
        const made: Plugin = { id: `f${Date.now()}`, name: p.name, code: p.code, html: p.html, figma: true };
        plugins = [...plugins, made];
        current = made.id;
        persist();
        editor.notify(`Imported ${p.name}`);
      } catch (e) {
        editor.notify(String(e instanceof Error ? e.message : e));
      }
    };
    input.click();
  }

  async function run() {
    if (!plugin || running) return;
    if (plugin.figma) {
      running = true;
      output = [`Running ${plugin.name} (Figma plugin API)…`];
      await runFigmaPlugin({ name: plugin.name, code: plugin.code, html: plugin.html ?? "" });
      output = [...output, "Closed (each run and each UI message is one undo step)"];
      running = false;
      return;
    }
    running = true;
    output = [];
    const r = await runScript(plugin.code, (line) => (output = [...output, line]));
    if (!r.ok) output = [...output, `Error: ${r.error}`];
    else if (r.result !== null && r.result !== undefined) output = [...output, `→ ${JSON.stringify(r.result, null, 1)}`];
    output = [...output, `Done in ${r.ms} ms (one undo step)`];
    running = false;
  }
</script>

<section class="plugins">
  <div class="head">
    <span class="section-title">Plugins</span>
    <span class="actions">
      <button class="icon-btn" title="Import a Figma plugin (its folder with manifest.json)" onclick={importFigma}><Figma size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="Import a .js plugin" onclick={importFile}><Upload size={14} strokeWidth={1.7} /></button>
      <button class="icon-btn" title="New plugin" onclick={() => create()}><Plus size={14} strokeWidth={1.7} /></button>
    </span>
  </div>
  <div class="list">
    {#each plugins as p (p.id)}
      <button class="item" class:on={p.id === current} onclick={() => (current = p.id)}>
        <span class="name">{p.name}</span>
        {#if p.figma}<span class="faint tag">Figma</span>{:else if p.builtin}<span class="faint tag">built-in</span>{/if}
      </button>
    {/each}
  </div>
  {#if plugin}
    <div class="editor">
      {#if !plugin.builtin}
        <input class="title" value={plugin.name} onkeydown={(e) => e.stopPropagation()} onchange={(e) => edit({ name: (e.target as HTMLInputElement).value })} />
      {/if}
      <textarea
        spellcheck="false"
        readonly={plugin.builtin}
        value={plugin.code}
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) run();
        }}
        oninput={(e) => edit({ code: (e.target as HTMLTextAreaElement).value })}
      ></textarea>
      <div class="row">
        <button class="primary" disabled={running} onclick={run}><Play size={12} strokeWidth={2} />{running ? "Running…" : "Run"}</button>
        {#if plugin.builtin}
          <button class="ghost" onclick={() => create(`${plugin.name} copy`, plugin.code)}>Duplicate to edit</button>
        {:else}
          <button class="icon-btn" title="Delete plugin" onclick={remove}><Trash size={14} strokeWidth={1.7} /></button>
        {/if}
      </div>
      {#if output.length}
        <pre class="output">{output.join("\n")}</pre>
      {/if}
      <p class="faint hint">
        API: <code>pergel.selection()</code>, <code>metadata(id)</code>, <code>create(specs, parentId)</code>, <code>update(id, props)</code>,
        <code>remove(ids)</code>, <code>select(ids)</code>, <code>designContext(id)</code>, <code>searchIcons(q)</code>, <code>notify(msg)</code>,
        <code>tool(name, args)</code> for any MCP tool. Ctrl+Enter runs.
      </p>
    </div>
  {/if}
  {#if editor.engine === null}<p class="faint hint">Loading…</p>{/if}
</section>

<style>
  .plugins {
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
  }
  .list {
    display: flex;
    flex-direction: column;
    padding: 0 8px;
    max-height: 160px;
    overflow: auto;
  }
  .item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    text-align: left;
  }
  .item:hover {
    background: var(--bg-hover, rgba(255, 255, 255, 0.06));
  }
  .item.on {
    background: var(--bg-selected, rgba(13, 153, 255, 0.2));
  }
  .tag {
    font-size: 10px;
  }
  .editor {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px 16px 16px;
    min-height: 0;
  }
  .title {
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
  }
  textarea {
    min-height: 180px;
    resize: vertical;
    padding: 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    font: 11px/1.5 ui-monospace, "Cascadia Code", Consolas, monospace;
    outline: none;
    tab-size: 2;
  }
  textarea:focus,
  .title:focus {
    border-color: var(--brand-strong);
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .primary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .primary:disabled {
    opacity: 0.6;
  }
  .ghost {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .output {
    margin: 0;
    max-height: 200px;
    overflow: auto;
    padding: 8px;
    border-radius: var(--radius);
    background: #111;
    color: #d4d4d4;
    font: 11px/1.45 ui-monospace, Consolas, monospace;
    white-space: pre-wrap;
  }
  .hint {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
  }
  code {
    font-size: 10.5px;
  }
</style>
