<script lang="ts">
  import { untrack } from "svelte";
  import { editor } from "../editor.svelte";
  import type { NodeInfo } from "../engine";

  // While a text layer is being edited the canvas skips its text and this textarea, styled to
  // match the engine's layout, takes its place: native caret, selection, IME and clipboard.

  const node = $derived(editor.selection.find((n) => n.id === editor.editing) ?? null);
  const view = $derived(editor.view);

  /** The engine registers the bundled Inter as "Inter"; in CSS it is the fontsource family. */
  const cssFamily = (family: string) =>
    family === "Inter" ? '"Inter Variable", Inter, sans-serif' : `"${family}", "Inter Variable", sans-serif`;

  function style(n: NodeInfo, zoom: number) {
    const t = n.text!;
    const auto = t.resize === "auto-width";
    const pad =
      t.resize === "auto-width" || t.vertical === "top"
        ? 0
        : (t.vertical === "middle" ? (n.h - t.textHeight) / 2 : n.h - t.textHeight) * zoom;
    const alpha = (n.fillOpacity / 100) * (n.opacity / 100);
    const rgb = n.fill.replace("#", "");
    const [r, g, b] = [0, 2, 4].map((i) => parseInt(rgb.slice(i, i + 2), 16));
    const transform = { original: "none", upper: "uppercase", lower: "lowercase", title: "capitalize" }[t.case];
    const decoration = { none: "none", underline: "underline", strikethrough: "line-through" }[t.decoration];
    return [
      `font-family:${cssFamily(t.family)}`,
      `font-size:${t.size * zoom}px`,
      `font-weight:${t.weight}`,
      `font-style:${t.italic ? "italic" : "normal"}`,
      `font-variation-settings:"wght" ${t.weight}`,
      `line-height:${t.linePx * zoom}px`,
      `letter-spacing:${((t.size * t.letterSpacing) / 100) * zoom}px`,
      `text-align:${t.align}`,
      `text-transform:${transform}`,
      `text-decoration:${decoration}`,
      `color:rgba(${r},${g},${b},${alpha})`,
      `white-space:${auto ? "pre" : "pre-wrap"}`,
      `padding-top:${Math.max(0, pad)}px`,
      // Auto-width boxes grow as the engine re-measures; leave room so typing never clips.
      `width:${auto ? n.w * zoom + t.size * zoom * 4 : n.w * zoom}px`,
      `height:${Math.max(n.h * zoom, t.linePx * zoom)}px`,
      `left:${(n.x - view.x) * zoom}px`,
      `top:${(n.y - view.y) * zoom}px`,
      // Rotated layers: turn the textarea about the layer's centre.
      `transform-origin:${(n.w * zoom) / 2}px ${(n.h * zoom) / 2}px`,
      `transform:rotate(${-n.rotation}deg)`,
    ].join(";");
  }

  function input(e: Event) {
    const engine = editor.engine;
    if (!engine || editor.editing < 0) return;
    engine.setText(editor.editing, (e.target as HTMLTextAreaElement).value);
    editor.refreshSelection();
    editor.refreshLayers();
    editor.invalidate();
  }

  /** Ctrl+B / I / U / Shift+X: style the selected characters (or the whole layer). */
  function styleSelection(area: HTMLTextAreaElement, style: Record<string, unknown>) {
    const engine = editor.engine;
    if (!engine || editor.editing < 0) return;
    // The engine counts characters (code points); the textarea counts UTF-16 units.
    const toChars = (i: number) => Array.from(area.value.slice(0, i)).length;
    const [a, b] = [toChars(area.selectionStart), toChars(area.selectionEnd)];
    if (a === b) {
      const t = node?.text;
      if (!t) return;
      if ("weight" in style) editor.setTextStyle({ weight: t.weight >= 600 ? 400 : 700 });
      else if ("italic" in style) editor.setTextStyle({ italic: !t.italic });
      else if ("decoration" in style) editor.setTextStyle({ decoration: t.decoration === style.decoration ? "none" : (style.decoration as "underline") });
      return;
    }
    engine.styleTextRange(editor.editing, a, b, style, true);
    editor.refreshSelection();
    editor.invalidate();
  }

  function keydown(e: KeyboardEvent) {
    e.stopPropagation();
    if (e.key === "Escape") {
      e.preventDefault();
      editor.finishEditing();
      return;
    }
    if (e.ctrlKey || e.metaKey) {
      const area = e.target as HTMLTextAreaElement;
      const k = e.key.toLowerCase();
      const style =
        k === "b" ? { weight: 700 } : k === "i" ? { italic: true } : k === "u" ? { decoration: "underline" } : e.shiftKey && k === "x" ? { decoration: "strikethrough" } : null;
      if (style) {
        e.preventDefault();
        styleSelection(area, style);
      }
    }
  }

  // Mixed styles can't be shown by a textarea: let the canvas draw the text under a clear one.
  const rich = $derived((node?.text?.runs?.length ?? 0) > 0);
  $effect(() => {
    editor.engine?.setEditing(rich ? -1 : editor.editing);
    editor.invalidate();
  });
</script>

{#if node?.text}
  <textarea
    class="text-editor"
    class:rich
    spellcheck="false"
    style={style(node, view.zoom)}
    oninput={input}
    onkeydown={keydown}
    onblur={() => editor.finishEditing()}
    {@attach (el) => {
      // Fill once: re-assigning the value on every keystroke would move the caret.
      const area = el as HTMLTextAreaElement;
      area.value = untrack(() => node?.text?.content ?? "");
      area.focus({ preventScroll: true });
      area.select();
    }}
  ></textarea>
{/if}

<style>
  .text-editor.rich {
    color: transparent !important;
    caret-color: #0d99ff;
  }
  .text-editor {
    position: absolute;
    z-index: 5;
    margin: 0;
    padding: 0;
    border: 0;
    outline: none;
    background: transparent;
    resize: none;
    overflow: hidden;
    font-kerning: normal;
    font-optical-sizing: none;
    caret-color: var(--brand-strong);
    transform-origin: 0 0;
  }
  .text-editor::selection {
    background: rgba(13, 153, 255, 0.35);
  }
</style>
