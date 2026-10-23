<script lang="ts">
  import Copy from "@lucide/svelte/icons/copy";
  import Trash from "@lucide/svelte/icons/trash-2";
  import { editor } from "../editor.svelte";

  // Agents: the built-in MCP server (how to connect Claude Code and others) and a live log
  // of what agents read and changed in this document.

  const command = $derived(editor.mcp?.url ? `claude mcp add --transport http pergel ${editor.mcp.url}` : "");

  function copy(text: string) {
    navigator.clipboard?.writeText(text).then(() => editor.notify("Copied — paste it in a terminal"));
  }

  const time = (t: number) => new Date(t).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
</script>

<section class="agents">
  <div class="section-head"><span class="section-title">Agents</span></div>
  <div class="card">
    {#if editor.mcp?.running}
      <div class="status"><span class="led on"></span>MCP server running</div>
      <code class="url">{editor.mcp.url}</code>
      <p class="faint">Connect Claude Code (run once in a terminal):</p>
      <div class="cmd">
        <code>{command}</code>
        <button class="icon-btn" title="Copy" onclick={() => copy(command)}><Copy size={13} strokeWidth={1.7} /></button>
      </div>
      <p class="faint">Then ask: “Implement the selected Pergel design” — or run the <b>/mcp__pergel__implement_design</b> prompt.</p>
    {:else}
      <div class="status"><span class="led"></span>{editor.mcp?.error ?? "The MCP server runs in the desktop app"}</div>
    {/if}
  </div>
  <div class="section-head">
    <span class="section-title">Activity</span>
    {#if editor.mcpLog.length}
      <button class="icon-btn" title="Clear" onclick={() => (editor.mcpLog = [])}><Trash size={13} strokeWidth={1.7} /></button>
    {/if}
  </div>
  <div class="log">
    {#each editor.mcpLog as e, i (i)}
      <div class="entry" class:error={!e.ok}>
        <span class="tool">{e.tool}</span>
        <span class="faint when">{time(e.time)}</span>
        {#if e.detail}<span class="detail faint">{e.detail}</span>{/if}
      </div>
    {:else}
      <p class="faint pad">Nothing yet. Every tool call an agent makes shows up here.</p>
    {/each}
  </div>
</section>

<style>
  .agents {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .section-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 40px;
    padding: 0 8px 0 16px;
  }
  .card {
    margin: 0 12px 8px;
    padding: 10px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    display: flex;
    flex-direction: column;
    gap: 6px;
    line-height: 1.45;
  }
  .card p {
    margin: 0;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 550;
  }
  .led {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: #f24822;
  }
  .led.on {
    background: #14ae5c;
  }
  code {
    font-family: ui-monospace, monospace;
    font-size: 11px;
    word-break: break-all;
  }
  .cmd {
    display: flex;
    align-items: flex-start;
    gap: 4px;
    padding: 6px;
    border-radius: 4px;
    background: var(--bg);
  }
  .cmd code {
    flex: 1;
  }
  .log {
    flex: 1;
    overflow-y: auto;
    padding: 0 8px 72px;
  }
  .entry {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 2px 8px;
    padding: 6px 8px;
    border-radius: var(--radius);
  }
  .entry:hover {
    background: var(--bg-secondary);
  }
  .tool {
    font-family: ui-monospace, monospace;
    font-size: 11px;
  }
  .entry.error .tool {
    color: #f87171;
  }
  .when {
    font-size: 10px;
  }
  .detail {
    grid-column: 1 / -1;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pad {
    padding: 0 8px;
    line-height: 1.5;
  }
</style>
