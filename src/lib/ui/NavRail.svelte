<script lang="ts">
  import FileIcon from "@lucide/svelte/icons/file";
  import Bot from "@lucide/svelte/icons/bot";
  import Shapes from "@lucide/svelte/icons/shapes";
  import Wrench from "@lucide/svelte/icons/wrench";
  import Variable from "@lucide/svelte/icons/variable";
  import { editor } from "../editor.svelte";

  // Figma UI3's navigation rail: File, Agents, Assets, Tools, Variables.

  type Tab = "file" | "agents" | "assets" | "tools";
  const items: { tab: Tab | "variables"; label: string; icon: typeof FileIcon }[] = [
    { tab: "file", label: "File", icon: FileIcon },
    { tab: "agents", label: "Agents", icon: Bot },
    { tab: "assets", label: "Assets", icon: Shapes },
    { tab: "tools", label: "Tools", icon: Wrench },
    { tab: "variables", label: "Variables", icon: Variable },
  ];

  function open(tab: Tab | "variables") {
    if (tab === "variables") {
      editor.variablesOpen = true;
      return;
    }
    editor.leftTab = tab;
    if (tab === "assets") editor.refreshComponents();
  }
</script>

<nav class="rail" aria-label="Navigation">
  {#each items as it (it.tab)}
    {@const Icon = it.icon}
    <button class="item" class:on={editor.leftTab === it.tab} title={it.label} onclick={() => open(it.tab)}>
      <span class="ico">
        <Icon size={17} strokeWidth={1.6} />
        {#if it.tab === "agents" && editor.mcpLog.length}<span class="dot"></span>{/if}
      </span>
      <span class="label">{it.label}</span>
    </button>
  {/each}
</nav>

<style>
  .rail {
    width: 56px;
    flex: none;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding-top: 10px;
    background: var(--bg);
    border-right: 1px solid var(--border);
  }
  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    width: 48px;
    padding: 4px 0;
    color: var(--text-secondary);
    font-size: 10px;
  }
  .ico {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 28px;
    border-radius: 8px;
  }
  .item:hover .ico {
    background: var(--bg-secondary);
  }
  .item.on {
    color: var(--text);
  }
  .item.on .ico {
    background: var(--bg-secondary);
    color: var(--text-brand, #0d99ff);
  }
  .dot {
    position: absolute;
    top: 3px;
    right: 4px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #14ae5c;
  }
</style>
