<script lang="ts">
  import { isTauri } from "@tauri-apps/api/core";
  import House from "@lucide/svelte/icons/house";
  import Plus from "@lucide/svelte/icons/plus";
  import X from "@lucide/svelte/icons/x";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Minus from "@lucide/svelte/icons/minus";
  import Square from "@lucide/svelte/icons/square";
  import Copy from "@lucide/svelte/icons/copy";
  import PenTool from "@lucide/svelte/icons/pen-tool";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { workspace } from "../workspace.svelte";
  import { editor } from "../editor.svelte";

  const desktop = isTauri();
  let maximized = $state(false);
  let tabsMenu = $state(false);

  async function win() {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    return getCurrentWindow();
  }

  async function syncMaximized() {
    if (desktop) maximized = await (await win()).isMaximized();
  }

  $effect(() => {
    if (!desktop) return;
    void syncMaximized();
    const onResize = () => void syncMaximized();
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  });

  async function closeWindow() {
    await workspace.saveCurrent();
    (await win()).close();
  }

  function tabsItems(): MenuItem[] {
    const items: MenuItem[] = [
      { label: "New tab", shortcut: "Ctrl+T", action: () => workspace.newFile() },
      { label: "Open file browser", shortcut: "Ctrl+O", action: () => workspace.goHome() },
      "-",
      {
        label: "Close tab",
        shortcut: "Ctrl+W",
        disabled: workspace.active === "home",
        action: () => workspace.active !== "home" && workspace.closeTab(workspace.active),
      },
      { label: "Reopen closed tab", shortcut: "Ctrl+Shift+T", action: () => workspace.reopenClosed() },
    ];
    if (workspace.tabs.length) {
      items.push("-");
      for (const t of workspace.tabs) {
        items.push({ label: t.file.name, checked: workspace.active === t.slot, action: () => workspace.switchTo(t.slot) });
      }
    }
    return items;
  }

  function onKey(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (!mod) return;
    const key = e.key.toLowerCase();
    if (key === "t" && e.shiftKey) workspace.reopenClosed();
    else if (key === "t") workspace.newFile();
    else if (key === "w" && !e.shiftKey && workspace.active !== "home") workspace.closeTab(workspace.active);
    else if (key === "o" && !e.shiftKey) workspace.goHome();
    else if (key === "tab") workspace.cycle(e.shiftKey ? -1 : 1);
    else if (key === "s" && !e.shiftKey && !e.altKey) {
      void workspace.saveCurrent(true).then(() => editor.notify("Saved"));
    } else return;
    e.preventDefault();
    e.stopPropagation();
  }
</script>

<svelte:window onkeydowncapture={onKey} />

<header class="titlebar" data-tauri-drag-region>
  <button class="home" class:active={workspace.active === "home"} title="Home (Ctrl+O)" onclick={() => workspace.goHome()}>
    <House size={16} strokeWidth={1.6} />
  </button>
  <div class="tabs" data-tauri-drag-region>
    {#each workspace.tabs as tab (tab.slot)}
      {@const active = workspace.active === tab.slot}
      <div
        class="tab"
        class:active
        role="tab"
        tabindex="0"
        aria-selected={active}
        title={tab.file.name}
        onclick={() => workspace.switchTo(tab.slot)}
        onauxclick={(e) => e.button === 1 && workspace.closeTab(tab.slot)}
        onkeydown={(e) => e.key === "Enter" && workspace.switchTo(tab.slot)}
      >
        <PenTool size={13} strokeWidth={1.7} />
        <span class="name">{tab.file.name}</span>
        {#if active && workspace.unsaved}<span class="dot" title="Unsaved changes"></span>{/if}
        <button
          class="close"
          title="Close tab (Ctrl+W)"
          onclick={(e) => {
            e.stopPropagation();
            workspace.closeTab(tab.slot);
          }}
        >
          <X size={13} strokeWidth={1.8} />
        </button>
      </div>
    {/each}
    <button class="new" title="New design file (Ctrl+T)" onclick={() => workspace.newFile()}>
      <Plus size={15} strokeWidth={1.7} />
    </button>
  </div>
  <div class="right">
    <div class="anchor">
      <button class="tool" data-menu-trigger title="Open and recently closed tabs" onclick={() => (tabsMenu = !tabsMenu)}>
        <ChevronDown size={14} strokeWidth={1.8} />
      </button>
      <Menu items={tabsMenu ? tabsItems() : []} bind:open={tabsMenu} align="right" />
    </div>
    {#if desktop}
      <button class="win" title="Minimize" onclick={async () => (await win()).minimize()}>
        <Minus size={15} strokeWidth={1.5} />
      </button>
      <button class="win" title={maximized ? "Restore" : "Maximize"} onclick={async () => (await win()).toggleMaximize()}>
        {#if maximized}<Copy size={12} strokeWidth={1.6} />{:else}<Square size={12} strokeWidth={1.6} />{/if}
      </button>
      <button class="win close-win" title="Close" onclick={closeWindow}>
        <X size={16} strokeWidth={1.5} />
      </button>
    {/if}
  </div>
</header>

<style>
  .titlebar {
    display: flex;
    align-items: stretch;
    height: 40px;
    flex: none;
    background: var(--titlebar, #1f1f1f);
    border-bottom: 1px solid var(--border);
    user-select: none;
  }
  .home,
  .new,
  .tool,
  .win {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
  }
  .home {
    width: 48px;
  }
  .home:hover,
  .new:hover,
  .tool:hover {
    color: var(--text);
    background: var(--bg-secondary);
  }
  .home.active {
    color: var(--text);
    background: var(--bg);
  }
  .tabs {
    display: flex;
    flex: 1;
    min-width: 0;
    overflow: hidden;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 96px;
    max-width: 220px;
    flex: 0 1 220px;
    padding: 0 6px 0 14px;
    border-right: 1px solid var(--border);
    color: var(--text-secondary);
    cursor: default;
  }
  .tab:hover {
    background: #262626;
    color: var(--text);
  }
  .tab.active {
    background: var(--bg);
    color: var(--text);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--text-secondary);
    flex: none;
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 4px;
    color: var(--text-secondary);
    visibility: hidden;
    flex: none;
  }
  .tab:hover .close,
  .tab.active .close {
    visibility: visible;
  }
  .close:hover {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .new {
    width: 40px;
  }
  .right {
    display: flex;
    align-items: stretch;
  }
  .anchor {
    position: relative;
    display: flex;
  }
  .tool {
    width: 40px;
  }
  .win {
    width: 46px;
  }
  .win:hover {
    background: rgba(255, 255, 255, 0.08);
    color: var(--text);
  }
  .close-win:hover {
    background: #c42b1c;
    color: #fff;
  }
</style>
