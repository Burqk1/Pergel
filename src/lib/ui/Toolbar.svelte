<script lang="ts">
  import MousePointer2 from "@lucide/svelte/icons/mouse-pointer-2";
  import Hand from "@lucide/svelte/icons/hand";
  import Scaling from "@lucide/svelte/icons/scaling";
  import Hash from "@lucide/svelte/icons/hash";
  import PanelTop from "@lucide/svelte/icons/panel-top";
  import SliceIcon from "@lucide/svelte/icons/slice";
  import Square from "@lucide/svelte/icons/square";
  import Circle from "@lucide/svelte/icons/circle";
  import Slash from "@lucide/svelte/icons/slash";
  import ArrowUpRight from "@lucide/svelte/icons/arrow-up-right";
  import Triangle from "@lucide/svelte/icons/triangle";
  import StarIcon from "@lucide/svelte/icons/star";
  import PenTool from "@lucide/svelte/icons/pen-tool";
  import Pencil from "@lucide/svelte/icons/pencil";
  import Type from "@lucide/svelte/icons/type";
  import MessageCircle from "@lucide/svelte/icons/message-circle";
  import Sparkles from "@lucide/svelte/icons/sparkles";
  import CodeXml from "@lucide/svelte/icons/code-xml";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import Menu, { type MenuItem } from "./Menu.svelte";
  import { editor, type Tool } from "../editor.svelte";

  let moveFace = $state<Tool>("move");
  let shapeFace = $state<Tool>("rectangle");
  let frameFace = $state<Tool>("frame");
  let openMenu = $state<string | null>(null);

  $effect(() => {
    if (editor.tool === "move" || editor.tool === "hand" || editor.tool === "scale") moveFace = editor.tool;
    if (editor.tool === "frame" || editor.tool === "section" || editor.tool === "slice") frameFace = editor.tool;
    if (["rectangle", "ellipse", "line", "arrow", "polygon", "star"].includes(editor.tool)) shapeFace = editor.tool;
  });

  const soon = (name: string) => () => editor.notify(`${name} is coming soon`);

  const menus: Record<string, () => MenuItem[]> = {
    move: () => [
      { label: "Move", shortcut: "V", checked: editor.tool === "move", action: () => editor.setTool("move") },
      { label: "Hand tool", shortcut: "H", checked: editor.tool === "hand", action: () => editor.setTool("hand") },
      { label: "Scale", shortcut: "K", checked: editor.tool === "scale", action: () => editor.setTool("scale") },
    ],
    frame: () => [
      { label: "Frame", shortcut: "F", checked: editor.tool === "frame", action: () => editor.setTool("frame") },
      { label: "Section", shortcut: "Shift+S", checked: editor.tool === "section", action: () => editor.setTool("section") },
      { label: "Slice", shortcut: "S", checked: editor.tool === "slice", action: () => editor.setTool("slice") },
    ],
    shape: () => [
      { label: "Rectangle", shortcut: "R", checked: editor.tool === "rectangle", action: () => editor.setTool("rectangle") },
      { label: "Line", shortcut: "L", checked: editor.tool === "line", action: () => editor.setTool("line") },
      { label: "Arrow", shortcut: "Shift+L", checked: editor.tool === "arrow", action: () => editor.setTool("arrow") },
      { label: "Ellipse", shortcut: "O", checked: editor.tool === "ellipse", action: () => editor.setTool("ellipse") },
      { label: "Polygon", checked: editor.tool === "polygon", action: () => editor.setTool("polygon") },
      { label: "Star", checked: editor.tool === "star", action: () => editor.setTool("star") },
      "-",
      { label: "Image/video…", shortcut: "Ctrl+Shift+K", action: () => editor.placeImageDialog() },
      { label: "Sticky note", checked: editor.tool === "sticky", action: () => editor.setTool("sticky") },
    ],
  };

  function toggle(name: string) {
    openMenu = openMenu === name ? null : name;
  }
</script>

{#snippet icon(tool: Tool)}
  {#if tool === "hand"}<Hand size={16} strokeWidth={1.6} />
  {:else if tool === "scale"}<Scaling size={16} strokeWidth={1.6} />
  {:else if tool === "frame"}<Hash size={16} strokeWidth={1.6} />
  {:else if tool === "section"}<PanelTop size={16} strokeWidth={1.6} />
  {:else if tool === "slice"}<SliceIcon size={16} strokeWidth={1.6} />
  {:else if tool === "rectangle"}<Square size={16} strokeWidth={1.6} />
  {:else if tool === "ellipse"}<Circle size={16} strokeWidth={1.6} />
  {:else if tool === "line"}<Slash size={16} strokeWidth={1.6} />
  {:else if tool === "arrow"}<ArrowUpRight size={16} strokeWidth={1.6} />
  {:else if tool === "polygon"}<Triangle size={16} strokeWidth={1.6} />
  {:else if tool === "star"}<StarIcon size={16} strokeWidth={1.6} />
  {:else}<MousePointer2 size={16} strokeWidth={1.6} />{/if}
{/snippet}

{#snippet group(name: string, face: Tool, title: string)}
  <div class="group">
    <button class="tool" class:on={editor.tool === face} {title} onclick={() => editor.setTool(face)}>
      {@render icon(face)}
    </button>
    <button class="caret" class:open={openMenu === name} data-menu-trigger aria-label="{title} options" onclick={() => toggle(name)}>
      <ChevronDown size={10} strokeWidth={2} />
    </button>
    <Menu items={menus[name]()} open={openMenu === name} placement="above" align="left" />
  </div>
{/snippet}

<div class="toolbar" role="toolbar">
  {@render group("move", moveFace, "Move (V)")}
  {@render group("frame", frameFace, frameFace === "section" ? "Section (Shift+S)" : frameFace === "slice" ? "Slice (S)" : "Frame (F)")}
  {@render group("shape", shapeFace, "Shapes (R, O)")}
  <button class="tool" class:on={editor.tool === "pen" || editor.vectorMode > 0} title="Pen (P)" onclick={() => editor.setTool("pen")}><PenTool size={16} strokeWidth={1.6} /></button>
  <button class="tool" class:on={editor.tool === "pencil"} title="Pencil (Shift+P)" onclick={() => editor.setTool("pencil")}><Pencil size={16} strokeWidth={1.6} /></button>
  <button class="tool" title="Text (T)" onclick={() => editor.setTool("text")}><Type size={16} strokeWidth={1.6} /></button>
  <button class="tool" title="Comment (C)" onclick={() => editor.setTool("comment")}>
    <MessageCircle size={16} strokeWidth={1.6} />
  </button>
  <span class="divider"></span>
  <button class="tool" title="Actions (Ctrl+K)" onclick={() => (editor.quickActions = true)}><Sparkles size={16} strokeWidth={1.6} /></button>
  <button class="tool dev" class:dev-on={editor.devMode} title="Dev Mode (Shift+D)" onclick={() => (editor.devMode = !editor.devMode)}>
    <CodeXml size={16} strokeWidth={1.6} />
  </button>
</div>

<style>
  .toolbar {
    position: absolute;
    left: 50%;
    bottom: 12px;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    height: 48px;
    padding: 0 8px;
    background: var(--bg);
    border-radius: 13px;
    box-shadow: var(--shadow-toolbar);
    z-index: 10;
  }
  .group {
    position: relative;
    display: flex;
    align-items: center;
  }
  .tool {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 32px;
    height: 32px;
    border-radius: var(--radius);
    color: var(--text);
  }
  .tool:hover {
    background: var(--bg-secondary);
  }
  .tool.on {
    background: var(--brand-strong);
    color: #fff;
  }
  .caret {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 32px;
    margin-left: -1px;
    border-radius: var(--radius);
    color: var(--text-secondary);
  }
  .caret:hover,
  .caret.open {
    background: var(--bg-secondary);
    color: var(--text);
  }
  .divider {
    width: 1px;
    height: 24px;
    margin: 0 6px;
    background: var(--border);
  }
  .dev-on {
    background: #0fa958;
    color: #fff;
  }
  .dev-on:hover {
    background: #12b866;
  }
</style>
