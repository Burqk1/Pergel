<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import Clock from "@lucide/svelte/icons/clock";
  import Folder from "@lucide/svelte/icons/folder";
  import Star from "@lucide/svelte/icons/star";
  import Trash from "@lucide/svelte/icons/trash";
  import Search from "@lucide/svelte/icons/search";
  import Plus from "@lucide/svelte/icons/plus";
  import Upload from "@lucide/svelte/icons/upload";
  import Figma from "@lucide/svelte/icons/file-input";
  import Grid from "@lucide/svelte/icons/layout-grid";
  import List from "@lucide/svelte/icons/list";
  import PenTool from "@lucide/svelte/icons/pen-tool";
  import Ellipsis from "@lucide/svelte/icons/ellipsis";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { workspace, type HomeView } from "../workspace.svelte";
  import { editor } from "../editor.svelte";
  import { ago, exportToDisk, importFromDisk, type FileMeta } from "../files";

  let query = $state("");
  let layout = $state<"grid" | "list">(readLayout());
  let sort = $state<"opened" | "modified" | "name">("opened");
  let renaming = $state<string | null>(null);
  let menu = $state<{ file: FileMeta; x: number; y: number } | null>(null);
  let menuOpen = $state(false);
  let sortOpen = $state(false);

  function readLayout(): "grid" | "list" {
    try {
      return localStorage.getItem("pergel.home.layout") === "list" ? "list" : "grid";
    } catch {
      return "grid";
    }
  }

  $effect(() => {
    try {
      localStorage.setItem("pergel.home.layout", layout);
    } catch {
      // ignore
    }
  });

  const views: { id: HomeView; label: string; icon: typeof Clock }[] = [
    { id: "recents", label: "Recents", icon: Clock },
    { id: "drafts", label: "Drafts", icon: Folder },
    { id: "starred", label: "Starred", icon: Star },
    { id: "trash", label: "Trash", icon: Trash },
  ];
  const title = $derived(views.find((v) => v.id === workspace.homeView)?.label ?? "Recents");

  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const view = workspace.homeView;
    let list = workspace.files.filter((f) => (view === "trash" ? f.trashed : !f.trashed));
    if (view === "starred") list = list.filter((f) => f.starred);
    if (q) list = list.filter((f) => f.name.toLowerCase().includes(q));
    const key = view === "recents" ? "opened" : sort;
    return [...list].sort((a, b) => (key === "name" ? a.name.localeCompare(b.name) : b[key] - a[key]));
  });

  // Thumbnails load lazily, once per file version.
  const asked = new Map<string, number>();
  $effect(() => {
    for (const f of shown) {
      if (asked.get(f.id) !== f.modified) {
        asked.set(f.id, f.modified);
        void workspace.loadThumbnail(f.id);
      }
    }
  });

  async function importFile() {
    if (!isTauri()) return editor.notify("Importing files needs the desktop app");
    try {
      const meta = await importFromDisk();
      if (meta) {
        await workspace.refresh();
        await workspace.open(meta);
      }
    } catch (e) {
      editor.notify(`Import failed: ${e}`);
    }
  }

  function cardMenu(file: FileMeta): MenuItem[] {
    if (file.trashed) {
      return [
        { label: "Restore", action: () => workspace.trash(file.id, false) },
        "-",
        { label: "Delete forever", action: () => workspace.deleteForever(file.id) },
      ];
    }
    return [
      { label: "Open", action: () => workspace.open(file) },
      "-",
      { label: file.starred ? "Remove from Starred" : "Add to Starred", action: () => workspace.update(file.id, { starred: !file.starred }) },
      { label: "Duplicate", action: () => workspace.duplicate(file.id) },
      { label: "Rename", action: () => (renaming = file.id) },
      { label: "Export as .pergel…", disabled: !isTauri(), action: () => exportToDisk(file) },
      "-",
      { label: "Move to trash", action: () => workspace.trash(file.id) },
    ];
  }

  function showMenu(e: MouseEvent, file: FileMeta) {
    e.preventDefault();
    e.stopPropagation();
    menu = { file, x: e.clientX, y: e.clientY };
    menuOpen = true;
  }

  function finishRename(file: FileMeta, value: string) {
    renaming = null;
    if (value.trim() && value.trim() !== file.name) void workspace.update(file.id, { name: value.trim() });
  }

  const sortItems = (): MenuItem[] => [
    { label: "Last viewed", checked: sort === "opened", action: () => (sort = "opened") },
    { label: "Last modified", checked: sort === "modified", action: () => (sort = "modified") },
    { label: "Alphabetical", checked: sort === "name", action: () => (sort = "name") },
  ];
</script>

{#snippet nameField(file: FileMeta)}
  {#if renaming === file.id}
    <input
      class="rename"
      value={file.name}
      onclick={(e) => e.stopPropagation()}
      onblur={(e) => finishRename(file, (e.target as HTMLInputElement).value)}
      onkeydown={(e) => {
        e.stopPropagation();
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") renaming = null;
      }}
      {@attach (el) => (el as HTMLInputElement).select()}
    />
  {:else}
    <span class="file-name">{file.name}</span>
  {/if}
{/snippet}

<div class="home">
  <aside class="side">
    <div class="brand">
      <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <circle cx="12" cy="4.5" r="2" /><path d="M11 6.5 5.5 21M13 6.5 18.5 21M8 14.5h8" />
      </svg>
      <span>Pergel</span>
    </div>
    <label class="search">
      <Search size={14} strokeWidth={1.7} />
      <input placeholder="Search" bind:value={query} onkeydown={(e) => e.stopPropagation()} />
    </label>
    <nav>
      {#each views as v (v.id)}
        <button class:active={workspace.homeView === v.id} onclick={() => (workspace.homeView = v.id)}>
          <v.icon size={15} strokeWidth={1.7} />
          {v.label}
          {#if v.id === "trash"}
            {@const n = workspace.files.filter((f) => f.trashed).length}
            {#if n}<span class="count">{n}</span>{/if}
          {/if}
        </button>
      {/each}
    </nav>
    {#if workspace.error}<p class="error">{workspace.error}</p>{/if}
  </aside>

  <section class="main">
    <header>
      <h1>{title}</h1>
      <span class="spacer"></span>
      <button class="primary" onclick={() => workspace.newFile()}><Plus size={14} strokeWidth={2} />New design file</button>
      <button class="secondary" onclick={importFile}><Upload size={14} strokeWidth={1.8} />Import</button>
      <button class="secondary" onclick={() => (editor.figmaImport = true)}><Figma size={14} strokeWidth={1.8} />Import from Figma</button>
    </header>
    <div class="bar">
      <span class="muted">{shown.length} file{shown.length === 1 ? "" : "s"}</span>
      <span class="spacer"></span>
      {#if workspace.homeView !== "recents"}
        <div class="anchor">
          <button class="ghost" data-menu-trigger onclick={() => (sortOpen = !sortOpen)}>
            {sort === "opened" ? "Last viewed" : sort === "modified" ? "Last modified" : "Alphabetical"}
          </button>
          <Menu items={sortOpen ? sortItems() : []} bind:open={sortOpen} align="right" />
        </div>
      {/if}
      <div class="seg">
        <button class="icon-btn" class:on={layout === "grid"} title="Show as grid" onclick={() => (layout = "grid")}><Grid size={14} strokeWidth={1.7} /></button>
        <button class="icon-btn" class:on={layout === "list"} title="Show as list" onclick={() => (layout = "list")}><List size={14} strokeWidth={1.7} /></button>
      </div>
    </div>

    {#if !shown.length}
      <div class="empty">
        {#if workspace.homeView === "trash"}
          <p>Trash is empty</p>
        {:else if query}
          <p>No files match “{query}”</p>
        {:else}
          <p>No files yet</p>
          <button class="primary" onclick={() => workspace.newFile()}><Plus size={14} strokeWidth={2} />New design file</button>
        {/if}
      </div>
    {:else if layout === "grid"}
      <div class="grid">
        {#each shown as file (file.id)}
          <div
            class="card"
            role="button"
            tabindex="0"
            onclick={() => !file.trashed && renaming !== file.id && workspace.open(file)}
            onkeydown={(e) => e.key === "Enter" && !file.trashed && workspace.open(file)}
            oncontextmenu={(e) => showMenu(e, file)}
          >
            <div class="thumb">
              {#if workspace.thumbnails[file.id]}<img src={workspace.thumbnails[file.id]} alt="" draggable="false" />{/if}
              {#if !file.trashed}
                <button
                  class="star"
                  class:on={file.starred}
                  title={file.starred ? "Remove from Starred" : "Add to Starred"}
                  onclick={(e) => {
                    e.stopPropagation();
                    workspace.update(file.id, { starred: !file.starred });
                  }}
                >
                  <Star size={14} strokeWidth={1.8} fill={file.starred ? "currentColor" : "none"} />
                </button>
              {/if}
            </div>
            <div class="meta">
              <span class="type"><PenTool size={13} strokeWidth={1.8} /></span>
              <div class="text">
                {@render nameField(file)}
                <span class="faint">Edited {ago(file.modified)}</span>
              </div>
              <button class="icon-btn more" title="More" onclick={(e) => showMenu(e, file)}><Ellipsis size={14} strokeWidth={1.8} /></button>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <div class="list">
        <div class="list-head faint"><span>Name</span><span>Last viewed</span><span>Edited</span><span>Created</span></div>
        {#each shown as file (file.id)}
          <div
            class="row"
            role="button"
            tabindex="0"
            onclick={() => !file.trashed && renaming !== file.id && workspace.open(file)}
            onkeydown={(e) => e.key === "Enter" && !file.trashed && workspace.open(file)}
            oncontextmenu={(e) => showMenu(e, file)}
          >
            <span class="cell-name">
              <span class="mini">{#if workspace.thumbnails[file.id]}<img src={workspace.thumbnails[file.id]} alt="" />{/if}</span>
              {@render nameField(file)}
              {#if file.starred}<Star size={12} strokeWidth={1.8} fill="currentColor" />{/if}
            </span>
            <span class="faint">{ago(file.opened)}</span>
            <span class="faint">{ago(file.modified)}</span>
            <span class="faint">{new Date(file.created).toLocaleDateString()}</span>
          </div>
        {/each}
      </div>
    {/if}
  </section>

  {#if menu}
    <div class="menu-anchor" style="left:{menu.x}px; top:{menu.y}px">
      <Menu items={menuOpen ? cardMenu(menu.file) : []} bind:open={menuOpen} />
    </div>
  {/if}
</div>

<style>
  .home {
    position: absolute;
    inset: 0;
    z-index: 30;
    display: flex;
    background: var(--bg);
  }
  .side {
    width: 240px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 8px;
    border-right: 1px solid var(--border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    font-size: 13px;
    font-weight: 600;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: none;
    border: 0;
    outline: none;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  nav button {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius);
    text-align: left;
    color: var(--text);
  }
  nav button:hover {
    background: var(--bg-secondary);
  }
  nav button.active {
    background: var(--bg-selected);
  }
  .count {
    margin-left: auto;
    color: var(--text-secondary);
  }
  .error {
    color: #ff8a73;
    padding: 0 10px;
  }
  .main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow-y: auto;
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 56px;
    padding: 0 24px;
    border-bottom: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .primary,
  .secondary {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 12px;
    border-radius: var(--radius);
    font-weight: 550;
  }
  .primary {
    background: var(--brand);
    color: #fff;
  }
  .primary:hover {
    background: var(--brand-strong);
  }
  .secondary {
    border: 1px solid var(--border);
  }
  .secondary:hover {
    background: var(--bg-secondary);
  }
  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px 24px 8px;
  }
  .anchor {
    position: relative;
  }
  .ghost {
    height: 28px;
    padding: 0 8px;
    border-radius: var(--radius);
    color: var(--text-secondary);
  }
  .ghost:hover {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .seg {
    display: flex;
    background: var(--bg-secondary);
    border-radius: var(--radius);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: 20px;
    padding: 8px 24px 32px;
  }
  .card {
    display: flex;
    flex-direction: column;
    border: 1px solid var(--border);
    border-radius: 8px;
    overflow: hidden;
    cursor: default;
    background: var(--bg);
  }
  .card:hover {
    border-color: var(--text-tertiary);
  }
  .card:focus-visible {
    outline: 2px solid var(--brand-strong);
  }
  .thumb {
    position: relative;
    aspect-ratio: 16 / 10;
    background: var(--canvas);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }
  .thumb img {
    max-width: 88%;
    max-height: 84%;
    object-fit: contain;
  }
  .star {
    position: absolute;
    top: 8px;
    right: 8px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: 6px;
    background: rgba(30, 30, 30, 0.85);
    color: var(--text);
    visibility: hidden;
  }
  .star.on {
    color: #ffc700;
    visibility: visible;
  }
  .card:hover .star {
    visibility: visible;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 8px 10px 12px;
    border-top: 1px solid var(--border);
  }
  .type {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    border-radius: 6px;
    background: var(--brand);
    color: #fff;
    flex: none;
  }
  .text {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
  }
  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 550;
  }
  .rename {
    height: 22px;
    padding: 0 4px;
    margin-left: -4px;
    background: var(--canvas);
    border: 1px solid var(--brand-strong);
    border-radius: 3px;
    outline: none;
  }
  .more {
    visibility: hidden;
  }
  .card:hover .more {
    visibility: visible;
  }
  .list {
    display: flex;
    flex-direction: column;
    padding: 0 24px 32px;
  }
  .list-head,
  .row {
    display: grid;
    grid-template-columns: minmax(0, 3fr) 1fr 1fr 1fr;
    align-items: center;
    gap: 12px;
    height: 44px;
    padding: 0 8px;
  }
  .list-head {
    height: 32px;
    border-bottom: 1px solid var(--border);
  }
  .row {
    border-radius: var(--radius);
    cursor: default;
  }
  .row:hover {
    background: var(--bg-secondary);
  }
  .cell-name {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }
  .mini {
    width: 40px;
    height: 28px;
    flex: none;
    border-radius: 4px;
    background: var(--canvas);
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .mini img {
    max-width: 100%;
    max-height: 100%;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
    padding: 80px 24px;
    color: var(--text-secondary);
  }
  .menu-anchor {
    position: fixed;
    width: 0;
    height: 0;
    z-index: 60;
  }
  .menu-anchor :global(.menu.below) {
    top: 0;
  }
</style>
