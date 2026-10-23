<script lang="ts">
  import X from "@lucide/svelte/icons/x";
  import { editor } from "../editor.svelte";
  import { onMount } from "svelte";
  import { importFigma, parseFigmaUrl, rememberToken, savedToken, type ImportProgress } from "../figma";
  import type { FigmaReport } from "../engine";

  // A pasted link comes in filled; with a saved token the import starts right away.
  let url = $state(editor.figmaImportUrl);
  let token = $state(savedToken());
  let remember = $state(savedToken() !== "");
  let busy = $state(false);
  let progress = $state<ImportProgress | null>(null);
  let error = $state("");
  let report = $state<FigmaReport | null>(null);

  const valid = $derived(parseFigmaUrl(url) !== null && token.trim().length > 10);

  function close() {
    if (busy) return;
    editor.figmaImport = false;
  }

  async function run() {
    if (!valid || busy) return;
    busy = true;
    error = "";
    report = null;
    rememberToken(token.trim(), remember);
    try {
      report = await importFigma(url, token.trim(), (p) => (progress = p));
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      busy = false;
      progress = null;
    }
  }

  onMount(() => {
    editor.figmaImportUrl = "";
    if (valid) void run();
  });

  const stop = (e: KeyboardEvent) => {
    e.stopPropagation();
    if (e.key === "Enter") void run();
    if (e.key === "Escape") close();
  };
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && close()}>
  <div class="dialog" role="dialog" aria-label="Import from Figma">
    <header>
      <span class="title">Import from Figma</span>
      <button class="icon-btn" title="Close" onclick={close}><X size={14} strokeWidth={1.8} /></button>
    </header>
    {#if report}
      <div class="body">
        <p class="ok">Imported and opened in a new tab.</p>
        <ul>
          <li>{report.pages} page{report.pages === 1 ? "" : "s"}, {report.layers.toLocaleString()} layers</li>
          <li>{report.components} component{report.components === 1 ? "" : "s"} (instances stay linked)</li>
          <li>{report.images} image{report.images === 1 ? "" : "s"}{report.missing_images ? `, ${report.missing_images} could not be downloaded` : ""}</li>
          {#if report.fonts.length}
            <li>Fonts: {report.fonts.join(", ")}</li>
          {/if}
        </ul>
        <p class="faint">Fonts Pergel doesn't have fall back to Inter until you add them.</p>
      </div>
      <footer>
        <button class="secondary" onclick={() => (report = null)}>Import another</button>
        <button class="primary" onclick={close}>Done</button>
      </footer>
    {:else}
      <div class="body">
        <label>
          <span>Figma file or frame link</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={url} placeholder="https://www.figma.com/design/…" spellcheck="false" autofocus onkeydown={stop} disabled={busy} />
        </label>
        <label>
          <span>Personal access token</span>
          <input bind:value={token} type="password" placeholder="figd_…" spellcheck="false" onkeydown={stop} disabled={busy} />
        </label>
        <p class="faint help">
          Create one in Figma → Settings → Security → Personal access tokens with the scope “File content: read”. It is
          only sent to api.figma.com.
        </p>
        <label class="check">
          <input type="checkbox" bind:checked={remember} disabled={busy} />
          <span>Remember the token on this computer</span>
        </label>
        {#if progress}
          <p class="progress">
            {progress.step}{progress.total ? ` ${progress.done}/${progress.total}` : ""}
          </p>
        {/if}
        {#if error}<p class="error">{error}</p>{/if}
      </div>
      <footer>
        <button class="secondary" onclick={close} disabled={busy}>Cancel</button>
        <button class="primary" onclick={run} disabled={!valid || busy}>{busy ? "Importing…" : "Import"}</button>
      </footer>
    {/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.45);
  }
  .dialog {
    width: 440px;
    background: var(--menu-bg, #1e1e1e);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 44px;
    padding: 0 8px 0 16px;
    border-bottom: 1px solid var(--border);
  }
  .title {
    font-weight: 600;
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 16px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  label > span {
    color: var(--text-secondary);
  }
  input:not([type="checkbox"]) {
    height: 32px;
    padding: 0 10px;
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
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  .help {
    margin: -4px 0 0;
    line-height: 1.5;
  }
  .progress {
    margin: 0;
    color: var(--text-brand);
  }
  .error {
    margin: 0;
    color: #f87171;
    line-height: 1.5;
  }
  .ok {
    margin: 0;
    font-weight: 550;
  }
  ul {
    margin: 0;
    padding-left: 18px;
    line-height: 1.7;
  }
  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 0 16px 16px;
  }
  button.primary,
  button.secondary {
    height: 32px;
    padding: 0 14px;
    border-radius: var(--radius);
    font-weight: 550;
  }
  button.primary {
    background: var(--brand-strong, #0d99ff);
    color: #fff;
  }
  button.primary:disabled {
    opacity: 0.5;
  }
  button.secondary {
    border: 1px solid var(--border);
  }
</style>
