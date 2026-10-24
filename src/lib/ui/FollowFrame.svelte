<script lang="ts">
  import { live } from "../live.svelte";

  // While following someone's view: a frame in their colour around the canvas, and who it is.

  const peer = $derived(live.following ? live.peers[live.following] : null);
</script>

{#if peer}
  <div class="frame" style="--c:{peer.color}">
    <div class="label">
      {peer.spotlight ? `${peer.name} is presenting` : `Following ${peer.name}`}
      <button onclick={() => live.unfollow()}>Stop</button>
    </div>
  </div>
{/if}

<style>
  .frame {
    position: absolute;
    inset: 0;
    z-index: 7;
    pointer-events: none;
    box-shadow: inset 0 0 0 2px var(--c);
  }
  .label {
    position: absolute;
    top: 0;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 3px 4px 4px 10px;
    border-radius: 0 0 6px 6px;
    background: var(--c);
    color: #fff;
    font-size: 11px;
    font-weight: 550;
    white-space: nowrap;
    pointer-events: auto;
  }
  button {
    padding: 1px 7px;
    border: none;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.25);
    color: #fff;
    font-size: 11px;
    cursor: pointer;
  }
</style>
