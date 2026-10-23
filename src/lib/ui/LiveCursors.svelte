<script lang="ts">
  import { editor } from "../editor.svelte";
  import { live } from "../live.svelte";

  // Other people's pointers on the canvas, with their names (live collaboration).

  const page = $derived(Math.max(0, editor.pages.findIndex((p) => p.current)));
  const cursors = $derived(Object.values(live.peers).filter((p) => Number.isFinite(p.x) && p.page === page));
</script>

{#each cursors as p (p.id)}
  <div class="cursor" style="left:{(p.x - editor.view.x) * editor.view.zoom}px; top:{(p.y - editor.view.y) * editor.view.zoom}px">
    <svg width="18" height="18" viewBox="0 0 18 18" aria-hidden="true">
      <path d="M2 1.5 L15 8.5 L9 10 L6.5 16 Z" fill={p.color} stroke="#fff" stroke-width="1.2" stroke-linejoin="round" />
    </svg>
    <span class="name" style="background:{p.color}">{p.name}</span>
  </div>
{/each}

<style>
  .cursor {
    position: absolute;
    z-index: 6;
    pointer-events: none;
    transition: left 60ms linear, top 60ms linear;
  }
  .name {
    position: absolute;
    left: 14px;
    top: 14px;
    padding: 2px 6px;
    border-radius: 4px;
    color: #fff;
    font-size: 11px;
    font-weight: 550;
    white-space: nowrap;
  }
</style>
