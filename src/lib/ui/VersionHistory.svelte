<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import History from "@lucide/svelte/icons/history";
  import { onMount } from "svelte";
  import { isTauri } from "@tauri-apps/api/core";
  import { editor } from "../editor.svelte";
  import { workspace } from "../workspace.svelte";
  import { ago, versions, type Version } from "../files";

  // Figma's version history: autosaved versions every few minutes plus named ones.

  let list = $state<Version[]>([]);
  let label = $state("");
  let busy = $state(false);

  async function load() {
    const id = workspace.current?.file.id;
    list = id ? await versions.list(id) : [];
  }

  async function saveNamed() {
    if (busy) return;
    busy = true;
    try {
      await workspace.saveVersion(label.trim() || "Saved version");
      label = "";
      await load();
      editor.notify("Saved to version history");
    } finally {
      busy = false;
    }
  }

  async function restore(v: Version) {
    busy = true;
    try {
      await workspace.restoreVersion(v.time);
      await load();
    } finally {
      busy = false;
    }
  }

  onMount(() => {
    void load();
  });
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && (editor.versionsOpen = false)}>
  <div class="panel" role="dialog" aria-label="Version history">
    <header>
      <History size={15} strokeWidth={1.8} />
      <span class="title">Version history</span>
      <span class="spacer"></span>
      <button class="icon-btn" title="Close" onclick={() => (editor.versionsOpen = false)}><X size={14} strokeWidth={1.8} /></button>
    </header>
    {#if !isTauri()}
      <p class="faint pad">Version history is kept by the desktop app.</p>
    {:else}
      <div class="save">
        <input
          placeholder="Name this version (e.g. Before client review)"
          bind:value={label}
          onkeydown={(e) => { e.stopPropagation(); if (e.key === "Enter") void saveNamed(); if (e.key === "Escape") editor.versionsOpen = false; }}
        />
        <button class="primary" disabled={busy} onclick={saveNamed}>Save version</button>
      </div>
      <div class="list">
        <div class="item current">
          <span class="dot on"></span>
          <div class="meta">
            <span class="name">Current version</span>
            <span class="faint">{workspace.current ? `Edited ${ago(workspace.current.file.modified)}` : ""}</span>
          </div>
        </div>
        {#each list as v (v.time)}
          <div class="item">
            <span class="dot" class:named={v.label}></span>
            <div class="meta">
              <span class="name">{v.label || "Autosaved"}</span>
              <span class="faint">{new Date(v.time).toLocaleString()} · {Math.max(1, Math.round(v.size / 1024))} KB</span>
            </div>
            <span class="actions">
              <button class="ghost" disabled={busy} onclick={() => restore(v)}>Restore</button>
              <button class="ghost" disabled={busy} onclick={() => workspace.openVersionCopy(v.time, v.label)}>Open copy</button>
            </span>
          </div>
        {:else}
          <p class="faint pad">No earlier versions yet. Pergel keeps one every 10 minutes while you work; save named versions any time.</p>
        {/each}
      </div>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 160;
    display: flex;
    justify-content: flex-end;
    background: rgba(0, 0, 0, 0.25);
  }
  .panel {
    width: 360px;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg, #2c2c2c);
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 44px;
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .save {
    display: flex;
    gap: 6px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 30px;
    padding: 0 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    outline: none;
  }
  input:focus {
    border-color: var(--brand-strong);
  }
  .primary {
    padding: 0 12px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .list {
    flex: 1;
    overflow-y: auto;
    padding: 8px 0;
  }
  .item {
    position: relative;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 16px;
  }
  .item:hover {
    background: var(--bg-secondary);
  }
  .dot {
    width: 8px;
    height: 8px;
    margin-top: 4px;
    border-radius: 50%;
    border: 1px solid var(--text-secondary);
    flex: none;
  }
  .dot.on,
  .dot.named {
    background: var(--brand-strong, #0d99ff);
    border-color: var(--brand-strong, #0d99ff);
  }
  .meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .name {
    font-weight: 550;
  }
  .actions {
    display: none;
    gap: 4px;
  }
  .item:hover .actions {
    display: flex;
  }
  .ghost {
    height: 24px;
    padding: 0 8px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
  }
  .ghost:hover {
    background: var(--bg);
  }
  .pad {
    padding: 12px 16px;
    line-height: 1.5;
  }
</style>
