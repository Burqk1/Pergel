<script lang="ts">
  import { onMount } from "svelte";
  import { Engine, INIT_ERRORS } from "../engine";
  import { editor, SHAPE_TOOLS, TOOL_KEYS } from "../editor.svelte";
  import Menu from "./Menu.svelte";
  import { layerMenu } from "../layerMenu";
  import TextEditor from "./TextEditor.svelte";
  import Rulers from "./Rulers.svelte";
  import Measure from "./Measure.svelte";
  import LiveCursors from "./LiveCursors.svelte";
  import FollowFrame from "./FollowFrame.svelte";
  import CommentPins from "./CommentPins.svelte";
  import { live } from "../live.svelte";
  import { runTool, startMcpBridge } from "../mcp";
  import { workspace } from "../workspace.svelte";

  type Mode = "idle" | "pan" | "move" | "marquee" | "create" | "transform" | "vector" | "pencil";

  // A curved-arrow cursor for the rotate zones outside the selection's corners.
  const ROTATE_CURSOR = `url("data:image/svg+xml,${encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M6 15a7 7 0 0 1 11-8" fill="none" stroke="white" stroke-width="4" stroke-linecap="round"/><path d="M6 15a7 7 0 0 1 11-8" fill="none" stroke="black" stroke-width="1.6" stroke-linecap="round"/><path d="M14 4l4 3-4 3z" fill="black" stroke="white" stroke-width="1"/></svg>',
  )}") 12 12, crosshair`;

  /** CSS cursor for a selection handle, turned with the selection like Figma's. */
  function handleCursor(handle: number, rotation: number) {
    if (handle >= 8) return ROTATE_CURSOR;
    const base = [135, 90, 45, 0, -45, -90, -135, 180][handle];
    const a = (((base + rotation) % 180) + 180) % 180;
    const steps = ["ew-resize", "nesw-resize", "ns-resize", "nwse-resize"];
    return steps[Math.round(a / 45) % 4];
  }

  let canvas: HTMLCanvasElement;
  let host: HTMLDivElement;
  let error = $state<string | null>(null);
  let mode: Mode = "idle";
  let stageW = $state(0);
  let measureTarget = $state<{ x: number; y: number; w: number; h: number } | null>(null);
  let stageH = $state(0);
  let space = $state(false);
  let start = { x: 0, y: 0 };
  let last = { x: 0, y: 0 };
  let moved = false;
  let lastHover = -1;
  let hoverCursor = $state<string | null>(null);
  let menu = $state({ x: 0, y: 0, open: false });
  const frameTimes: number[] = [];
  let lastFrame = 0;

  const creating = $derived(
    SHAPE_TOOLS.has(editor.tool) || editor.tool === "text",
  );
  const cursor = $derived(
    space || editor.tool === "hand"
      ? "grab"
      : editor.tool === "pen" || editor.tool === "pencil"
        ? "crosshair"
        : editor.tool === "text"
        ? "text"
        : creating
          ? "crosshair"
          : (hoverCursor ?? "default"),
  );

  function local(e: { clientX: number; clientY: number }) {
    const r = canvas.getBoundingClientRect();
    return { x: e.clientX - r.left, y: e.clientY - r.top };
  }

  function dpr() {
    return window.devicePixelRatio || 1;
  }

  function resize() {
    const engine = editor.engine;
    if (!engine) return;
    const r = host.getBoundingClientRect();
    canvas.width = Math.max(1, Math.round(r.width * dpr()));
    canvas.height = Math.max(1, Math.round(r.height * dpr()));
    engine.resize(canvas.width, canvas.height, dpr());
    editor.invalidate();
  }

  let panics = 0;

  function frame(now: number) {
    const engine = editor.engine;
    // A bug inside the engine is caught there and its change rolled back; say so.
    if (engine && engine.panics() !== panics) {
      panics = engine.panics();
      editor.notify(`Something went wrong and the last change was undone (${engine.lastPanic()})`);
      editor.refreshAll();
      editor.invalidate();
    }
    if (engine && (editor.dirty || editor.benchmarking)) {
      const t = performance.now();
      const drawn = engine.render();
      const ms = performance.now() - t;
      editor.view = engine.view();
      if (lastFrame) {
        frameTimes.push(now - lastFrame);
        if (frameTimes.length > 60) frameTimes.shift();
      }
      lastFrame = now;
      if (editor.showStats) {
        const avg = frameTimes.reduce((a, b) => a + b, 0) / Math.max(1, frameTimes.length);
        editor.stats = { fps: avg ? Math.round(1000 / avg) : 0, ms, drawn };
      }
      editor.dirty = false;
    } else {
      lastFrame = 0;
    }
    requestAnimationFrame(frame);
  }

  function onDown(e: PointerEvent) {
    const engine = editor.engine;
    if (!engine) return;
    const p = local(e);
    start = p;
    last = p;
    moved = false;
    // Clicking the canvas ends text editing before anything else happens.
    if (editor.editing >= 0) editor.finishEditing();
    canvas.setPointerCapture(e.pointerId);
    editor.interacting = true;

    if (e.button === 1 || space || editor.tool === "hand") {
      mode = "pan";
      e.preventDefault();
      return;
    }
    if (e.button === 2) {
      const id = engine.pick(p.x, p.y, e.ctrlKey || e.metaKey);
      if (id >= 0 && !engine.isSelected(id)) editor.select(id);
      return;
    }
    if (e.button !== 0) return;
    if (editor.tool === "sticky") {
      // FigJam's sticky note: drops a note and starts typing into it.
      const x = editor.view.x + p.x / editor.view.zoom;
      const y = editor.view.y + p.y / editor.view.zoom;
      const r = engine.agent("create_nodes", { nodes: [{ type: "sticky", x: x - 120, y: y - 120, text: "" }] });
      editor.tool = "move";
      editor.refreshAll();
      const made = r.ok ? (r.result as { created: { id: string }[] }).created[0] : null;
      const kid = made ? (engine.agent("list_children", { nodeId: made.id }).result as { children: string[] } | undefined)?.children[0] : undefined;
      if (kid) editor.startEditing(Number(kid), true);
      mode = "idle";
      return;
    }
    if (editor.tool === "comment") {
      // Comment tool (C): a new thread where the pointer is; it pins to the layer under it.
      editor.openThread = null;
      editor.commentDraft = { x: editor.view.x + p.x / editor.view.zoom, y: editor.view.y + p.y / editor.view.zoom };
      mode = "idle";
      return;
    }
    if (editor.tool === "pencil") {
      engine.pencilBegin(p.x, p.y);
      mode = "pencil";
      return;
    }
    if (editor.tool === "pen") {
      // Pen: each click adds an anchor; dragging pulls out curve handles.
      const r = engine.penDown(p.x, p.y);
      mode = "vector";
      editor.vectorMode = engine.vectorState();
      if (r === 2) editor.endVector();
      editor.refreshLayers();
      editor.invalidate();
      return;
    }
    if (editor.vectorMode === 1) {
      engine.vectorDown(p.x, p.y, e.shiftKey, e.altKey);
      mode = "vector";
      editor.invalidate();
      return;
    }
    if (creating) {
      mode = "create";
      return;
    }
    const handle = engine.handleAt(p.x, p.y);
    if (handle >= 0) {
      mode = "transform";
      engine.begin();
      engine.transformBegin(handle, p.x, p.y);
      return;
    }
    // Shift adds to the selection; Ctrl reaches the innermost layer (Figma's deep select).
    const id = engine.pick(p.x, p.y, e.ctrlKey || e.metaKey);
    if (id >= 0) {
      if (e.shiftKey) editor.select(id, true);
      else if (!engine.isSelected(id)) editor.select(id);
      mode = engine.isSelected(id) ? "move" : "idle";
      if (mode === "move") engine.begin();
    } else {
      if (!e.shiftKey) editor.clearSelection();
      mode = "marquee";
    }
  }

  function onMove(e: PointerEvent) {
    const engine = editor.engine;
    if (!engine) return;
    const p = local(e);
    const dx = p.x - last.x;
    const dy = p.y - last.y;
    last = p;
    if (live.status === "on") live.cursor(editor.view.x + p.x / editor.view.zoom, editor.view.y + p.y / editor.view.zoom);

    switch (mode) {
      case "pan":
        engine.pan(dx, dy);
        break;
      case "vector":
        engine.vectorMove(p.x, p.y);
        break;
      case "pencil":
        engine.pencilPoint(p.x, p.y);
        break;
      case "move":
        if (!moved && Math.hypot(p.x - start.x, p.y - start.y) < 2) return;
        // Alt-drag drags a copy and leaves the original in place; both undo as one step.
        if (!moved && e.altKey) engine.duplicate();
        if (!moved) engine.moveStart();
        // Smart guides snap to other layers; Ctrl turns snapping off, Shift locks the axis.
        engine.moveTotal(p.x - start.x, p.y - start.y, !(e.ctrlKey || e.metaKey), e.shiftKey);
        moved = true;
        break;
      case "marquee":
      case "create":
        engine.setMarquee(start.x, start.y, p.x - start.x, p.y - start.y);
        break;
      case "transform":
        engine.transformTo(p.x, p.y, e.shiftKey, e.altKey, editor.tool === "scale");
        moved = true;
        editor.refreshSelection();
        break;
      default: {
        // Alt-hover: measure from the selection to the layer under the pointer.
        if (e.altKey && editor.selection.length && mode === "idle") {
          const id = engine.pick(p.x, p.y, true);
          if (id >= 0 && !engine.isSelected(id)) {
            const r = engine.agent("get_node", { nodeId: String(id) });
            const n = r.ok ? (r.result as { absX: number; absY: number; width: number; height: number }) : null;
            measureTarget = n ? { x: n.absX, y: n.absY, w: n.width, h: n.height } : null;
          } else measureTarget = null;
        } else if (measureTarget) measureTarget = null;
        if (editor.vectorMode === 2) {
          // The pen's rubber band follows the pointer.
          engine.vectorMove(p.x, p.y);
          break;
        }
        if ((editor.tool !== "move" && editor.tool !== "scale") || space) return;
        const handle = engine.handleAt(p.x, p.y);
        hoverCursor = handle >= 0 ? handleCursor(handle, editor.view.rotation) : null;
        if (handle >= 0) {
          if (lastHover !== -1) {
            lastHover = -1;
            engine.setHover(-1);
            editor.invalidate();
          }
          return;
        }
        const id = engine.pick(p.x, p.y, e.ctrlKey || e.metaKey);
        if (id === lastHover) return;
        lastHover = id;
        engine.setHover(id);
      }
    }
    editor.invalidate();
  }

  function onUp(e: PointerEvent) {
    const engine = editor.engine;
    if (!engine) return;
    if (canvas.hasPointerCapture(e.pointerId)) canvas.releasePointerCapture(e.pointerId);
    const p = local(e);
    const w = p.x - start.x;
    const h = p.y - start.y;

    if (mode === "pencil") {
      engine.pencilEnd();
      mode = "idle";
      editor.interacting = false;
      editor.refreshAll();
      return;
    }
    if (mode === "vector") {
      engine.vectorUp();
      editor.refreshSelection();
      mode = "idle";
      editor.interacting = false;
      editor.invalidate();
      return;
    }
    if (mode === "move") {
      engine.moveEnd();
      if (moved) engine.drop(p.x, p.y);
      engine.commit();
      if (moved) editor.refreshAll();
    } else if (mode === "transform") {
      engine.transformEnd();
      engine.commit();
      editor.refreshAll();
    } else if (mode === "marquee") {
      engine.setMarquee(0, 0, 0, 0);
      if (Math.abs(w) > 2 || Math.abs(h) > 2) {
        engine.selectArea(start.x, start.y, w, h, e.shiftKey);
      }
      editor.refreshSelection();
    } else if (mode === "create" && editor.tool === "text") {
      engine.setMarquee(0, 0, 0, 0);
      engine.begin();
      const id = engine.createText(start.x, start.y, w, h);
      editor.tool = "move";
      editor.startEditing(id, true);
    } else if (mode === "create") {
      engine.setMarquee(0, 0, 0, 0);
      const kind = editor.tool as "frame" | "section" | "slice" | "rectangle" | "ellipse" | "line" | "arrow" | "polygon" | "star";
      engine.create(kind, start.x, start.y, w, h);
      editor.tool = "move";
      editor.refreshAll();
    }
    mode = "idle";
    editor.interacting = false;
    editor.invalidate();
  }

  function onCancel() {
    editor.interacting = false;
    if (mode === "transform") editor.engine?.transformEnd();
    if (mode === "move" || mode === "transform") {
      editor.engine?.commit();
      editor.refreshAll();
    }
    editor.engine?.setMarquee(0, 0, 0, 0);
    mode = "idle";
    editor.invalidate();
  }

  function onDoubleClick(e: MouseEvent) {
    const engine = editor.engine;
    if (!engine || creating) return;
    const p = local(e);
    const id = engine.dive(p.x, p.y);
    if (id < 0) return;
    const only = editor.selection.length === 1 ? editor.selection[0] : null;
    if (editor.vectorMode) return;
    // Double-clicking a selected text layer starts typing; a vector or shape edits its points.
    if (only && only.id === id && only.kind === "Text") editor.startEditing(id);
    else if (only && only.id === id && ["Vector", "Rectangle", "Ellipse"].includes(only.kind) && !only.inInstance) editor.editVector(id);
    else editor.select(id);
  }

  function onContextMenu(e: MouseEvent) {
    e.preventDefault();
    const p = local(e);
    menu = { x: p.x, y: p.y, open: true };
  }

  function onWheel(e: WheelEvent) {
    const engine = editor.engine;
    if (!engine) return;
    e.preventDefault();
    const p = local(e);
    if (e.ctrlKey || e.metaKey) engine.zoom(Math.exp(-e.deltaY * 0.01), p.x, p.y);
    else engine.pan(-(e.shiftKey ? e.deltaY : e.deltaX), -(e.shiftKey ? 0 : e.deltaY));
    editor.invalidate();
  }

  function typing(e: KeyboardEvent) {
    const t = e.target as HTMLElement;
    return t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT";
  }

  function onKeyDown(e: KeyboardEvent) {
    if (typing(e) || !editor.engine || workspace.active === "home") return;
    if (e.code === "Space") {
      space = true;
      e.preventDefault();
      return;
    }
    if (shortcut(e)) e.preventDefault();
  }

  /** Figma's shortcuts. Returns whether the key was used. */
  function shortcut(e: KeyboardEvent): boolean {
    const key = e.key.toLowerCase();
    const { shiftKey: shift, altKey: alt } = e;
    const any = editor.selection.length > 0;
    // Vector edit mode / pen: Enter or Escape finishes, Delete removes points.
    if (editor.vectorMode) {
      if (key === "escape" || key === "enter") {
        editor.endVector();
        return true;
      }
      if (key === "delete" || key === "backspace") {
        editor.engine?.vectorDelete();
        editor.invalidate();
        return true;
      }
      if (e.ctrlKey && (key === "z" || key === "y")) return true;
    }
    if (e.ctrlKey || e.metaKey) {
      if (!alt && !shift && e.code === "KeyK") editor.quickActions = true;
      else if (!alt && shift && e.code === "KeyA") editor.selectInverse();
      else if (!alt && !shift && e.code === "KeyF") editor.focusLayerSearch = true;
      else if (!alt && !shift && e.code === "KeyH") editor.findOpen = true;
      else if (!alt && !shift && e.code === "KeyB" && editor.toggleText("bold")) {}
      else if (!alt && !shift && e.code === "KeyI" && editor.toggleText("italic")) {}
      else if (!alt && !shift && e.code === "KeyU" && editor.toggleText("underline")) {}
      else if (!alt && shift && e.code === "KeyX" && editor.toggleText("strike")) {}
      else if (alt && !shift && ["KeyL", "KeyT", "KeyR", "KeyJ"].includes(e.code) && editor.selection.some((n) => n.text))
        editor.setTextStyle({ align: ({ KeyL: "left", KeyT: "center", KeyR: "right", KeyJ: "justify" } as const)[e.code as "KeyL"] });
      else if (alt && e.code === "Enter") editor.present();
      else if (alt && shift && e.code === "KeyT") editor.align("tidy");
      else if (!alt && !shift && e.code === "KeyJ") editor.join();
      else if (alt && !shift && e.code === "KeyS") editor.versionsOpen = true;
      else if (!alt && e.code === "Backslash") editor.uiHidden = !editor.uiHidden;
      else if (alt && shift && e.code === "KeyR") editor.resizeToFit();
      else if (alt && e.code === "KeyC") editor.copyProperties();
      else if (alt && e.code === "KeyV") editor.pasteProperties();
      else if (shift && e.code === "KeyC") editor.copyAsPng();
      else if (shift && e.code === "KeyR") editor.pasteReplace();
      else if (!shift && !alt && e.code === "KeyR") editor.renameSelection();
      else if (alt && e.code === "KeyK") editor.createComponent();
      else if (alt && !shift && e.code === "KeyM") editor.useAsMask();
      else if (alt && !shift && e.code === "KeyA") editor.selectMatching();
      else if (shift && e.code === "KeyK") editor.placeImageDialog();
      else if (!shift && !alt && e.code === "KeyE") editor.boolean("flatten");
      else if ((shift || alt) && e.code === "KeyO") editor.outlineStroke();
      else if (alt && e.code === "KeyB") editor.detachInstance();
      else if (key === "z" && !shift) editor.undo();
      else if (key === "z" || key === "y") editor.redo();
      else if (key === "=" || key === "+") editor.zoomBy(2);
      else if (key === "-") editor.zoomBy(0.5);
      else if (key === "a" && !shift) editor.selectAll();
      else if (key === "d") editor.duplicate();
      else if (e.code === "KeyG" && alt) editor.group(true);
      else if (e.code === "KeyG" && shift) editor.ungroup();
      else if (e.code === "KeyG") editor.group();
      else if (key === "backspace") editor.ungroup();
      else if (e.code === "KeyH" && shift) editor.toggleFlag(-1, "visible");
      else if (e.code === "KeyL" && shift) editor.toggleFlag(-1, "locked");
      else if (e.code === "BracketRight") editor.reorder("forward");
      else if (e.code === "BracketLeft") editor.reorder("backward");
      else return false;
      return true;
    }
    if (alt && any && !shift) {
      const ops: Record<string, "left" | "centerX" | "right" | "top" | "centerY" | "bottom"> = {
        KeyA: "left",
        KeyH: "centerX",
        KeyD: "right",
        KeyW: "top",
        KeyV: "centerY",
        KeyS: "bottom",
      };
      if (ops[e.code]) {
        editor.align(ops[e.code]);
        return true;
      }
      if (e.code === "Slash") {
        editor.patch({ fillOn: false });
        return true;
      }
    }
    if (shift && any && e.code === "KeyA") {
      editor.autoLayout(!alt);
      return true;
    }
    if (alt && shift && any && e.code === "KeyF") {
      editor.boolean("flatten");
      return true;
    }
    if (alt && shift && any && (e.code === "KeyH" || e.code === "KeyV")) {
      editor.align(e.code === "KeyH" ? "distributeX" : "distributeY");
      return true;
    }
    if (shift && !alt && any && e.code === "KeyH") editor.flip(true);
    else if (shift && !alt && any && e.code === "KeyV") editor.flip(false);
    else if (shift && !alt && any && e.code === "KeyX") editor.swapFillStroke();
    else if (shift && !alt && any && e.code === "Slash") editor.patch({ strokeOn: false });
    else if (!shift && !alt && any && /^Digit[0-9]$/.test(e.code)) {
      // Figma: 1 = 10% opacity … 9 = 90%, 0 = 100%.
      const d = Number(e.code.slice(5));
      editor.patch({ opacity: d === 0 ? 100 : d * 10 });
    } else if (shift && e.code === "Digit0") editor.zoomTo(1);
    else if (shift && e.code === "Digit1") editor.fit();
    else if (shift && e.code === "Digit2") editor.fitSelection();
    else if (shift && key === "d") editor.devMode = !editor.devMode;
    else if (!alt && !e.ctrlKey && e.code === "KeyN") editor.nextFrame(shift ? -1 : 1);
    else if (shift && !alt && e.code === "KeyG") editor.toggleGrids();
    else if (shift && !alt && e.code === "KeyR") editor.showRulers = !editor.showRulers;
    else if (!alt && !shift && e.code === "KeyI") void editor.pickColor();
    else if (e.code === "PageDown") editor.stepPage(1);
    else if (e.code === "PageUp") editor.stepPage(-1);
    else if (e.code === "BracketRight" && any) editor.reorder("front");
    else if (e.code === "BracketLeft" && any) editor.reorder("back");
    else if (key === "enter" && !shift && editor.selection.length === 1 && editor.selection[0].kind === "Text")
      editor.startEditing(editor.selection[0].id);
    else if (key === "enter" && !shift && editor.selection.length === 1 && editor.selection[0].kind === "Vector") editor.editVector();
    else if (key === "enter" && any) editor.selectRelative(shift ? "parent" : "children");
    else if (key === "tab" && any) editor.selectRelative(shift ? "previous" : "next");
    else if ((key === "delete" || key === "backspace") && any) editor.deleteSelection();
    else if (key === "escape") {
      if (menu.open) menu.open = false;
      else if (editor.tool !== "move") editor.tool = "move";
      else editor.clearSelection();
    } else if (key.startsWith("arrow") && any) {
      const step = shift ? 10 : 1;
      const dx = key === "arrowleft" ? -step : key === "arrowright" ? step : 0;
      const dy = key === "arrowup" ? -step : key === "arrowdown" ? step : 0;
      editor.engine!.nudge(dx, dy);
      editor.refreshSelection();
      editor.invalidate();
    } else if (alt && e.code === "KeyL") editor.collapseAll();
    else if (!alt && shift && e.code === "KeyL") editor.setTool("arrow");
    else if (!alt && shift && e.code === "KeyP") editor.setTool("pencil");
    else if (!alt && shift && e.code === "KeyS") editor.setTool("section");
    else if (!alt && !shift && TOOL_KEYS[key]) editor.setTool(TOOL_KEYS[key]);
    else return false;
    return true;
  }

  function onKeyUp(e: KeyboardEvent) {
    if (e.code === "Space") space = false;
    if (e.key === "Alt") measureTarget = null;
  }

  function onClipboard(e: ClipboardEvent, action: "copy" | "cut" | "paste") {
    const t = e.target as HTMLElement;
    if (t.isContentEditable || t.tagName === "INPUT" || t.tagName === "TEXTAREA" || !editor.engine) return;
    if (action === "paste") editor.paste(e);
    else editor.copy(e, action === "cut");
  }

  function onDragOver(e: DragEvent) {
    if (e.dataTransfer?.types.includes("Files")) {
      e.preventDefault();
      e.dataTransfer.dropEffect = "copy";
    }
  }

  function onDrop(e: DragEvent) {
    const files = [...(e.dataTransfer?.files ?? [])];
    if (!files.length) return;
    e.preventDefault();
    const r = canvas.getBoundingClientRect();
    void editor.importFiles(files, e.clientX - r.left, e.clientY - r.top);
  }

  onMount(() => {
    let observer: ResizeObserver | undefined;
    (async () => {
      try {
        const engine = await Engine.load(canvas);
        const r = host.getBoundingClientRect();
        canvas.width = Math.round(r.width * dpr());
        canvas.height = Math.round(r.height * dpr());
        const status = engine.init(canvas.width, canvas.height, dpr());
        if (status !== 1) {
          error = INIT_ERRORS[status] ?? `Engine failed to start (${status})`;
          return;
        }
        editor.engine = engine;
        editor.refreshAll();
        // Fonts first, so restored documents measure their text with the right font.
        editor.loadFonts().then(() => workspace.init());
        startMcpBridge();
        // Dev builds: reach the editor from the console (and from automated checks).
        if (import.meta.env.DEV) {
          const w = window as unknown as { __pergel: typeof editor; __pergelTool: typeof runTool; __live: typeof live };
          w.__pergel = editor;
          w.__pergelTool = runTool;
          w.__live = live;
          (window as unknown as Record<string, unknown>).__workspace = workspace;
          void import("../libraries.svelte").then((m) => ((window as unknown as Record<string, unknown>).__libraries = m.libraries));
        }
        observer = new ResizeObserver(resize);
        observer.observe(host);
        requestAnimationFrame(frame);
      } catch (e) {
        error = `Engine could not be loaded: ${e}. Run "npm run wasm" first.`;
      }
    })();
    return () => observer?.disconnect();
  });

  const size = (v: number) => Math.round(v * 100) / 100;
</script>

<svelte:window
  onkeydown={onKeyDown}
  onkeyup={onKeyUp}
  onblur={() => (space = false)}
  oncopy={(e) => onClipboard(e, "copy")}
  oncut={(e) => onClipboard(e, "cut")}
  onpaste={(e) => onClipboard(e, "paste")}
/>

<div class="stage" bind:this={host} bind:clientWidth={stageW} bind:clientHeight={stageH} style="cursor:{cursor}" role="application" ondragover={onDragOver} ondrop={onDrop}>
  {#if editor.showRulers}<Rulers width={stageW} height={stageH} />{/if}
  <Measure target={measureTarget} />
  <LiveCursors />
  <FollowFrame />
  <CommentPins />
  <canvas
    id="pergel-canvas"
    bind:this={canvas}
    onpointerdown={onDown}
    onpointermove={onMove}
    onpointerup={onUp}
    onpointercancel={onCancel}
    ondblclick={onDoubleClick}
    onpointerleave={() => {
      if (mode === "idle" && lastHover !== -1) {
        lastHover = -1;
        editor.engine?.setHover(-1);
        editor.invalidate();
      }
    }}
    onwheel={onWheel}
    oncontextmenu={onContextMenu}
  ></canvas>

  <TextEditor />

  <div class="context-anchor" style="left:{menu.x}px; top:{menu.y}px">
    <Menu items={menu.open ? layerMenu({ at: { x: menu.x, y: menu.y } }) : []} bind:open={menu.open} />
  </div>

  {#each editor.view.frames as f (f.id)}
    {#if f.w > 24}
      <button
        class="frame-label"
        class:selected={editor.selectedIds.has(f.id)}
        class:component={f.component}
        class:section={f.section}
        style="left:{f.section ? f.x + 6 : f.x}px; top:{f.section ? f.y + 6 : f.y - 20}px; max-width:{f.section ? f.w - 12 : f.w}px"
        onpointerdown={(e) => {
          e.stopPropagation();
          editor.select(f.id, e.shiftKey);
        }}
      >
        {#if f.ready}<span class="ready" title="Ready for dev">&lt;/&gt;</span>{/if}{f.name}
      </button>
    {/if}
  {/each}

  {#if editor.view.selection}
    {@const s = editor.view.selection}
    <div class="size-badge" style="left:{s.x + s.w / 2}px; top:{s.y + s.h + 8}px">
      {size(s.width)} × {size(s.height)}
    </div>
  {/if}

  {#if editor.showStats}
    <div class="stats">
      <span>{editor.stats.fps} fps</span><span>{editor.stats.ms.toFixed(2)} ms</span>
      <span>{editor.stats.drawn.toLocaleString("en-US")} drawn</span>
    </div>
  {/if}

  {#if editor.benchmark}
    {@const b = editor.benchmark}
    <div class="dialog">
      <b>Benchmark · {b.count.toLocaleString("en-US")} layers</b>
      <span>Average: <b>{b.avg} fps</b></span>
      <span>95th percentile frame: {b.p95.toFixed(1)} ms</span>
      <span>Worst frame: {b.worst.toFixed(1)} ms</span>
      <span class="faint">{b.frames} frames over 6 s of continuous pan + zoom</span>
      <button onclick={() => (editor.benchmark = null)}>Close</button>
    </div>
  {:else if editor.benchmarking}
    <div class="dialog"><b>Running benchmark…</b></div>
  {/if}

  {#if error}<div class="dialog error">{error}</div>{/if}
  {#if editor.toast}<div class="toast" class:raised={editor.selection.length > 0}>{editor.toast}</div>{/if}
</div>

<style>
  .ready {
    display: inline-block;
    margin-right: 4px;
    padding: 0 3px;
    border-radius: 3px;
    background: #14ae5c;
    color: #fff;
    font-size: 9px;
    font-weight: 700;
    line-height: 14px;
  }
  .stage {
    position: relative;
    flex: 1;
    min-width: 0;
    /* clip, not hidden: a focused text overlay must never scroll the canvas */
    overflow: clip;
  }
  canvas {
    display: block;
    width: 100%;
    height: 100%;
    touch-action: none;
  }
  .context-anchor {
    position: absolute;
    width: 0;
    height: 0;
  }
  .context-anchor :global(.menu.below) {
    top: 0;
  }
  .frame-label {
    position: absolute;
    height: 18px;
    line-height: 18px;
    padding: 0 2px;
    margin-left: -2px;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
  }
  .frame-label:hover {
    color: var(--text);
  }
  .frame-label.selected {
    color: var(--text-brand);
  }
  .frame-label.section {
    height: 22px;
    line-height: 22px;
    padding: 0 8px;
    margin-left: 0;
    border-radius: 5px;
    background: var(--bg, #fff);
    color: var(--text);
    box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.1);
    font-weight: 500;
  }
  .frame-label.component {
    color: var(--component, #9747ff);
  }
  .frame-label.component::before {
    content: "❖ ";
  }
  .size-badge {
    position: absolute;
    transform: translateX(-50%);
    padding: 0 4px;
    height: 16px;
    line-height: 16px;
    border-radius: 3px;
    background: var(--brand-strong);
    color: #fff;
    font-size: 10px;
    font-weight: 500;
    white-space: nowrap;
    pointer-events: none;
    font-variant-numeric: tabular-nums;
  }
  .stats {
    position: absolute;
    top: 8px;
    right: 8px;
    display: flex;
    gap: 10px;
    padding: 4px 8px;
    background: var(--menu-bg);
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
    pointer-events: none;
  }
  .dialog {
    position: absolute;
    top: 16px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 14px 18px;
    background: var(--bg);
    border-radius: 13px;
    box-shadow: var(--shadow-menu);
  }
  .dialog button {
    align-self: flex-end;
    margin-top: 6px;
    height: 24px;
    padding: 0 10px;
    border-radius: var(--radius);
    background: var(--bg-secondary);
  }
  .dialog.error {
    color: #ff8a73;
  }
  .toast {
    position: absolute;
    left: 50%;
    bottom: 76px;
    transform: translateX(-50%);
    padding: 8px 12px;
    background: var(--menu-bg);
    border-radius: 8px;
    box-shadow: var(--shadow-menu);
    pointer-events: none;
    z-index: 11;
  }
  /* Above the selection's contextual toolbar. */
  .toast.raised {
    bottom: 124px;
  }
</style>
