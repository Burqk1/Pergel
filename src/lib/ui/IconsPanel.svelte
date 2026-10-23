<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import { editor } from "../editor.svelte";
  import { fetchIcon, searchIcons } from "../mcp";

  // Tools → Icons: search 200,000+ open-source icons (Iconify) and insert them as real vectors.

  let query = $state("");
  let set = $state("");
  let icons = $state<string[]>([]);
  let busy = $state(false);
  let error = $state("");
  const SETS = ["", "lucide", "mdi", "tabler", "heroicons", "ph", "material-symbols", "logos", "skill-icons", "simple-icons"];
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function run() {
    const q = query.trim();
    if (!q) {
      icons = [];
      return;
    }
    busy = true;
    error = "";
    const r = await searchIcons({ query: q, prefixes: set, limit: 64 });
    busy = false;
    if (r.ok) icons = (r.result as { icons: string[] }).icons;
    else error = r.error ?? "Search failed";
  }

  function schedule() {
    clearTimeout(timer);
    timer = setTimeout(run, 300);
  }

  async function insert(id: string) {
    try {
      const svg = await fetchIcon(id);
      editor.importSvg(id.split(":").pop() ?? id, svg);
    } catch (e) {
      editor.notify(String(e instanceof Error ? e.message : e));
    }
  }

  const url = (id: string) => {
    const [p, n] = id.split(":");
    return `https://api.iconify.design/${p}/${n}.svg?color=%23e6e6e6`;
  };
</script>

<section class="icons">
  <div class="section-head"><span class="section-title">Icons</span></div>
  <div class="search">
    <Search size={14} strokeWidth={1.8} />
    <!-- svelte-ignore a11y_autofocus -->
    <input placeholder="Search icons (e.g. arrow, cart, github)" bind:value={query} autofocus oninput={schedule} onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") run(); }} />
  </div>
  <select bind:value={set} onchange={run}>
    {#each SETS as s (s)}<option value={s}>{s || "All icon sets"}</option>{/each}
  </select>
  {#if error}<p class="error">{error}</p>{/if}
  <div class="grid">
    {#each icons as id (id)}
      <button class="icon" title="{id} — click to insert" onclick={() => insert(id)}>
        <img src={url(id)} alt={id} loading="lazy" />
      </button>
    {:else}
      <p class="faint pad">{busy ? "Searching…" : query ? "No icons found" : "Icons insert as editable vector layers. MCP agents use the same library (search_icons)."}</p>
    {/each}
  </div>
</section>

<style>
  .icons {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .section-head {
    display: flex;
    align-items: center;
    height: 40px;
    padding: 0 16px;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0 12px 6px;
    padding: 0 8px;
    height: 30px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text-secondary);
  }
  .search input {
    flex: 1;
    min-width: 0;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
  }
  select {
    margin: 0 12px 8px;
    height: 26px;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    color: var(--text);
  }
  .grid {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(44px, 1fr));
    gap: 4px;
    align-content: start;
    padding: 0 12px 72px;
  }
  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 44px;
    border-radius: var(--radius);
  }
  .icon:hover {
    background: var(--bg-secondary);
  }
  .icon img {
    width: 24px;
    height: 24px;
  }
  .pad {
    grid-column: 1 / -1;
    padding: 4px;
    line-height: 1.5;
  }
  .error {
    margin: 0 12px;
    color: #f87171;
  }
</style>
