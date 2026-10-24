<script lang="ts">
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import ChevronRight from "@lucide/svelte/icons/chevron-right";
  import ChevronsDownUp from "@lucide/svelte/icons/chevrons-down-up";
  import PanelTop from "@lucide/svelte/icons/panel-top";
  import SliceIcon from "@lucide/svelte/icons/slice";
  import Hash from "@lucide/svelte/icons/hash";
  import Square from "@lucide/svelte/icons/square";
  import SquareDashed from "@lucide/svelte/icons/square-dashed";
  import Circle from "@lucide/svelte/icons/circle";
  import Type from "@lucide/svelte/icons/type";
  import Check from "@lucide/svelte/icons/check";
  import Crosshair from "@lucide/svelte/icons/crosshair";
  import AgentsPanel from "./AgentsPanel.svelte";
  import Upload from "@lucide/svelte/icons/upload";
  import { libraries } from "../libraries.svelte";
  import PluginsPanel from "./PluginsPanel.svelte";
  import IconsPanel from "./IconsPanel.svelte";
  import PenTool from "@lucide/svelte/icons/pen-tool";
  import ImageIcon from "@lucide/svelte/icons/image";
  import MaskIcon from "@lucide/svelte/icons/contrast";
  import ComponentIcon from "@lucide/svelte/icons/component";
  import Diamond from "@lucide/svelte/icons/diamond";
  import Eye from "@lucide/svelte/icons/eye";
  import EyeOff from "@lucide/svelte/icons/eye-off";
  import Lock from "@lucide/svelte/icons/lock";
  import LockOpen from "@lucide/svelte/icons/lock-open";
  import Plus from "@lucide/svelte/icons/plus";
  import PanelLeftClose from "@lucide/svelte/icons/panel-left-close";
  import Search from "@lucide/svelte/icons/search";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";
  import { workspace } from "../workspace.svelte";
  import { exportToDisk } from "../files";
  import { layerMenu } from "../layerMenu";
  import type { LayerInfo } from "../engine";

  const ROW = 32;
  const INDENT = 16;
  let menuOpen = $state(false);
  let renamingFile = $state(false);
  let rowMenu = $state<{ x: number; y: number } | null>(null);
  let rowMenuOpen = $state(false);
  let pageMenu = $state<{ x: number; y: number; index: number } | null>(null);
  let layerSearch = $state<string | null>(null);

  // Layer thumbnails (Figma UI3): small renders of the visible rows, refreshed after edits.
  let thumbs = $state(new Map<number, string>());
  let thumbRev = -1;
  let thumbTimer: ReturnType<typeof setTimeout> | undefined;
  const THUMB_KINDS = new Set(["Frame", "Section", "Rectangle", "Ellipse", "Vector", "Image", "Mask"]);
  $effect(() => {
    void editor.layers;
    void start;
    void end;
    clearTimeout(thumbTimer);
    thumbTimer = setTimeout(() => {
      const engine = editor.engine;
      if (!engine) return;
      const rev = engine.docRevision();
      const next = rev === thumbRev ? new Map(thumbs) : new Map<number, string>();
      let changed = rev !== thumbRev;
      for (const l of rows.slice(start, end)) {
        if (!THUMB_KINDS.has(l.kind) || next.has(l.id)) continue;
        const url = engine.thumbnail(l.id, 40);
        if (url) {
          next.set(l.id, url);
          changed = true;
        }
      }
      thumbRev = rev;
      if (changed) thumbs = next;
    }, 250);
    return () => clearTimeout(thumbTimer);
  });
  $effect(() => {
    if (editor.focusLayerSearch) {
      editor.focusLayerSearch = false;
      editor.leftTab = "file";
      layerSearch = layerSearch ?? "";
    }
  });
  /** Layers whose name matches the search, with the frame they are in. */
  const matches = $derived.by(() => {
    const q = layerSearch?.trim().toLowerCase();
    if (!q) return [];
    const byId = new Map(editor.layers.map((l) => [l.id, l]));
    return editor.layers
      .filter((l) => l.name.toLowerCase().includes(q))
      .slice(0, 300)
      .map((l) => {
        let p = byId.get(l.parent);
        while (p && p.parent >= 0 && byId.get(p.parent)) p = byId.get(p.parent);
        return { ...l, path: p && p.id !== l.id ? p.name : "" };
      });
  });
  let pageMenuOpen = $state(false);

  function pageItems(i: number): MenuItem[] {
    return [
      { label: "Rename page", action: () => (editor.renamingPage = i) },
      { label: "Duplicate page", action: () => editor.duplicatePage(i) },
      { label: "Delete page", disabled: editor.pages.length <= 1, action: () => editor.deletePage(i) },
    ];
  }
  let scrollTop = $state(0);
  let viewport = $state(0);
  let list = $state<HTMLDivElement>();
  let anchor = -1;

  type Place = "above" | "below" | "inside";
  let drag = $state<{ id: number; y: number; active: boolean; target: number; place: Place } | null>(null);

  /** Layers in panel order, without the children of collapsed layers. */
  const rows = $derived.by(() => {
    const out: LayerInfo[] = [];
    let skip = Infinity;
    for (const l of editor.layers) {
      if (l.depth > skip) continue;
      skip = Infinity;
      out.push(l);
      if (l.children > 0 && !editor.expanded.has(l.id)) skip = l.depth;
    }
    return out;
  });
  const start = $derived(Math.max(0, Math.floor(scrollTop / ROW) - 6));
  const end = $derived(Math.min(rows.length, Math.ceil((scrollTop + viewport) / ROW) + 6));
  const selected = $derived(editor.selectedIds);

  /** Rows inside a selected layer get the lighter selection tint, as in Figma. */
  function insideSelection(layer: LayerInfo) {
    for (let p = layer.parent; p >= 0; p = editor.parentOf.get(p) ?? -1) if (selected.has(p)) return true;
    return false;
  }

  // Ctrl+R on the canvas: show the row being renamed.
  $effect(() => {
    const id = editor.renaming;
    if (id < 0 || !list) return;
    const i = rows.findIndex((r) => r.id === id);
    if (i >= 0 && (i * ROW < list.scrollTop || i * ROW + ROW > list.scrollTop + list.clientHeight)) {
      list.scrollTop = Math.max(0, i * ROW - list.clientHeight / 2);
    }
  });

  // Keep the selected layer in view when it was selected on the canvas.
  $effect(() => {
    const first = editor.selection[0];
    if (!first || !list || drag) return;
    const i = rows.findIndex((r) => r.id === first.id);
    if (i < 0) return;
    const y = i * ROW;
    if (y < list.scrollTop || y + ROW > list.scrollTop + list.clientHeight) {
      list.scrollTop = Math.max(0, y - list.clientHeight / 2);
    }
  });

  const mainMenu = (): MenuItem[] => [
    { label: "Back to files", shortcut: "Ctrl+O", action: () => workspace.goHome() },
    { label: "New design file", shortcut: "Ctrl+T", action: () => workspace.newFile() },
    { label: "Import from Figma…", action: () => (editor.figmaImport = true) },
    { label: "Show version history", action: () => (editor.versionsOpen = true) },
    { label: "Place image…", shortcut: "Ctrl+Shift+K", action: () => editor.placeImageDialog() },
    {
      label: "Save local copy…",
      disabled: !workspace.current,
      action: async () => {
        await workspace.saveCurrent();
        const file = workspace.current?.file;
        if (file && !(await exportToDisk(file))) editor.notify("Saving a local copy needs the desktop app");
      },
    },
    { label: "Quick actions…", shortcut: "Ctrl+K", action: () => (editor.quickActions = true) },
    "-",
    { label: "Undo", shortcut: "Ctrl+Z", disabled: !editor.canUndo, action: () => editor.undo() },
    { label: "Redo", shortcut: "Ctrl+Y", disabled: !editor.canRedo, action: () => editor.redo() },
    "-",
    { label: "New sample document", action: () => editor.loadSample() },
    { label: "Zoom to fit", shortcut: "Shift+1", action: () => editor.fit() },
    "-",
    { label: "Benchmark: 10,000 shapes", action: () => editor.loadBenchmark(10_000) },
    { label: "Benchmark: 50,000 shapes", action: () => editor.loadBenchmark(50_000) },
    { label: "Benchmark: 100,000 shapes", action: () => editor.loadBenchmark(100_000) },
    { label: "Run benchmark", disabled: editor.benchmarking, action: () => editor.runBenchmark() },
    { label: "Show performance stats", checked: editor.showStats, action: () => (editor.showStats = !editor.showStats) },
    "-",
    {
      label: editor.mcp?.running ? `MCP server: ${editor.mcp.url}` : `MCP server: ${editor.mcp?.error ?? "desktop app only"}`,
      disabled: true,
      action: () => {},
    },
    {
      label: "Copy Claude Code setup command",
      disabled: !editor.mcp?.running,
      action: () => {
        navigator.clipboard.writeText(`claude mcp add --transport http pergel ${editor.mcp?.url}`);
        editor.notify("Copied — paste it in a terminal");
      },
    },
  ];

  function pressRow(e: PointerEvent, layer: LayerInfo, index: number) {
    if (e.button !== 0 || editor.renaming >= 0) return;
    const engine = editor.engine;
    if (!engine) return;
    if (e.shiftKey && anchor >= 0) {
      const [a, b] = anchor < index ? [anchor, index] : [index, anchor];
      engine.select(-1);
      for (const r of rows.slice(a, b + 1)) engine.select(r.id, true);
      editor.refreshSelection();
      editor.invalidate();
    } else {
      anchor = index;
      if (e.ctrlKey || e.metaKey) editor.select(layer.id, true);
      else if (!selected.has(layer.id)) editor.select(layer.id);
    }
    drag = { id: layer.id, y: e.clientY, active: false, target: -1, place: "above" };
  }

  function dragMove(e: PointerEvent) {
    if (!drag || !list) return;
    if (!drag.active && Math.abs(e.clientY - drag.y) < 4) return;
    drag.active = true;
    const r = list.getBoundingClientRect();
    const y = e.clientY - r.top + list.scrollTop;
    const i = Math.min(rows.length - 1, Math.max(0, Math.floor(y / ROW)));
    const row = rows[i];
    if (!row) return;
    const t = (y - i * ROW) / ROW;
    const container = row.kind === "Frame" || row.kind === "Section" || row.kind === "Group";
    let place: Place = container ? (t < 0.3 ? "above" : t > 0.7 ? "below" : "inside") : t < 0.5 ? "above" : "below";
    // Just below an expanded container means "first child", i.e. on top inside it.
    if (place === "below" && container && row.children > 0 && editor.expanded.has(row.id)) place = "inside";
    drag.target = row.id;
    drag.place = place;
  }

  function dragEnd() {
    const d = drag;
    drag = null;
    if (!d?.active || d.target < 0 || selected.has(d.target)) return;
    if (d.place === "inside") editor.expanded = new Set(editor.expanded).add(d.target);
    editor.run((engine) => {
      if (!engine.moveTo(d.target, d.place)) editor.notify("A layer can't be moved inside itself");
    });
  }

  // Assets: click a component to place an instance in view, or drag it onto the canvas.
  let assetDrag = $state<{ id: number; x: number; y: number; moved: boolean } | null>(null);

  $effect(() => {
    if (editor.leftTab === "assets") {
      // Re-list whenever the document changes while the tab is open.
      void editor.layers;
      editor.refreshComponents();
    }
  });

  function assetMove(e: PointerEvent) {
    if (assetDrag && Math.hypot(e.clientX - assetDrag.x, e.clientY - assetDrag.y) > 4) assetDrag.moved = true;
  }

  function assetUp(e: PointerEvent) {
    const d = assetDrag;
    assetDrag = null;
    if (!d) return;
    if (!d.moved) return editor.insertInstance(d.id);
    const canvas = document.getElementById("pergel-canvas");
    const r = canvas?.getBoundingClientRect();
    if (r && e.clientX >= r.left && e.clientX <= r.right && e.clientY >= r.top && e.clientY <= r.bottom) {
      editor.insertInstance(d.id, e.clientX - r.left, e.clientY - r.top);
    }
  }

  function hover(id: number) {
    if (drag?.active) return;
    editor.engine?.setHover(id);
    editor.invalidate();
  }

  function finishRename(id: number, value: string) {
    editor.renaming = -1;
    if (value.trim()) editor.rename(id, value);
  }
  let toolsTab = $state<"icons" | "plugins">("icons");
</script>

<svelte:window
  onpointermove={(e) => {
    dragMove(e);
    assetMove(e);
  }}
  onpointerup={(e) => {
    dragEnd();
    assetUp(e);
  }}
/>

{#snippet layerIcon(kind: string, component: string | null = null)}
  {#if component === "main"}<ComponentIcon size={14} strokeWidth={1.7} />
  {:else if component === "instance"}<Diamond size={14} strokeWidth={1.7} />
  {:else if kind === "Frame"}<Hash size={14} strokeWidth={1.7} />
  {:else if kind === "Section"}<PanelTop size={14} strokeWidth={1.7} />
  {:else if kind === "Slice"}<SliceIcon size={14} strokeWidth={1.7} />
  {:else if kind === "Group"}<SquareDashed size={14} strokeWidth={1.7} />
  {:else if kind === "Ellipse"}<Circle size={14} strokeWidth={1.7} />
  {:else if kind === "Text"}<Type size={14} strokeWidth={1.7} />
  {:else if kind === "Vector"}<PenTool size={14} strokeWidth={1.7} />
  {:else if kind === "Image"}<ImageIcon size={14} strokeWidth={1.7} />
  {:else if kind === "Mask"}<MaskIcon size={14} strokeWidth={1.7} />
  {:else}<Square size={14} strokeWidth={1.7} />{/if}
{/snippet}

<aside class="left">
  <header>
    <div class="top">
      <div class="menu-anchor">
        <button class="logo" data-menu-trigger onclick={() => (menuOpen = !menuOpen)} aria-label="Main menu">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
            <circle cx="12" cy="4.5" r="2" /><path d="M11 6.5 5.5 21M13 6.5 18.5 21M8 14.5h8" />
          </svg>
          <ChevronDown size={10} strokeWidth={2} />
        </button>
        <Menu items={mainMenu()} bind:open={menuOpen} />
      </div>
      <span class="spacer"></span>
      <button class="icon-btn" title="Hide UI (Ctrl+\)" onclick={() => (editor.uiHidden = true)}>
        <PanelLeftClose size={16} strokeWidth={1.6} />
      </button>
    </div>
    <div class="file">
      {#if renamingFile}
        <input
          class="rename"
          value={editor.fileName}
          onblur={(e) => {
            renamingFile = false;
            const name = (e.target as HTMLInputElement).value.trim() || "Untitled";
            editor.fileName = name;
            const file = workspace.current?.file;
            if (file && file.name !== name) void workspace.update(file.id, { name });
          }}
          onkeydown={(e) => {
            e.stopPropagation();
            if (e.key === "Enter" || e.key === "Escape") (e.target as HTMLInputElement).blur();
          }}
          {@attach (el) => (el as HTMLInputElement).select()}
        />
      {:else}
        <button class="file-name" ondblclick={() => (renamingFile = true)} title="Double-click to rename">
          {editor.fileName}
        </button>
      {/if}
      <span class="faint">Drafts</span>
    </div>
    <div class="tabs">
      <span class="tab-title">{({ file: "File", assets: "Assets", agents: "Agents", tools: "Tools" })[editor.leftTab]}</span>
      <span class="spacer"></span>
      <button class="icon-btn" class:on={layerSearch !== null} title="Search layers" onclick={() => (layerSearch = layerSearch === null ? "" : null)}>
        <Search size={14} strokeWidth={1.7} />
      </button>
    </div>
    {#if layerSearch !== null}
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="layer-search"
        placeholder="Find layers"
        bind:value={layerSearch}
        autofocus
        onkeydown={(e) => {
          e.stopPropagation();
          if (e.key === "Escape") layerSearch = null;
          if (e.key === "Enter" && matches[0]) editor.selectAndReveal(matches[0].id);
        }}
      />
    {/if}
  </header>
  {#if layerSearch !== null && layerSearch.trim()}
    <section class="results">
      <div class="section-head"><span class="section-title">{matches.length} result{matches.length === 1 ? "" : "s"}</span></div>
      {#each matches as m (m.id)}
        <button class="result" class:on={editor.selection.some((n) => n.id === m.id)} onclick={(e) => (e.shiftKey ? editor.select(m.id, true) : editor.selectAndReveal(m.id))}>
          <span class="kind">{@render layerIcon(m.kind, m.component)}</span>
          <span class="rname">{m.name}</span>
          <span class="faint path">{m.path}</span>
        </button>
      {/each}
    </section>
  {/if}

  {#if editor.leftTab === "file" && !(layerSearch !== null && layerSearch.trim())}
    <section class="pages">
      <div class="section-head">
        <span class="section-title">Pages</span>
        <button class="icon-btn" title="Add page" onclick={() => editor.addPage()}>
          <Plus size={14} strokeWidth={1.7} />
        </button>
      </div>
      <div class="page-list">
        {#each editor.pages as page, i (i)}
          {#if editor.renamingPage === i}
            <!-- svelte-ignore a11y_autofocus -->
            <input
              class="page page-input"
              value={page.name}
              autofocus
              onfocus={(e) => (e.target as HTMLInputElement).select()}
              onkeydown={(e) => {
                e.stopPropagation();
                if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                if (e.key === "Escape") editor.renamingPage = -1;
              }}
              onblur={(e) => editor.renamePage(i, (e.target as HTMLInputElement).value)}
            />
          {:else}
            <button
              class="page"
              class:active={page.current}
              draggable="true"
              ondragstart={(e) => e.dataTransfer?.setData("pergel/page", String(i))}
              ondragover={(e) => {
                if (e.dataTransfer?.types.includes("pergel/page")) e.preventDefault();
              }}
              ondrop={(e) => {
                const from = Number(e.dataTransfer?.getData("pergel/page"));
                if (Number.isFinite(from)) editor.movePage(from, i);
              }}
              onclick={() => editor.switchPage(i)}
              ondblclick={() => (editor.renamingPage = i)}
              oncontextmenu={(e) => {
                e.preventDefault();
                pageMenu = { x: e.clientX, y: e.clientY, index: i };
                pageMenuOpen = true;
              }}
            >
              <span class="check">{#if page.current}<Check size={12} strokeWidth={2} />{/if}</span>
              <span class="page-name">{page.name}</span>
            </button>
          {/if}
        {/each}
      </div>
    </section>

    <section class="layers">
      <div class="section-head">
        <span class="section-title">Layers</span>
        <button class="icon-btn" title="Collapse layers (Alt+L)" onclick={() => editor.collapseAll()}>
          <ChevronsDownUp size={14} strokeWidth={1.7} />
        </button>
      </div>
      <div
        class="list"
        class:dragging={drag?.active}
        role="tree"
        tabindex="-1"
        bind:this={list}
        bind:clientHeight={viewport}
        onscroll={(e) => (scrollTop = (e.target as HTMLElement).scrollTop)}
        onpointerleave={() => hover(-1)}
      >
        <div style="height:{rows.length * ROW}px; position:relative">
          {#each rows.slice(start, end) as layer, i (layer.id)}
            {@const index = start + i}
            {@const container = layer.children > 0}
            {@const open = editor.expanded.has(layer.id)}
            <div
              class="row"
              class:selected={selected.has(layer.id)}
              class:inside={!selected.has(layer.id) && layer.depth > 0 && insideSelection(layer)}
              class:frame={(layer.kind === "Frame" || layer.kind === "Section") && layer.depth === 0}
              class:hidden={!layer.visible}
              class:purple={layer.purple}
              class:drop-above={drag?.active && drag.target === layer.id && drag.place === "above"}
              class:drop-below={drag?.active && drag.target === layer.id && drag.place === "below"}
              class:drop-inside={drag?.active && drag.target === layer.id && drag.place === "inside"}
              role="treeitem"
              aria-selected={selected.has(layer.id)}
              aria-expanded={container ? open : undefined}
              tabindex="-1"
              style="top:{index * ROW}px; padding-left:{4 + layer.depth * INDENT}px"
              onpointerdown={(e) => pressRow(e, layer, index)}
              onkeydown={() => {}}
              ondblclick={() => (editor.renaming = layer.id)}
              oncontextmenu={(e) => {
                e.preventDefault();
                if (!selected.has(layer.id)) editor.select(layer.id);
                rowMenu = { x: e.clientX, y: e.clientY };
                rowMenuOpen = true;
              }}
              onpointerenter={() => hover(layer.id)}
            >
              <button
                class="twisty"
                class:none={!container}
                tabindex="-1"
                aria-label={open ? "Collapse" : "Expand"}
                onpointerdown={(e) => e.stopPropagation()}
                onclick={(e) => container && editor.toggleExpanded(layer.id, e.altKey)}
              >
                {#if container}
                  {#if open}<ChevronDown size={12} strokeWidth={2} />{:else}<ChevronRight size={12} strokeWidth={2} />{/if}
                {/if}
              </button>
              <span class="kind">{@render layerIcon(layer.kind, layer.component)}</span>
              {#if thumbs.get(layer.id)}
                <img class="layer-thumb" src={thumbs.get(layer.id)} alt="" draggable="false" />
              {/if}
              {#if editor.renaming === layer.id}
                <input
                  class="rename"
                  value={layer.name}
                  onpointerdown={(e) => e.stopPropagation()}
                  onblur={(e) => finishRename(layer.id, (e.target as HTMLInputElement).value)}
                  onkeydown={(e) => {
                    e.stopPropagation();
                    if (e.key === "Enter") (e.target as HTMLInputElement).blur();
                    if (e.key === "Escape") editor.renaming = -1;
                  }}
                  {@attach (el) => (el as HTMLInputElement).select()}
                />
              {:else}
                <span class="name">{layer.name}</span>
              {/if}
              <span class="toggles">
                <button
                  class="toggle pan"
                  title="Pan to layer"
                  onpointerdown={(e) => e.stopPropagation()}
                  onclick={() => editor.selectAndReveal(layer.id)}
                >
                  <Crosshair size={12} strokeWidth={1.8} />
                </button>
                <button
                  class="toggle"
                  class:on={layer.locked}
                  title={layer.locked ? "Unlock (Ctrl+Shift+L)" : "Lock (Ctrl+Shift+L)"}
                  onpointerdown={(e) => e.stopPropagation()}
                  onclick={() => editor.toggleFlag(layer.id, "locked")}
                >
                  {#if layer.locked}<Lock size={12} strokeWidth={1.8} />{:else}<LockOpen size={12} strokeWidth={1.8} />{/if}
                </button>
                <button
                  class="toggle"
                  class:on={!layer.visible}
                  title={layer.visible ? "Hide (Ctrl+Shift+H)" : "Show (Ctrl+Shift+H)"}
                  onpointerdown={(e) => e.stopPropagation()}
                  onclick={() => editor.toggleFlag(layer.id, "visible")}
                >
                  {#if layer.visible}<Eye size={12} strokeWidth={1.8} />{:else}<EyeOff size={12} strokeWidth={1.8} />{/if}
                </button>
              </span>
            </div>
          {/each}
        </div>
      </div>
    </section>
  {:else if editor.leftTab === "agents"}
    <AgentsPanel />
  {:else if editor.leftTab === "tools"}
    <div class="tool-tabs">
      <button class:on={toolsTab === "icons"} onclick={() => (toolsTab = "icons")}>Icons</button>
      <button class:on={toolsTab === "plugins"} onclick={() => (toolsTab = "plugins")}>Plugins</button>
    </div>
    {#if toolsTab === "icons"}<IconsPanel />{:else}<PluginsPanel />{/if}
  {:else if editor.leftTab === "assets"}
    <section class="assets">
      <div class="section-head"><span class="section-title">Local components</span></div>
      {#if editor.components.length}
        <div class="asset-grid">
          {#each editor.components as c (c.id)}
            <button
              class="asset"
              class:dragging={assetDrag?.id === c.id && assetDrag.moved}
              title="{c.name} · {c.instances} instance{c.instances === 1 ? '' : 's'} — click to insert, or drag onto the canvas"
              onpointerdown={(e) => {
                if (e.button === 0) assetDrag = { id: c.id, x: e.clientX, y: e.clientY, moved: false };
              }}
              ondblclick={() => {
                editor.select(c.id);
                editor.fitSelection();
              }}
            >
              <span class="thumb">
                {#if c.thumbnail}<img src={c.thumbnail} alt="" draggable="false" />{:else}<ComponentIcon size={20} />{/if}
              </span>
              <span class="asset-name">{c.name}</span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="muted pad">No components yet.</p>
        <p class="faint pad">Select layers and press Ctrl+Alt+K to create one.</p>
      {/if}
      <div class="section-head">
        <span class="section-title">Team library</span>
        <button class="icon-btn" title="Publish this file's components as a library" onclick={() => libraries.publish()}><Upload size={14} strokeWidth={1.7} /></button>
      </div>
      {#if libraries.updates.length}
        <div class="lib-update">
          <span>{libraries.updates.length} library update{libraries.updates.length === 1 ? "" : "s"}</span>
          <button class="primary-sm" onclick={() => libraries.updateAll()}>Update</button>
        </div>
      {/if}
      {#each Object.values(libraries.all) as lib (lib.id)}
        <div class="lib-name">{lib.name}<span class="faint"> · {lib.components.length}</span></div>
        <div class="asset-grid">
          {#each lib.components as c (c.key)}
            <button class="asset" title="{c.name} from {lib.name} — click to insert" onclick={() => libraries.use(lib.id, c.key)}>
              <span class="thumb">
                {#if c.thumbnail}<img src={c.thumbnail} alt="" draggable="false" />{:else}<ComponentIcon size={20} />{/if}
              </span>
              <span class="asset-name">{c.name}</span>
            </button>
          {/each}
        </div>
      {:else}
        <p class="faint pad">Publish a file's components (↑) to use them in every file.</p>
      {/each}
    </section>
  {/if}
  {#if pageMenu}
    <div class="row-menu" style="left:{pageMenu.x}px; top:{pageMenu.y}px">
      <Menu items={pageMenuOpen ? pageItems(pageMenu.index) : []} bind:open={pageMenuOpen} />
    </div>
  {/if}
  {#if rowMenu}
    <div class="row-menu" style="left:{rowMenu.x}px; top:{rowMenu.y}px">
      <Menu items={rowMenuOpen ? layerMenu({ panel: true }) : []} bind:open={rowMenuOpen} />
    </div>
  {/if}
</aside>

<style>
  .lib-name {
    padding: 8px 16px 4px;
    font-weight: 600;
  }
  .lib-update {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin: 0 12px 8px;
    padding: 6px 8px;
    border-radius: var(--radius);
    background: rgba(13, 153, 255, 0.15);
  }
  .primary-sm {
    height: 24px;
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .tool-tabs {
    display: flex;
    gap: 4px;
    padding: 8px 12px 0;
  }
  .tool-tabs button {
    height: 26px;
    padding: 0 10px;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-weight: 550;
  }
  .tool-tabs button.on {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .tab-title {
    font-weight: 600;
    padding-left: 8px;
  }
  .layer-thumb {
    width: 20px;
    height: 20px;
    flex: none;
    object-fit: contain;
    margin-right: 6px;
    border-radius: 3px;
    background: repeating-conic-gradient(#3a3a3a 0 25%, #2f2f2f 0 50%) 0 0 / 6px 6px;
  }
  .toggle.pan {
    opacity: 0;
  }
  .row:hover .toggle.pan {
    opacity: 1;
  }
  .row-menu {
    position: fixed;
    width: 0;
    height: 0;
    z-index: 60;
  }
  .row-menu :global(.menu.below) {
    top: 0;
  }
  .left {
    display: flex;
    flex-direction: column;
    width: 240px;
    flex: none;
    background: var(--bg);
    border-right: 1px solid var(--border);
    min-height: 0;
  }
  header {
    padding: 8px 8px 0;
    border-bottom: 1px solid var(--border);
  }
  .top {
    display: flex;
    align-items: center;
  }
  .menu-anchor {
    position: relative;
  }
  .logo {
    display: inline-flex;
    align-items: center;
    gap: 2px;
    height: 32px;
    padding: 0 6px;
    border-radius: var(--radius);
    color: var(--text);
  }
  .logo:hover {
    background: var(--bg-secondary);
  }
  .spacer {
    flex: 1;
  }
  .file {
    display: flex;
    flex-direction: column;
    padding: 6px 8px 8px;
  }
  .file-name {
    text-align: left;
    font-size: 13px;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .rename {
    height: 22px;
    padding: 0 4px;
    margin: -3px 0 -3px -4px;
    background: var(--canvas);
    border: 1px solid var(--brand-strong);
    border-radius: 3px;
    outline: none;
    width: calc(100% + 4px);
    min-width: 0;
  }
  .tabs {
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 0 0 8px;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
  }
  .pages {
    border-bottom: 1px solid var(--border);
    padding-bottom: 8px;
  }
  .page {
    display: flex;
    align-items: center;
    width: calc(100% - 16px);
    height: 32px;
    margin: 0 8px;
    padding: 0 8px 0 24px;
    border-radius: var(--radius);
    text-align: left;
  }
  .page.active {
    font-weight: 550;
  }
  .page:hover {
    background: var(--bg-secondary);
  }
  .layer-search {
    margin: 0 8px 8px;
    width: calc(100% - 16px);
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid var(--brand-strong);
    color: var(--text);
    outline: none;
  }
  .results {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 72px;
  }
  .result {
    display: flex;
    align-items: center;
    gap: 6px;
    width: calc(100% - 16px);
    height: 32px;
    margin: 0 8px;
    padding: 0 8px;
    border-radius: var(--radius);
    text-align: left;
  }
  .result:hover {
    background: var(--bg-secondary);
  }
  .result.on {
    background: var(--selection-bg, rgba(13, 153, 255, 0.25));
  }
  .result .kind {
    display: flex;
    color: var(--text-secondary);
  }
  .rname {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .path {
    max-width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .page-list {
    max-height: 160px;
    overflow-y: auto;
  }
  .check {
    position: absolute;
    left: 8px;
    display: flex;
  }
  .page {
    position: relative;
  }
  .page-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .page-input {
    background: var(--bg);
    border: 1px solid var(--brand-strong);
    outline: none;
    color: var(--text);
  }
  .layers {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding-bottom: 72px;
  }
  .list.dragging {
    cursor: grabbing;
  }
  .row {
    position: absolute;
    left: 8px;
    right: 8px;
    height: 32px;
    display: flex;
    align-items: center;
    gap: 4px;
    padding-right: 4px;
    border-radius: var(--radius);
    white-space: nowrap;
  }
  .row:hover {
    outline: 1px solid var(--border);
    outline-offset: -1px;
  }
  .row.selected {
    background: var(--bg-selected);
    outline: none;
  }
  .row.inside {
    background: var(--bg-selected-secondary, rgba(74, 88, 120, 0.45));
    border-radius: 0;
  }
  .row.frame .name {
    font-weight: 600;
  }
  .row.hidden .name,
  .row.hidden .kind {
    opacity: 0.45;
  }
  .row.drop-above::before,
  .row.drop-below::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--brand-strong);
    pointer-events: none;
  }
  .row.drop-above::before {
    top: -1px;
  }
  .row.drop-below::after {
    bottom: -1px;
  }
  .row.drop-inside {
    outline: 2px solid var(--brand-strong);
    outline-offset: -2px;
  }
  .twisty {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    flex: none;
    border-radius: 3px;
    color: var(--text-secondary);
  }
  .twisty:hover:not(.none) {
    color: var(--text);
  }
  .twisty.none {
    pointer-events: none;
  }
  .kind {
    display: inline-flex;
    color: var(--text-secondary);
    flex: none;
    margin-right: 2px;
  }
  .row.selected .kind {
    color: var(--text);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .toggles {
    display: flex;
    flex: none;
  }
  .toggle {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: var(--radius);
    color: var(--text-secondary);
    visibility: hidden;
  }
  .toggle:hover {
    color: var(--text);
    background: var(--bg-secondary);
  }
  .toggle.on,
  .row:hover .toggle {
    visibility: visible;
  }
  .assets {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .assets p {
    margin: 0 0 6px;
  }
  .pad {
    padding: 0 16px;
  }
  .asset-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    padding: 0 12px 16px;
  }
  .asset {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 6px;
    border-radius: var(--radius);
    text-align: left;
    cursor: grab;
  }
  .asset:hover {
    background: var(--bg-secondary);
  }
  .asset.dragging {
    opacity: 0.5;
    cursor: grabbing;
  }
  .thumb {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 72px;
    border-radius: 4px;
    background: var(--canvas);
    color: var(--component, #9747ff);
    overflow: hidden;
  }
  .thumb img {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .asset-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .row.purple .name,
  .row.purple .kind {
    color: var(--component, #9747ff);
  }
  .row.purple.selected .name,
  .row.purple.selected .kind {
    color: #c7a6ff;
  }
</style>
