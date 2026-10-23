<script lang="ts" module>
  export type MenuItem =
    | {
        label: string;
        shortcut?: string;
        checked?: boolean;
        disabled?: boolean;
        /** Opens a submenu on hover instead of acting. */
        children?: MenuItem[];
        /** Section title row (not clickable). */
        heading?: boolean;
        action?: () => void;
      }
    | "-";
</script>

<script lang="ts">
  import ChevronRight from "@lucide/svelte/icons/chevron-right";

  let {
    items,
    open = $bindable(false),
    align = "left",
    placement = "below",
    onclose,
  }: {
    items: MenuItem[];
    open?: boolean;
    align?: "left" | "right" | "center";
    placement?: "below" | "above";
    onclose?: () => void;
  } = $props();

  /** Open submenus, one per depth: which item opened it and where it sits on screen. */
  let subs = $state<{ index: number; x: number; y: number; flip: boolean }[]>([]);
  let el = $state<HTMLDivElement>();

  function dismiss() {
    open = false;
    subs = [];
    onclose?.();
  }

  $effect(() => {
    if (!open) {
      subs = [];
      return;
    }
    const close = (e: PointerEvent) => {
      if (el && !el.contains(e.target as Node) && !(e.target as HTMLElement).closest("[data-menu-trigger]")) {
        dismiss();
      }
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && dismiss();
    window.addEventListener("pointerdown", close, true);
    window.addEventListener("keydown", esc);
    return () => {
      window.removeEventListener("pointerdown", close, true);
      window.removeEventListener("keydown", esc);
    };
  });

  function hover(e: PointerEvent, depth: number, index: number, item: MenuItem) {
    if (item === "-") return;
    const next = subs.slice(0, depth);
    if (item.children?.length && !item.disabled) {
      const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
      const flip = r.right + 220 > window.innerWidth;
      next.push({ index, x: flip ? r.left - 4 : r.right + 4, y: r.top - 8, flip });
    }
    subs = next;
  }

  function childrenAt(depth: number): MenuItem[] {
    let list = items;
    for (let d = 0; d <= depth; d++) {
      const item = list[subs[d]?.index ?? -1];
      if (!item || item === "-") return [];
      list = item.children ?? [];
    }
    return list;
  }
</script>

{#snippet rows(list: MenuItem[], depth: number)}
  {#each list as item, i (i)}
    {#if item === "-"}
      <div class="sep"></div>
    {:else if item.heading}
      <div class="heading">{item.label}</div>
    {:else}
      <button
        role="menuitem"
        disabled={item.disabled}
        class:open={subs[depth]?.index === i}
        onpointerenter={(e) => hover(e, depth, i, item)}
        onclick={(e) => {
          if (item.children) {
            hover(e as unknown as PointerEvent, depth, i, item);
            return;
          }
          dismiss();
          item.action?.();
        }}
      >
        <span class="check">{item.checked ? "✓" : ""}</span>
        <span class="label">{item.label}</span>
        {#if item.shortcut}<span class="shortcut">{item.shortcut}</span>{/if}
        {#if item.children}<span class="arrow"><ChevronRight size={12} strokeWidth={2} /></span>{/if}
      </button>
    {/if}
  {/each}
{/snippet}

{#if open}
  <div class="menu {align} {placement}" role="menu" bind:this={el}>
    {@render rows(items, 0)}
    {#each subs as sub, depth (depth)}
      <div
        class="menu sub"
        role="menu"
        style="left:{sub.x}px; top:{sub.y}px; {sub.flip ? 'transform: translateX(-100%)' : ''}"
      >
        {@render rows(childrenAt(depth), depth + 1)}
      </div>
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: absolute;
    z-index: 50;
    min-width: 200px;
    padding: 8px 0;
    background: var(--menu-bg);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
    font-size: 12px;
    max-height: min(640px, 80vh);
    overflow-y: auto;
  }
  .menu.sub {
    position: fixed;
    z-index: 51;
  }
  .below {
    top: calc(100% + 6px);
  }
  .above {
    bottom: calc(100% + 8px);
  }
  .left {
    left: 0;
  }
  .right {
    right: 0;
  }
  .center {
    left: 50%;
    transform: translateX(-50%);
  }
  button {
    display: flex;
    align-items: center;
    width: calc(100% - 16px);
    margin: 0 8px;
    height: 24px;
    padding: 0 8px 0 0;
    border-radius: var(--radius);
    color: var(--text);
    text-align: left;
    white-space: nowrap;
  }
  button:hover:not(:disabled),
  button.open {
    background: var(--brand);
  }
  button:disabled {
    color: var(--text-tertiary);
  }
  .check {
    width: 24px;
    text-align: center;
    flex: none;
  }
  .label {
    flex: 1;
  }
  .shortcut {
    margin-left: 24px;
    color: var(--text-secondary);
  }
  button:hover:not(:disabled) .shortcut,
  button.open .shortcut {
    color: var(--text);
  }
  .arrow {
    display: inline-flex;
    margin-left: 12px;
    color: var(--text-secondary);
  }
  .heading {
    height: 24px;
    line-height: 24px;
    padding: 0 16px 0 32px;
    color: var(--text-tertiary);
    white-space: nowrap;
  }
  .sep {
    height: 1px;
    margin: 8px 0;
    background: var(--menu-border);
  }
</style>
