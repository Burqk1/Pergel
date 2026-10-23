<script lang="ts">
  // A Figma-style slider. Dragging calls `onstart`, `oninput` many times, then `onend`, so the
  // caller can make the whole drag one undo step.
  let {
    value,
    min = 0,
    max = 100,
    title = "",
    oninput,
    onstart,
    onend,
  }: {
    value: number;
    min?: number;
    max?: number;
    title?: string;
    oninput: (v: number) => void;
    onstart?: () => void;
    onend?: () => void;
  } = $props();

  const pct = $derived(((Math.min(max, Math.max(min, value)) - min) / (max - min || 1)) * 100);
</script>

<input
  class="range"
  type="range"
  {min}
  {max}
  step="1"
  {title}
  value={Math.min(max, Math.max(min, value))}
  style="--pct:{pct}%"
  onpointerdown={() => onstart?.()}
  oninput={(e) => oninput(Number((e.target as HTMLInputElement).value))}
  onchange={() => onend?.()}
  onkeydown={(e) => e.stopPropagation()}
/>

<style>
  .range {
    width: 100%;
    height: 16px;
    margin: 0;
    background: transparent;
    appearance: none;
    -webkit-appearance: none;
  }
  .range::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 2px;
    background: linear-gradient(to right, var(--brand-strong) var(--pct), var(--bg-tertiary, #444) var(--pct));
  }
  .range::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 12px;
    height: 12px;
    margin-top: -4px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.3);
  }
</style>
