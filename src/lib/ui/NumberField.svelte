<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    value,
    label,
    suffix = "",
    min = -Infinity,
    max = Infinity,
    step = 1,
    disabled = false,
    title = "",
    mixed = false,
    onCommit,
  }: {
    value: number;
    /** The selected layers disagree: show "Mixed" until a value is typed. */
    mixed?: boolean;
    label: string | Snippet;
    suffix?: string;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    title?: string;
    onCommit: (v: number) => void;
  } = $props();

  let editing = $state(false);
  let text = $state("");
  let input = $state<HTMLInputElement>();

  const shown = $derived(mixed ? "Mixed" : `${Math.round(value * 100) / 100}${suffix}`);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));

  function evaluate(raw: string): number | null {
    const s = raw.replace(suffix, "").replace(/,/g, ".").trim();
    if (!/^[\d+\-*/().\s]+$/.test(s)) return null;
    try {
      const v = Function(`"use strict"; return (${s});`)() as number;
      return Number.isFinite(v) ? v : null;
    } catch {
      return null;
    }
  }

  function commit() {
    editing = false;
    const v = evaluate(text);
    if (v !== null && (mixed || v !== value)) onCommit(clamp(v));
  }

  function onKey(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Enter") input?.blur();
    else if (e.key === "Escape") {
      text = shown;
      editing = false;
      input?.blur();
    } else if ((e.key === "ArrowUp" || e.key === "ArrowDown") && !mixed) {
      e.preventDefault();
      const delta = (e.key === "ArrowUp" ? 1 : -1) * step * (e.shiftKey ? 10 : 1);
      onCommit(clamp(value + delta));
      text = `${Math.round(clamp(value + delta) * 100) / 100}`;
    }
  }

  function scrub(e: PointerEvent) {
    if (disabled || mixed) return;
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    let last = e.clientX;
    let current = value;
    const move = (ev: PointerEvent) => {
      const dx = ev.clientX - last;
      last = ev.clientX;
      if (dx === 0) return;
      current = clamp(current + dx * step * (ev.shiftKey ? 10 : 1));
      onCommit(Math.round(current * 100) / 100);
    };
    const up = () => {
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", up);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", up);
  }
</script>

<label class="field" class:disabled {title}>
  <span class="label" role="presentation" onpointerdown={scrub}>
    {#if typeof label === "string"}{label}{:else}{@render label()}{/if}
  </span>
  <input
    bind:this={input}
    value={editing ? text : shown}
    {disabled}
    spellcheck="false"
    onfocus={(e) => {
      editing = true;
      text = mixed ? "" : shown.replace(suffix, "");
      queueMicrotask(() => (e.target as HTMLInputElement).select());
    }}
    oninput={(e) => (text = (e.target as HTMLInputElement).value)}
    onblur={commit}
    onkeydown={onKey}
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
  .field.disabled {
    opacity: 0.5;
  }
  .label {
    flex: none;
    width: 24px;
    display: inline-flex;
    justify-content: center;
    color: var(--text-tertiary);
    cursor: ew-resize;
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
