<script lang="ts">
  import ComponentIcon from "@lucide/svelte/icons/component";
  import Contrast from "@lucide/svelte/icons/contrast";
  import SquaresUnite from "@lucide/svelte/icons/squares-unite";
  import SquaresSubtract from "@lucide/svelte/icons/squares-subtract";
  import SquaresIntersect from "@lucide/svelte/icons/squares-intersect";
  import SquaresExclude from "@lucide/svelte/icons/squares-exclude";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Pencil from "@lucide/svelte/icons/pencil";
  import ScanSearch from "@lucide/svelte/icons/scan-search";
  import Diamond from "@lucide/svelte/icons/diamond";
  import Unlink from "@lucide/svelte/icons/unlink";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor } from "../editor.svelte";

  // Figma UI3's contextual toolbar: actions for the current selection, above the main toolbar.

  type Op = "union" | "subtract" | "intersect" | "exclude";
  let boolOpen = $state(false);
  let last = $state<Op>("union");

  const sel = $derived(editor.selection);
  const show = $derived(sel.length > 0 && !editor.interacting && editor.editing < 0 && !editor.vectorMode);
  const inInstance = $derived(sel.some((n) => n.inInstance));
  const instance = $derived(sel.length === 1 && sel[0].component === "instance");
  const editable = $derived(sel.length === 1 && (sel[0].kind === "Text" || sel[0].kind === "Vector"));

  const LABELS: Record<Op, string> = { union: "Union", subtract: "Subtract", intersect: "Intersect", exclude: "Exclude" };

  function run(op: Op) {
    last = op;
    editor.boolean(op);
  }

  const boolItems = (): MenuItem[] => [
    ...(["union", "subtract", "intersect", "exclude"] as Op[]).map((op) => ({ label: `${LABELS[op]} selection`, checked: last === op, action: () => run(op) })),
    "-",
    { label: "Flatten", shortcut: "Ctrl+E", action: () => editor.boolean("flatten") },
  ];
</script>

{#if show}
  <div class="bar" role="toolbar" aria-label="Selection actions">
    {#if editable}
      <button class="act" title={sel[0].kind === "Text" ? "Edit text (Enter)" : "Edit object (Enter)"} onclick={() => (sel[0].kind === "Text" ? editor.startEditing(sel[0].id) : editor.editVector())}>
        <Pencil size={15} strokeWidth={1.7} />
      </button>
    {/if}
    {#if instance}
      <button class="act" title="Go to main component" onclick={() => editor.goToMain()}><Diamond size={15} strokeWidth={1.7} /></button>
      <button class="act" title="Detach instance (Ctrl+Alt+B)" onclick={() => editor.detachInstance()}><Unlink size={15} strokeWidth={1.7} /></button>
    {:else}
      <button class="act" title="Create component (Ctrl+Alt+K)" disabled={inInstance} onclick={() => editor.createComponent()}>
        <ComponentIcon size={15} strokeWidth={1.7} />
      </button>
    {/if}
    <button class="act" title={sel.every((n) => n.mask) ? "Remove mask (Ctrl+Alt+M)" : "Use as mask (Ctrl+Alt+M)"} disabled={inInstance} onclick={() => editor.useAsMask()}>
      <Contrast size={15} strokeWidth={1.7} />
    </button>
    {#if sel.length > 1}
      <div class="split">
        <button class="act" title="{LABELS[last]} selection" disabled={inInstance} onclick={() => run(last)}>
          {#if last === "union"}<SquaresUnite size={15} strokeWidth={1.7} />
          {:else if last === "subtract"}<SquaresSubtract size={15} strokeWidth={1.7} />
          {:else if last === "intersect"}<SquaresIntersect size={15} strokeWidth={1.7} />
          {:else}<SquaresExclude size={15} strokeWidth={1.7} />{/if}
        </button>
        <button class="caret" data-menu-trigger aria-label="Boolean operations" disabled={inInstance} onclick={() => (boolOpen = !boolOpen)}>
          <ChevronDown size={10} strokeWidth={2} />
        </button>
        <Menu items={boolOpen ? boolItems() : []} bind:open={boolOpen} placement="above" align="center" />
      </div>
    {/if}
    <span class="divider"></span>
    <button class="act" title="Select matching layers (Ctrl+Alt+A)" onclick={() => editor.selectMatching()}>
      <ScanSearch size={15} strokeWidth={1.7} />
    </button>
  </div>
{/if}

<style>
  .bar {
    position: absolute;
    left: 50%;
    bottom: 68px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    height: 40px;
    padding: 0 6px;
    background: var(--bg);
    border-radius: 11px;
    box-shadow: var(--shadow-toolbar);
    z-index: 10;
  }
  .act {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 30px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--text);
  }
  .act:hover:not(:disabled) {
    background: var(--bg-secondary);
  }
  .act:disabled {
    opacity: 0.4;
  }
  .split {
    position: relative;
    display: flex;
    align-items: center;
  }
  .caret {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 30px;
    border-radius: var(--radius);
    color: var(--text-secondary);
  }
  .caret:hover {
    background: var(--bg-secondary);
  }
  .divider {
    width: 1px;
    height: 20px;
    margin: 0 4px;
    background: var(--border);
  }
</style>
