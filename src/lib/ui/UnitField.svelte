<script lang="ts">
  import type { Snippet } from "svelte";

  // A field whose value carries a unit ("Auto", "150%", "24"); parsing is left to the caller.

  let {
    value,
    label,
    title = "",
    onCommit,
  }: { value: string; label: Snippet; title?: string; onCommit: (raw: string) => void } = $props();

  let input = $state<HTMLInputElement>();
  let text = $state("");
  let editing = $state(false);

  function commit() {
    editing = false;
    if (text.trim() !== value) onCommit(text);
  }
</script>

<label class="field" {title}>
  <span class="label">{@render label()}</span>
  <input
    bind:this={input}
    value={editing ? text : value}
    spellcheck="false"
    onfocus={(e) => {
      editing = true;
      text = value;
      queueMicrotask(() => (e.target as HTMLInputElement).select());
    }}
    oninput={(e) => (text = (e.target as HTMLInputElement).value)}
    onblur={commit}
    onkeydown={(e) => {
      e.stopPropagation();
      if (e.key === "Enter") input?.blur();
      if (e.key === "Escape") {
        text = value;
        input?.blur();
      }
    }}
  />
</label>

<style>
  .field {
    display: flex;
    align-items: center;
    height: 24px;
    min-width: 0;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    outline: 1px solid transparent;
    outline-offset: -1px;
  }
  .field:hover {
    outline-color: var(--border);
  }
  .field:focus-within {
    outline-color: var(--brand-strong);
  }
  .label {
    flex: none;
    width: 24px;
    display: inline-flex;
    justify-content: center;
    color: var(--text-tertiary);
  }
  input {
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 6px 0 0;
    background: transparent;
    border: 0;
    outline: none;
  }
</style>
