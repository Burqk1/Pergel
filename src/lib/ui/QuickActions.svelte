<script lang="ts">
  import Search from "@lucide/svelte/icons/search";
  import { COMMANDS, type Command } from "../commands";
  import { editor } from "../editor.svelte";

  // Figma's Quick actions (Ctrl+K): search commands and layers, Enter runs the highlighted one.

  let query = $state("");
  let index = $state(0);

  type Item = { label: string; group: string; shortcut?: string; run: () => void };

  const items = $derived.by((): Item[] => {
    const q = query.trim().toLowerCase();
    const commands = COMMANDS.filter((c: Command) => (c.enabled?.() ?? true) && (!q || c.label.toLowerCase().includes(q) || c.group.toLowerCase().includes(q)));
    const layers: Item[] = q
      ? editor.layers
          .filter((l) => l.name.toLowerCase().includes(q))
          .slice(0, 8)
          .map((l) => ({ label: l.name, group: "Layers", run: () => editor.selectAndReveal(l.id) }))
      : [];
    return [...commands, ...layers].slice(0, 60);
  });

  $effect(() => {
    void query;
    index = 0;
  });

  function run(item: Item | undefined) {
    if (!item) return;
    editor.quickActions = false;
    item.run();
  }

  function key(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") editor.quickActions = false;
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      index = Math.min(items.length - 1, index + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      index = Math.max(0, index - 1);
    } else if (e.key === "Enter") run(items[index]);
  }
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && (editor.quickActions = false)}>
  <div class="palette" role="dialog" aria-label="Quick actions">
    <div class="search">
      <Search size={15} strokeWidth={1.8} />
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={query} placeholder="Search actions and layers" autofocus onkeydown={key} />
    </div>
    <div class="list" role="listbox">
      {#each items as item, i (item.group + item.label + i)}
        <button class="item" class:on={i === index} role="option" aria-selected={i === index} onpointerenter={() => (index = i)} onclick={() => run(item)}>
          <span class="label">{item.label}</span>
          <span class="group faint">{item.group}</span>
          {#if item.shortcut}<span class="shortcut faint">{item.shortcut}</span>{/if}
        </button>
      {:else}
        <p class="faint empty">No matching actions</p>
      {/each}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 180;
    display: flex;
    justify-content: center;
    padding-top: 12vh;
    background: rgba(0, 0, 0, 0.25);
  }
  .palette {
    width: 520px;
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
    overflow: hidden;
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 48px;
    padding: 0 14px;
    border-bottom: 1px solid var(--border);
    color: var(--text-secondary);
  }
  input {
    flex: 1;
    height: 100%;
    background: transparent;
    border: none;
    outline: none;
    color: var(--text);
    font-size: 14px;
  }
  .list {
    overflow-y: auto;
    padding: 6px;
  }
  .item {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    height: 32px;
    padding: 0 10px;
    border-radius: var(--radius);
    text-align: left;
  }
  .item.on {
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  .item.on .faint {
    color: rgba(255, 255, 255, 0.75);
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .shortcut {
    font-variant-numeric: tabular-nums;
  }
  .empty {
    padding: 12px;
  }
</style>
