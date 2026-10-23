<script lang="ts">
  import GitBranch from "@lucide/svelte/icons/git-branch";
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";
  import { workspace } from "../workspace.svelte";

  // Figma's branching: create a branch of the file, review what it changed, merge it back.

  let name = $state("");
  let busy = $state(false);
  const info = $derived(editor.engine?.branch() ?? null);
  const changes = $derived(info && editor.engine ? editor.engine.docDiff(info.base, editor.engine.docJson()) : []);
  const LABEL = { added: "Added", changed: "Changed", deleted: "Deleted" } as const;

  async function create() {
    busy = true;
    await workspace.createBranch(name.trim());
    busy = false;
    editor.branchOpen = false;
  }

  async function merge() {
    busy = true;
    await workspace.mergeBranch();
    busy = false;
    editor.branchOpen = false;
  }

  const keys = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") editor.branchOpen = false;
    if (e.key === "Enter" && !info) create();
  };
</script>

<div class="panel" role="dialog" aria-label="Branches">
  <header>
    <span class="title"><GitBranch size={14} strokeWidth={1.8} />{info ? "Branch" : "Create branch"}</span>
    <button class="icon-btn" title="Close" onclick={() => (editor.branchOpen = false)}><X size={14} strokeWidth={1.8} /></button>
  </header>
  {#if info}
    <p class="faint">Branch of <strong>{info.of_name}</strong>{info.merged ? ` · merged ${new Date(info.merged).toLocaleString()}` : ""}</p>
    {#if changes.length}
      <ul class="changes">
        {#each changes as c, i (i)}
          <li><span class="tag {c.change}">{LABEL[c.change]}</span>{c.name}{#if c.page}<span class="faint"> · {c.page}</span>{/if}</li>
        {/each}
      </ul>
    {:else}
      <p class="faint">No changes yet.</p>
    {/if}
    <p class="hint faint">Merging brings these into {info.of_name}. Its own changes since the branch stay; where both changed the same frame, the branch wins. A version is saved before and after.</p>
    <button class="primary" disabled={busy || !changes.length} onclick={merge}>{busy ? "Merging…" : `Merge into ${info.of_name}`}</button>
  {:else}
    <p class="faint">Work on a copy of this file without changing it, then merge the changes back.</p>
    <!-- svelte-ignore a11y_autofocus -->
    <input placeholder="Branch name (e.g. New checkout)" bind:value={name} autofocus onkeydown={keys} />
    <button class="primary" disabled={busy} onclick={create}>{busy ? "Creating…" : "Create branch"}</button>
  {/if}
</div>

<style>
  .panel {
    position: fixed;
    top: 56px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 130;
    width: 360px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 0 14px 14px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 42px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  p {
    margin: 0;
    line-height: 1.45;
  }
  .hint {
    font-size: 11px;
  }
  input {
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
  .changes {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 220px;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .changes li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .tag {
    min-width: 58px;
    padding: 1px 6px;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 600;
    text-align: center;
  }
  .tag.added {
    background: rgba(20, 174, 92, 0.25);
    color: #4ade80;
  }
  .tag.changed {
    background: rgba(13, 153, 255, 0.25);
    color: #60a5fa;
  }
  .tag.deleted {
    background: rgba(242, 72, 34, 0.25);
    color: #f87171;
  }
  .primary {
    height: 30px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .primary:disabled {
    opacity: 0.5;
  }
</style>
