<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";

  // Figma's "Find and replace…" for text layers on the current page.

  let find = $state("");
  let replace = $state("");
  let matchCase = $state(false);
  let result = $state<{ layers: number; matches: number } | null>(null);

  function search() {
    if (!find || !editor.engine) return (result = null);
    result = editor.engine.findReplace(find, replace, matchCase, true);
    editor.refreshSelection();
    editor.revealSelection();
    editor.invalidate();
  }

  function replaceAll() {
    if (!find || !editor.engine) return;
    const r = editor.engine.findReplace(find, replace, matchCase, false);
    editor.refreshAll();
    editor.notify(`Replaced ${r.matches} match${r.matches === 1 ? "" : "es"} in ${r.layers} layer${r.layers === 1 ? "" : "s"}`);
    result = { layers: 0, matches: 0 };
  }

  const keys = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") editor.findOpen = false;
    if (e.key === "Enter") search();
  };
</script>

<div class="panel" role="dialog" aria-label="Find and replace">
  <header>
    <span class="title">Find and replace</span>
    <button class="icon-btn" title="Close" onclick={() => (editor.findOpen = false)}><X size={14} strokeWidth={1.8} /></button>
  </header>
  <!-- svelte-ignore a11y_autofocus -->
  <input placeholder="Find" bind:value={find} autofocus oninput={search} onkeydown={keys} />
  <input placeholder="Replace with" bind:value={replace} onkeydown={keys} />
  <label class="check"><input type="checkbox" bind:checked={matchCase} onchange={search} />Match case</label>
  <div class="row">
    <span class="faint">{result ? `${result.matches} match${result.matches === 1 ? "" : "es"} in ${result.layers} layer${result.layers === 1 ? "" : "s"}` : ""}</span>
    <button class="primary" disabled={!result?.matches} onclick={replaceAll}>Replace all</button>
  </div>
</div>

<style>
  .panel {
    position: fixed;
    top: 56px;
    right: 260px;
    z-index: 120;
    width: 280px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 0 12px 12px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    margin: 0 -4px 0 4px;
  }
  .title {
    font-weight: 600;
  }
  input:not([type="checkbox"]) {
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
  .check {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .primary {
    height: 28px;
    padding: 0 12px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .primary:disabled {
    opacity: 0.5;
  }
</style>
