<script lang="ts">
  import Check from "@lucide/svelte/icons/check";
  import Trash from "@lucide/svelte/icons/trash-2";
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";
  import { live } from "../live.svelte";

  // Comment pins on the canvas and the thread popover (Figma's comments).

  let draft = $state("");
  let reply = $state("");

  const visible = $derived(editor.comments.filter((c) => editor.showResolved || !c.resolved || c.id === editor.openThread));
  const thread = $derived(editor.comments.find((c) => c.id === editor.openThread) ?? null);
  const author = () => (live.status === "on" ? live.name : "You");
  const sx = (x: number) => (x - editor.view.x) * editor.view.zoom;
  const sy = (y: number) => (y - editor.view.y) * editor.view.zoom;
  const initial = (name: string) => name.trim().slice(0, 1).toUpperCase() || "?";
  const when = (t: number) => (t ? new Date(t).toLocaleString(undefined, { dateStyle: "short", timeStyle: "short" }) : "");

  function post() {
    const engine = editor.engine;
    const d = editor.commentDraft;
    if (!engine || !d || !draft.trim()) return;
    const id = engine.commentOp({ op: "add", x: d.x, y: d.y, text: draft.trim(), author: author(), time: Date.now() });
    draft = "";
    editor.commentDraft = null;
    editor.refreshAll();
    editor.openThread = id || null;
  }

  function op(change: Record<string, unknown>) {
    editor.engine?.commentOp({ id: thread?.id, author: author(), time: Date.now(), ...change });
    editor.refreshAll();
  }

  function send() {
    if (!reply.trim()) return;
    op({ op: "reply", text: reply.trim() });
    reply = "";
  }

  const keys = (submit: () => void) => (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Escape") {
      editor.commentDraft = null;
      editor.openThread = null;
    }
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      submit();
    }
  };
</script>

{#each visible as c (c.id)}
  <button
    class="pin"
    class:resolved={c.resolved}
    class:open={c.id === editor.openThread}
    style="left:{sx(c.x)}px; top:{sy(c.y)}px"
    title="{c.messages[0]?.author}: {c.messages[0]?.text}"
    onpointerdown={(e) => e.stopPropagation()}
    onclick={() => ((editor.commentDraft = null), (editor.openThread = editor.openThread === c.id ? null : c.id))}
  >
    {initial(c.messages[0]?.author ?? "?")}
    {#if c.messages.length > 1}<span class="count">{c.messages.length}</span>{/if}
  </button>
{/each}

{#if editor.commentDraft}
  {@const d = editor.commentDraft}
  <div class="card" style="left:{sx(d.x) + 18}px; top:{sy(d.y) - 8}px" role="dialog" tabindex="-1" aria-label="New comment" onpointerdown={(e) => e.stopPropagation()}>
    <!-- svelte-ignore a11y_autofocus -->
    <textarea placeholder="Add a comment" bind:value={draft} autofocus onkeydown={keys(post)}></textarea>
    <div class="row">
      <span class="faint">Enter to post</span>
      <button class="primary" disabled={!draft.trim()} onclick={post}>Post</button>
    </div>
  </div>
  <span class="pin draft" style="left:{sx(d.x)}px; top:{sy(d.y)}px">{initial(author())}</span>
{/if}

{#if thread}
  <div class="card" style="left:{sx(thread.x) + 18}px; top:{sy(thread.y) - 8}px" role="dialog" tabindex="-1" aria-label="Comment thread" onpointerdown={(e) => e.stopPropagation()}>
    <div class="head">
      <span class="faint">{thread.nodeName ? `On ${thread.nodeName}` : "On the canvas"}</span>
      <span class="actions">
        <button class="icon-btn" class:on={thread.resolved} title={thread.resolved ? "Reopen" : "Resolve"} onclick={() => op({ op: "resolve", resolved: !thread.resolved })}><Check size={14} strokeWidth={2} /></button>
        <button class="icon-btn" title="Delete thread" onclick={() => (op({ op: "delete" }), (editor.openThread = null))}><Trash size={14} strokeWidth={1.7} /></button>
        <button class="icon-btn" title="Close" onclick={() => (editor.openThread = null)}><X size={14} strokeWidth={1.8} /></button>
      </span>
    </div>
    <div class="messages">
      {#each thread.messages as m, i (i)}
        <div class="message">
          <div class="who"><span class="avatar">{initial(m.author)}</span>{m.author}<span class="faint time">{when(m.time)}</span></div>
          <p>{m.text}</p>
        </div>
      {/each}
    </div>
    <textarea placeholder="Reply" bind:value={reply} onkeydown={keys(send)}></textarea>
  </div>
{/if}

<style>
  .pin {
    position: absolute;
    z-index: 7;
    width: 28px;
    height: 28px;
    margin: -28px 0 0 0;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 14px 14px 14px 2px;
    background: #ffc700;
    color: #1e1e1e;
    font-weight: 700;
    font-size: 12px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.3);
  }
  .pin.open {
    outline: 2px solid var(--brand-strong, #0d99ff);
  }
  .pin.resolved {
    background: #9ca3af;
  }
  .pin.draft {
    pointer-events: none;
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  .count {
    position: absolute;
    right: -6px;
    top: -6px;
    min-width: 16px;
    height: 16px;
    padding: 0 3px;
    border-radius: 8px;
    background: #1e1e1e;
    color: #fff;
    font-size: 10px;
    line-height: 16px;
  }
  .card {
    position: absolute;
    z-index: 8;
    width: 280px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    border-radius: 12px;
    background: var(--menu-bg, #1e1e1e);
    box-shadow: var(--shadow-menu);
  }
  .head,
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  .actions {
    display: flex;
  }
  .icon-btn.on {
    color: #14ae5c;
  }
  .messages {
    display: flex;
    flex-direction: column;
    gap: 10px;
    max-height: 260px;
    overflow: auto;
  }
  .who {
    display: flex;
    align-items: center;
    gap: 6px;
    font-weight: 600;
  }
  .time {
    font-weight: 400;
    font-size: 10px;
  }
  .avatar {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background: #ffc700;
    color: #1e1e1e;
    font-size: 10px;
  }
  .message p {
    margin: 4px 0 0 26px;
    white-space: pre-wrap;
    line-height: 1.45;
  }
  textarea {
    min-height: 56px;
    resize: vertical;
    padding: 8px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
    border: 1px solid transparent;
    color: var(--text);
    font: inherit;
    outline: none;
  }
  textarea:focus {
    border-color: var(--brand-strong);
  }
  .primary {
    height: 26px;
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
