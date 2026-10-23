<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy";
  import Radio from "@lucide/svelte/icons/radio";
  import { editor } from "../editor.svelte";
  import { live } from "../live.svelte";

  // The Share button's panel: start a live session on the local network or join one.

  let { onclose }: { onclose: () => void } = $props();
  let link = $state("");

  const peers = $derived(Object.values(live.peers));

  async function copy() {
    try {
      await navigator.clipboard.writeText(live.shareUrl);
      editor.notify("Session link copied");
    } catch {
      editor.notify("Copy the link from the field");
    }
  }

  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") onclose();
  };
</script>

<div class="share-panel" role="dialog" aria-label="Live collaboration">
  <div class="title"><Radio size={14} strokeWidth={1.8} />Live collaboration</div>
  <label class="field">
    <span class="faint">Your name</span>
    <input value={live.name} onkeydown={stop} onchange={(e) => live.setName((e.target as HTMLInputElement).value)} />
  </label>
  {#if live.status === "off"}
    <button class="primary" onclick={() => live.host()}>Start live session</button>
    <div class="or faint">or join one</div>
    <div class="join">
      <input placeholder="ws://192.168.1.20:3856/live?token=…" bind:value={link} onkeydown={(e) => { stop(e); if (e.key === "Enter" && link) live.join(link); }} />
      <button class="ghost" disabled={!link} onclick={() => live.join(link)}>Join</button>
    </div>
    <p class="faint hint">People on the same network open this file live: they see each other's cursors and every change.</p>
  {:else}
    <div class="state">
      <span class="dot" class:on={live.status === "on"}></span>
      {live.status === "on" ? (live.hosting ? "Sharing" : "Connected") : "Connecting…"}
      <span class="faint">· {peers.length + 1} {peers.length ? "people" : "person"}</span>
    </div>
    {#if live.shareUrl}
      <div class="join">
        <input readonly value={live.shareUrl} onkeydown={stop} />
        <button class="icon-btn" title="Copy link" onclick={copy}><Copy size={14} strokeWidth={1.7} /></button>
      </div>
    {/if}
    <ul class="people">
      <li><span class="swatch" style="background:{live.color}"></span>{live.name} <span class="faint">(you)</span></li>
      {#each peers as p (p.id)}
        <li><span class="swatch" style="background:{p.color}"></span>{p.name}</li>
      {/each}
    </ul>
    <button class="ghost" onclick={() => live.disconnect()}>{live.hosting ? "Stop sharing" : "Leave session"}</button>
  {/if}
  {#if live.error}<p class="error">{live.error}</p>{/if}
</div>

<style>
  .share-panel {
    position: absolute;
    top: 44px;
    right: 8px;
    z-index: 130;
    width: 300px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  .title {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .field {
    display: grid;
    grid-template-columns: 72px 1fr;
    align-items: center;
    gap: 8px;
  }
  input {
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
  .join {
    display: flex;
    gap: 6px;
  }
  .join input {
    flex: 1;
  }
  .primary {
    height: 30px;
    border-radius: var(--radius);
    background: var(--brand-strong, #0d99ff);
    color: #fff;
    font-weight: 550;
  }
  .ghost {
    height: 28px;
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .or {
    text-align: center;
    font-size: 11px;
  }
  .hint,
  .error {
    margin: 0;
    font-size: 11px;
    line-height: 1.45;
  }
  .error {
    color: #f87171;
  }
  .state {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #f59e0b;
  }
  .dot.on {
    background: #14ae5c;
  }
  .people {
    margin: 0;
    padding: 0;
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .people li {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .swatch {
    width: 10px;
    height: 10px;
    border-radius: 50%;
  }
</style>
