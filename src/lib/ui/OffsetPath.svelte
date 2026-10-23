<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";

  // Figma's "Offset path": a new layer grown (or shrunk) from the selected shapes.

  let offset = $state(8);
  let join = $state(1);

  function apply() {
    editor.offsetPath(offset, join);
    editor.offsetOpen = false;
  }

  const keys = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") editor.offsetOpen = false;
    if (e.key === "Enter") apply();
  };
</script>

<div class="panel" role="dialog" aria-label="Offset path">
  <header>
    <span class="title">Offset path</span>
    <button class="icon-btn" title="Close" onclick={() => (editor.offsetOpen = false)}><X size={14} strokeWidth={1.8} /></button>
  </header>
  <label class="field">
    <span class="faint">Offset</span>
    <!-- svelte-ignore a11y_autofocus -->
    <input type="number" step="1" bind:value={offset} autofocus onkeydown={keys} />
  </label>
  <label class="field">
    <span class="faint">Corners</span>
    <select bind:value={join} onkeydown={keys}>
      <option value={0}>Miter</option>
      <option value={1}>Round</option>
      <option value={2}>Bevel</option>
    </select>
  </label>
  <p class="faint hint">Positive grows outward, negative shrinks inward. The original stays.</p>
  <button class="primary" disabled={!offset} onclick={apply}>Offset</button>
</div>

<style>
  .panel {
    position: fixed;
    top: 56px;
    right: 260px;
    z-index: 120;
    width: 240px;
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
  .field {
    display: grid;
    grid-template-columns: 64px 1fr;
    align-items: center;
    gap: 8px;
  }
  input,
  select {
    height: 28px;
    min-width: 0;
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
  .hint {
    margin: 0;
    font-size: 11px;
  }
  .primary {
    height: 28px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .primary:disabled {
    opacity: 0.5;
  }
</style>
