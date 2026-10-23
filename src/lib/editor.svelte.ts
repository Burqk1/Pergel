import { ensureFonts, loadFamily, systemFonts, type SystemFamily } from "./fonts";
import type { ALIGN, BindProp, StyleKind, Styles, ComponentInfo, Engine, FontFamily, LayerInfo, NodeInfo, Patch, TextStyle, VarValue, Variables, ViewState } from "./engine";

const CLIPBOARD_MIME = "web application/x-pergel";

export type Tool = "move" | "scale" | "hand" | "frame" | "section" | "slice" | "sticky" | "rectangle" | "ellipse" | "line" | "arrow" | "polygon" | "star" | "pen" | "pencil" | "text" | "comment";

/** Tools that draw a shape by dragging a box. */
export const SHAPE_TOOLS: ReadonlySet<Tool> = new Set(["frame", "section", "slice", "rectangle", "ellipse", "line", "arrow", "polygon", "star"]);

export const TOOL_KEYS: Record<string, Tool> = {
  v: "move",
  k: "scale",
  h: "hand",
  f: "frame",
  a: "frame",
  r: "rectangle",
  o: "ellipse",
  l: "line",
  p: "pen",
  t: "text",
  s: "slice",
  c: "comment",
};

export const UNAVAILABLE: ReadonlySet<Tool> = new Set([]);

/** Fonts that ship with Pergel, registered under their family name. */
const BUNDLED_FONTS = [
  { family: "Inter", url: "/fonts/InterVariable.ttf" },
  { family: "Inter", url: "/fonts/InterVariable-Italic.ttf" },
];

class Editor {
  engine: Engine | null = $state.raw(null);
  tool = $state<Tool>("move");
  selection = $state<NodeInfo[]>([]);
  layers = $state.raw<LayerInfo[]>([]);
  pages = $state.raw<{ name: string; current: boolean }[]>([]);
  systemFamilies = $state.raw<SystemFamily[]>([]);
  /** The Comment tool asked for an annotation on the selected layer. */
  annotating = $state(false);
  /** Rulers visible (Shift+R). */
  showRulers = $state(false);
  /** Layout guides visible (Shift+G). */
  showGrids = $state(true);
  toggleGrids() {
    this.showGrids = !this.showGrids;
    this.engine?.showGrids(this.showGrids);
    this.invalidate();
  }
  /** Find and replace dialog open. */
  findOpen = $state(false);
  /** Version history panel open. */
  versionsOpen = $state(false);
  /** Presentation view open, starting at this screen. */
  presenting = $state<number | null>(null);

  /** Present (▷): plays the prototype from the selected screen (or the first one). */
  present() {
    const engine = this.engine;
    if (!engine) return;
    const sel = this.selection[0];
    const fromSel = sel ? engine.screenOf(sel.id) : -1;
    const start = fromSel >= 0 ? fromSel : (engine.screens()[0]?.id ?? -1);
    if (start < 0) return this.notify("Add a frame to present");
    this.presenting = start;
  }

  /** Ctrl+F asks the layers panel to open its search. */
  focusLayerSearch = $state(false);
  /** Quick actions (Ctrl+K) palette. */
  quickActions = $state(false);
  /** Ctrl+\ hides the panels and toolbars. */
  uiHidden = $state(false);
  private fontsBusy = false;
  /** The "Import from Figma" dialog is open. */
  figmaImport = $state(false);
  /** A Figma link pasted or dropped on the canvas, for the import dialog. */
  figmaImportUrl = $state("");
  /** The local variables editor is open. */
  variablesOpen = $state(false);
  variables = $state.raw<Variables>({ collections: [], variables: [] });
  styles = $state.raw<Styles>({ list: [] });
  /** Page being renamed in the pages list. */
  renamingPage = $state(-1);
  view = $state.raw<ViewState>({ zoom: 1, x: 0, y: 0, rotation: 0, frames: [], selection: null, undo: false, redo: false });
  expanded = $state.raw(new Set<number>());
  parentOf = new Map<number, number>();
  clipboard = "";
  fonts = $state.raw<FontFamily[]>([]);
  /** Built-in MCP server (desktop app). */
  mcp = $state.raw<{ url: string; running: boolean; error: string | null } | null>(null);
  components = $state.raw<(ComponentInfo & { thumbnail: string | null })[]>([]);
  /** The text layer being typed into (-1 when none). */
  editing = $state(-1);
  /** Whether the editing session created the layer (an empty new layer is discarded). */
  private editingCreated = false;
  hover = $state(-1);
  fileName = $state("Untitled");
  leftTab = $state<"file" | "assets" | "agents" | "tools">("file");
  /** What MCP agents did in this session (Agents panel). */
  mcpLog = $state.raw<{ time: number; tool: string; ok: boolean; detail: string }[]>([]);
  rightTab = $state<"design" | "prototype">("design");
  showStats = $state(false);
  devMode = $state(false);
  toast = $state<string | null>(null);
  stats = $state({ fps: 0, ms: 0, drawn: 0 });
  dirty = true;
  /** Layer whose name is being edited in the layers panel (Ctrl+R), or -1. */
  renaming = $state(-1);
  /** Styles copied with Copy properties (Ctrl+Alt+C). */
  private properties: { patch: Patch; text: TextStyle | null } | null = null;
  /** A drag or other pointer interaction is in progress (autosave waits). */
  interacting = false;

  get selectedIds() {
    return new Set(this.selection.map((n) => n.id));
  }

  invalidate() {
    this.dirty = true;
  }

  refreshSelection() {
    if (!this.engine) return;
    this.selection = this.engine.selection();
    this.revealSelection();
  }

  refreshLayers() {
    if (!this.engine) return;
    this.pages = this.engine.pages();
    this.variables = this.engine.variables();
    this.styles = this.engine.styles();
    this.layers = this.engine.layers();
    this.parentOf = new Map(this.layers.map((l) => [l.id, l.parent]));
  }

  /** Expands the layer tree down to every selected layer, as Figma does. */
  revealSelection() {
    let next: Set<number> | null = null;
    for (const n of this.selection) {
      for (let p = n.parent; p >= 0; p = this.parentOf.get(p) ?? -1) {
        if (!(next ?? this.expanded).has(p)) (next ??= new Set(this.expanded)).add(p);
      }
    }
    if (next) this.expanded = next;
  }

  toggleExpanded(id: number, deep = false) {
    const next = new Set(this.expanded);
    const open = !next.has(id);
    const ids = deep ? this.descendants(id) : [id];
    for (const i of ids) open ? next.add(i) : next.delete(i);
    this.expanded = next;
  }

  collapseAll() {
    this.expanded = new Set();
  }

  private descendants(id: number) {
    const out = [id];
    for (const l of this.layers) {
      for (let p = l.parent; p >= 0; p = this.parentOf.get(p) ?? -1) {
        if (p === id) {
          out.push(l.id);
          break;
        }
      }
    }
    return out;
  }

  async loadFonts() {
    await Promise.all(
      BUNDLED_FONTS.map(async (f) => {
        const res = await fetch(f.url);
        if (res.ok) this.engine?.addFont(new Uint8Array(await res.arrayBuffer()), f.family);
      }),
    );
    this.fonts = this.engine?.fonts() ?? [];
    this.refreshAll();
    void systemFonts().then((list) => (this.systemFamilies = list));
  }

  /** Loads the fonts the open document uses (installed ones, else from Fontsource). */
  async ensureDocFonts() {
    const engine = this.engine;
    if (!engine || this.fontsBusy) return;
    this.fontsBusy = true;
    try {
      const { loaded, missing } = await ensureFonts(engine);
      if (loaded.length) {
        this.fonts = engine.fonts();
        this.refreshAll();
      }
      if (missing.length) this.notify(`Missing fonts (shown in Inter): ${missing.join(", ")}`);
    } finally {
      this.fontsBusy = false;
    }
  }

  /** Picks a font family that may not be loaded yet (font menu). */
  async useFamily(family: string, weight: number, italic: boolean) {
    const engine = this.engine;
    if (!engine) return;
    if (!(await loadFamily(engine, family))) return this.notify(`${family} could not be loaded`);
    this.fonts = engine.fonts();
    const f = this.fonts.find((x) => x.name.toLowerCase() === family.toLowerCase());
    const w = f ? Math.min(f.weights[1], Math.max(f.weights[0], weight)) : weight;
    this.setTextStyle({ family: f?.name ?? family, weight: w, italic: italic && (f?.italic ?? false) });
  }

  /** Starts typing into a text layer; the whole session is one undo step. */
  startEditing(id: number, created = false) {
    const engine = this.engine;
    if (!engine || this.editing === id) return;
    if (this.editing >= 0) this.finishEditing();
    if (!created) engine.begin();
    this.editingCreated = created;
    engine.select(id);
    engine.setEditing(id);
    this.editing = id;
    this.refreshAll();
  }

  finishEditing() {
    const engine = this.engine;
    const id = this.editing;
    if (!engine || id < 0) return;
    this.editing = -1;
    engine.setEditing(-1);
    const node = this.selection.find((n) => n.id === id) ?? engine.selection().find((n) => n.id === id);
    // Figma drops text layers left empty.
    if (node?.text && !node.text.content.trim()) {
      engine.select(id);
      engine.deleteSelection();
    }
    engine.commit();
    this.refreshAll();
  }

  /** Changes typography of the selected text layers. */
  setTextStyle(change: Partial<TextStyle>) {
    const first = this.selection.find((n) => n.text)?.text;
    if (!first || !this.engine) return;
    const { weightName: _w, linePx: _l, textHeight: _t, ...style } = first;
    this.engine.setTextStyle(-1, { ...style, ...change });
    this.refreshAll();
  }

  /** Runs an engine command that may change structure and selection, then syncs the UI. */
  run(command: (engine: Engine) => unknown) {
    if (!this.engine) return;
    command(this.engine);
    this.refreshAll();
  }

  undo() {
    if (this.engine && !this.engine.undo()) this.notify("Nothing to undo");
    this.refreshAll();
  }

  redo() {
    if (this.engine && !this.engine.redo()) this.notify("Nothing to redo");
    this.refreshAll();
  }

  group(frame = false) {
    if (!this.selection.length) return;
    this.run((e) => e.group(frame));
  }

  ungroup() {
    this.run((e) => e.ungroup());
  }

  duplicate() {
    if (!this.selection.length) return;
    this.run((e) => e.duplicate());
  }

  reorder(op: "forward" | "backward" | "front" | "back") {
    if (!this.selection.length) return;
    this.run((e) => e.reorder(op));
  }

  toggleFlag(id: number, flag: "visible" | "locked" | "clip", value?: boolean) {
    this.run((e) => e.setFlag(id, flag, value));
  }

  selectRelative(which: "parent" | "children" | "next" | "previous" | "siblings") {
    this.run((e) => e.selectRelative(which));
  }

  /** Copies to the system clipboard (when the event allows it) and to an in-app fallback. */
  copy(event?: ClipboardEvent, cut = false) {
    const text = this.engine?.copy();
    if (!text) return false;
    this.clipboard = text;
    if (event?.clipboardData) {
      event.clipboardData.setData("text/plain", text);
      event.clipboardData.setData(CLIPBOARD_MIME, text);
      event.preventDefault();
    } else {
      navigator.clipboard?.writeText(text).catch(() => {});
    }
    if (cut) this.deleteSelection();
    return true;
  }

  paste(event?: ClipboardEvent) {
    const data = event?.clipboardData;
    // Image files and SVG markup paste as layers, as in Figma.
    const files = data ? [...data.files].filter((f) => f.type.startsWith("image/")) : [];
    if (files.length) {
      event?.preventDefault();
      void this.importFiles(files);
      return true;
    }
    const plain = data?.getData("text/plain") ?? "";
    // A Figma link (a file or one frame of it): import it, as the shared design.
    const link = plain.trim().match(/^https?:\/\/(www\.)?figma\.com\/(design|file|proto|board)\/[^\s]+$/i)?.[0];
    if (link && !data?.getData(CLIPBOARD_MIME)) {
      event?.preventDefault();
      this.figmaImportUrl = link;
      this.figmaImport = true;
      return true;
    }
    if (!data?.getData(CLIPBOARD_MIME) && /^\s*(<\?xml[^>]*>\s*)?<svg[\s>]/i.test(plain)) {
      event?.preventDefault();
      return this.importSvg("SVG", plain) >= 0;
    }
    const fromEvent = data?.getData(CLIPBOARD_MIME) || plain;
    const pasted = this.engine?.paste(fromEvent || this.clipboard) ?? 0;
    if (pasted > 0) event?.preventDefault();
    this.refreshAll();
    return pasted > 0;
  }

  /** Styles: link, create, edit (linked layers follow). */
  linkStyle(kind: StyleKind, style: string | null) {
    if (!this.engine?.linkStyle(kind, style)) this.notify("Select layers to apply the style to");
    this.refreshAll();
  }

  createStyle(kind: StyleKind, name: string) {
    if (!name.trim()) return;
    if (!this.engine?.createStyle(kind, name)) this.notify("Select a layer to make the style from");
    this.refreshAll();
  }

  setStyles(s: Styles) {
    this.engine?.setStyles(s);
    this.refreshAll();
  }

  /** Saves an edited copy of the variables; bound layers update (one undo step). */
  setVariables(v: Variables) {
    this.engine?.setVariables(v);
    this.refreshAll();
  }

  bindVariable(prop: BindProp, variable: string | null) {
    if (!this.engine?.bind(prop, variable)) this.notify("Select layers to bind");
    this.refreshAll();
  }

  /** A variable's value in its collection's active mode, following aliases. */
  resolveVariable(id: string, depth = 0): VarValue | null {
    const v = this.variables.variables.find((x) => x.id === id);
    const c = v && this.variables.collections.find((x) => x.id === v.collection);
    if (!v || !c || depth > 16) return null;
    const val = v.values[c.active] ?? v.values[c.modes[0]?.id ?? ""];
    if (!val) return null;
    return val.kind === "alias" ? this.resolveVariable(val.value, depth + 1) : val;
  }

  /** Boolean operations and Flatten (Ctrl+E). */
  boolean(op: "union" | "subtract" | "intersect" | "exclude" | "flatten") {
    const id = this.engine?.boolean(op) ?? -1;
    if (id < 0) this.notify(op === "flatten" ? "Select shapes to flatten" : "Select two or more shapes");
    this.refreshAll();
  }

  /** Vector edit mode: 0 off, 1 editing points, 2 drawing with the pen. */
  vectorMode = $state(0);

  /** Enter (or double-click): edit the selected vector's points. */
  editVector(id = -1) {
    if (!this.engine?.vectorBegin(id)) return false;
    this.vectorMode = this.engine.vectorState();
    this.refreshAll();
    return true;
  }

  /** Leaves vector edit mode / finishes the pen path. */
  endVector() {
    if (!this.engine || !this.engine.vectorState()) return;
    this.engine.vectorEnd();
    this.vectorMode = 0;
    if (this.tool === "pen") this.tool = "move";
    this.refreshAll();
  }

  /** Code Connect mapping for a main component / set. */
  setCodeConnect(id: number, code: { component: string; import: string; source: string; props: Record<string, Record<string, string>>; text: Record<string, string> }) {
    const r = this.engine?.agent("add_code_connect_map", { nodeId: String(id), ...code });
    if (r && !r.ok) this.notify(r.error ?? "Could not save the mapping");
    this.refreshAll();
  }

  selectInverse() {
    this.run((e) => e.selectInverse());
  }

  roundToPixel() {
    this.run((e) => e.roundToPixel());
  }

  nextFrame(dir: 1 | -1) {
    this.run((e) => e.nextFrame(dir));
  }

  /** Previous/next page (PgUp / PgDn). */
  stepPage(dir: 1 | -1) {
    const i = this.pages.findIndex((p) => p.current);
    const next = i + dir;
    if (next >= 0 && next < this.pages.length) this.switchPage(next);
  }

  /** Ctrl+B / Ctrl+I / Ctrl+U on text layers. */
  toggleText(what: "bold" | "italic" | "underline" | "strike") {
    const t = this.selection.find((n) => n.text)?.text;
    if (!t) return false;
    if (what === "bold") this.setTextStyle({ weight: t.weight >= 600 ? 400 : 700 });
    else if (what === "italic") this.setTextStyle({ italic: !t.italic });
    else if (what === "underline") this.setTextStyle({ decoration: t.decoration === "underline" ? "none" : "underline" });
    else this.setTextStyle({ decoration: t.decoration === "strikethrough" ? "none" : "strikethrough" });
    return true;
  }

  /** "Pick color" (I): samples the screen and fills the selection with it. */
  async pickColor() {
    const Dropper = (window as unknown as { EyeDropper?: new () => { open(): Promise<{ sRGBHex: string }> } }).EyeDropper;
    if (!Dropper) return this.notify("Color picking needs a newer webview");
    try {
      const { sRGBHex } = await new Dropper().open();
      if (this.selection.length) this.patch({ fill: sRGBHex.toUpperCase(), fillOpacity: 100, fillOn: true });
      else await navigator.clipboard?.writeText(sRGBHex.toUpperCase());
      this.notify(`${sRGBHex.toUpperCase()}${this.selection.length ? "" : " copied"}`);
    } catch {
      // cancelled
    }
  }

  /** Combine as variants. */
  combineVariants() {
    if ((this.engine?.combineVariants() ?? -1) < 0) this.notify("Select main components to combine");
    this.refreshAll();
    if (this.leftTab === "assets") this.refreshComponents();
  }

  setVariant(property: string, value: string) {
    this.engine?.setVariant(property, value);
    this.refreshAll();
  }

  /** Select matching layers (Ctrl+Alt+A). */
  selectMatching() {
    const n = this.engine?.selectMatching() ?? 0;
    this.refreshSelection();
    this.revealSelection();
    this.invalidate();
    if (n > 1) this.notify(`${n} matching layers selected`);
  }

  /** Use as mask (Ctrl+Alt+M). */
  useAsMask() {
    if (!this.engine?.useAsMask()) this.notify("Select layers to mask");
    this.refreshAll();
  }

  /** Branch dialog open. */
  branchOpen = $state(false);
  /** Offset path dialog open. */
  offsetOpen = $state(false);

  offsetPath(d: number, join: number) {
    if (!this.engine?.offsetPath(d, join)) this.notify("Select shapes or vectors to offset");
    this.refreshAll();
  }

  textOnPath() {
    const r = this.engine?.textOnPath() ?? 0;
    if (!r) this.notify("Select a text layer and a vector path");
    else this.notify(r === 1 ? "Text placed on path" : "Text detached from path");
    this.refreshAll();
  }

  join() {
    if (!this.engine?.join()) this.notify("Select vectors with open paths to join");
    this.refreshAll();
  }

  simplify(tolerance = 1.5) {
    const n = this.engine?.simplify(tolerance) ?? 0;
    this.notify(n ? `Simplified ${n} vector${n === 1 ? "" : "s"}` : "Select vectors to simplify");
    this.refreshAll();
  }

  /** Outline stroke (Ctrl+Shift+O). */
  outlineStroke() {
    if (!this.engine?.outlineStroke()) this.notify("Select layers with a stroke");
    this.refreshAll();
  }

  /** Pages: switch, add, rename, duplicate, delete, move (Figma's Pages list). */
  switchPage(i: number) {
    if (this.editing >= 0) this.finishEditing();
    if (this.engine?.pageSwitch(i)) {
      this.expanded = new Set();
      this.refreshAll();
      if (this.leftTab === "assets") this.refreshComponents();
    }
  }

  addPage() {
    if (this.editing >= 0) this.finishEditing();
    const i = this.engine?.pageAdd() ?? -1;
    this.refreshAll();
    if (i >= 0) this.renamingPage = i;
  }

  renamePage(i: number, name: string) {
    this.renamingPage = -1;
    if (name.trim()) this.engine?.pageRename(i, name.trim());
    this.refreshLayers();
  }

  duplicatePage(i: number) {
    this.engine?.pageDuplicate(i);
    this.refreshAll();
  }

  deletePage(i: number) {
    if (this.pages.length <= 1) return this.notify("A file needs at least one page");
    this.engine?.pageDelete(i);
    this.refreshAll();
  }

  movePage(from: number, to: number) {
    if (this.engine?.pageMove(from, to)) this.refreshLayers();
  }

  /** SVG markup as vector layers at a canvas point (default: the view's centre). */
  importSvg(name: string, svg: string, x = NaN, y = NaN) {
    const id = this.engine?.importSvg(name, svg, x, y) ?? -1;
    if (id < 0) this.notify("That SVG could not be read");
    this.refreshAll();
    return id;
  }

  /** Image and SVG files dropped, pasted or placed: each becomes a layer, side by side. */
  async importFiles(files: File[], x = NaN, y = NaN) {
    let placed = 0;
    let offset = 0;
    for (const file of files) {
      const name = file.name.replace(/\.[^.]+$/, "") || "Image";
      const isSvg = file.type === "image/svg+xml" || /\.svg$/i.test(file.name);
      const at = Number.isNaN(x) ? x : x + offset;
      const id = isSvg
        ? (this.engine?.importSvg(name, await file.text(), at, y) ?? -1)
        : (this.engine?.placeImage(name, new Uint8Array(await file.arrayBuffer()), at, y) ?? -1);
      if (id >= 0) {
        placed++;
        offset += 24;
      }
    }
    if (!placed) this.notify(files.length ? "Pergel can place PNG, JPEG, GIF, WebP and SVG files" : "Nothing to place");
    this.refreshAll();
    return placed;
  }

  private imageUrls = new Map<string, string>();

  /** An object URL for an image fill's file (cached per image). */
  imageUrl(hash: string): string | null {
    const cached = this.imageUrls.get(hash);
    if (cached) return cached;
    const bytes = this.engine?.imageBytes(hash);
    if (!bytes) return null;
    const url = URL.createObjectURL(new Blob([bytes.slice().buffer]));
    this.imageUrls.set(hash, url);
    return url;
  }

  /** Figma's "Place image" (Ctrl+Shift+K): pick files and place them. */
  placeImageDialog() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/gif,image/webp,image/svg+xml,.svg";
    input.multiple = true;
    input.onchange = () => {
      if (input.files?.length) void this.importFiles([...input.files]);
    };
    input.click();
  }

  /** Replaces the image in the selected layers' fills with a picked file. */
  replaceImageDialog() {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = "image/png,image/jpeg,image/gif,image/webp";
    input.onchange = async () => {
      const file = input.files?.[0];
      if (!file || !this.engine) return;
      if (!this.engine.setImage(new Uint8Array(await file.arrayBuffer()))) this.notify("That file is not an image Pergel can show");
      this.refreshAll();
    };
    input.click();
  }

  refreshAll() {
    this.refreshLayers();
    this.refreshSelection();
    this.refreshComments();
    this.invalidate();
  }

  /** Comment threads on the shown page, and the one being written or read. */
  comments = $state.raw<import("./engine").CommentThread[]>([]);
  commentDraft = $state<{ x: number; y: number } | null>(null);
  openThread = $state<string | null>(null);
  showResolved = $state(false);

  refreshComments() {
    this.comments = this.engine?.comments() ?? [];
  }

  setTool(tool: Tool) {
    if (this.vectorMode && tool !== "pen") this.endVector();
    if (UNAVAILABLE.has(tool)) {
      this.notify(`${tool[0].toUpperCase()}${tool.slice(1)} tool is coming soon`);
      return;
    }
    this.tool = tool;
  }

  select(id: number, additive = false) {
    this.engine?.select(id, additive);
    this.refreshSelection();
    this.invalidate();
  }

  /** Selects a layer, opens its parents in the layers panel and zooms to it. */
  selectAndReveal(id: number) {
    this.select(id);
    this.revealSelection();
    this.fitSelection();
  }

  clearSelection() {
    this.select(-1);
  }

  deleteSelection() {
    if (!this.selection.length) return;
    this.run((e) => e.deleteSelection());
  }

  /** Changes properties of the selection (or of layer `id`); one undo step. */
  patch(change: Patch, id = -1) {
    if (!this.engine) return;
    this.engine.patch(change, id);
    this.refreshAll();
  }

  align(op: keyof typeof ALIGN) {
    this.run((e) => e.align(op));
  }

  flip(horizontal: boolean) {
    if (this.selection.length) this.run((e) => e.flip(horizontal));
  }

  rotateBy(degrees: number) {
    if (this.selection.length) this.run((e) => e.rotateBy(degrees));
  }

  /** Live edits (colour picker drags) that undo as one step: begin, patch many times, end. */
  beginLive() {
    this.engine?.begin();
  }

  livePatch(change: Patch) {
    if (!this.engine) return;
    this.engine.patch(change);
    this.refreshSelection();
    this.invalidate();
  }

  endLive() {
    this.engine?.commit();
    this.refreshAll();
  }

  /** The colours used on the page, most used first (colour picker swatches). */
  pageColors(): string[] {
    const r = this.engine?.agent("get_variable_defs", { page: true });
    const colors = (r?.result as { colors?: Record<string, number> } | undefined)?.colors ?? {};
    return Object.entries(colors)
      .sort((a, b) => b[1] - a[1])
      .map(([c]) => c.slice(0, 7))
      .filter((c, i, all) => all.indexOf(c) === i)
      .slice(0, 27);
  }

  /** Rename (Ctrl+R): edit the first selected layer's name in the layers panel. */
  renameSelection() {
    const first = this.selection[0];
    if (first) this.renaming = first.id;
  }

  private async writeClipboard(text: string, what: string) {
    try {
      await navigator.clipboard.writeText(text);
      this.notify(`Copied ${what}`);
    } catch {
      this.notify("The clipboard is not available");
    }
  }

  copyAsText() {
    const text = this.engine?.textContent() ?? "";
    if (!text) return this.notify("No text in the selection");
    void this.writeClipboard(text, "text");
  }

  copyAsSvg() {
    const svg = this.engine?.svg() ?? "";
    if (svg) void this.writeClipboard(svg, "as SVG");
  }

  /** Copy as PNG (Ctrl+Shift+C), at 2× like Figma. */
  async copyAsPng() {
    const first = this.selection[0];
    if (!first || !this.engine) return;
    const max = Math.min(4096, Math.max(first.w, first.h) * 2);
    const shot = this.engine.capture(first.id, max);
    if (!shot) return;
    try {
      const blob = await (await fetch(shot.url)).blob();
      await navigator.clipboard.write([new ClipboardItem({ "image/png": blob })]);
      this.notify("Copied as PNG");
    } catch {
      this.notify("Copying images is not supported here");
    }
  }

  /** Copy as code: "css" (the layer's own styles), "html" or "react" (the whole subtree). */
  async copyAsCode(kind: "css" | "tailwind" | "html" | "react") {
    const first = this.selection[0];
    if (!first || !this.engine) return;
    if (kind === "css" || kind === "tailwind") {
      const { css, tailwind } = await import("./code");
      return this.writeClipboard(kind === "css" ? css(first) : tailwind(first), kind === "css" ? "CSS" : "Tailwind classes");
    }
    const r = this.engine.agent("get_design_context", { nodeId: String(first.id), framework: kind });
    const code = (r.result as { code?: string } | undefined)?.code;
    if (code) void this.writeClipboard(code, kind === "html" ? "HTML + CSS" : "React + Tailwind");
  }

  /** Copy properties (Ctrl+Alt+C): fill, stroke, radius, opacity, blend and text style. */
  copyProperties() {
    const n = this.selection[0];
    if (!n) return;
    const patch: Patch = {
      fill: n.fill,
      fillOpacity: n.fillOpacity,
      fillOn: n.fillOn,
      opacity: n.opacity,
      blend: n.blend,
      strokeOn: !!n.stroke,
    };
    if (n.kind === "Frame" || n.kind === "Rectangle") {
      if (n.corners) patch.corners = [...n.corners];
      else patch.radius = n.radius;
    }
    if (n.stroke) {
      patch.strokeColor = n.stroke.color;
      patch.strokeOpacity = n.stroke.opacity;
      patch.strokeWidth = n.stroke.width;
      patch.strokeAlign = n.stroke.align;
    }
    let text: TextStyle | null = null;
    if (n.text) {
      const { weightName: _w, linePx: _l, textHeight: _t, ...style } = n.text;
      text = style;
    }
    this.properties = { patch, text };
    this.notify("Copied properties");
  }

  /** Paste properties (Ctrl+Alt+V) onto the selection. */
  pasteProperties() {
    const p = this.properties;
    if (!p || !this.engine || !this.selection.length) return;
    this.engine.patch(p.patch);
    if (p.text && this.selection.some((n) => n.text)) {
      for (const n of this.selection.filter((n) => n.text)) {
        this.engine.setTextStyle(n.id, { ...p.text, content: n.text!.content, autoName: n.text!.autoName });
      }
    }
    this.refreshAll();
  }

  get hasProperties() {
    return this.properties !== null;
  }

  /** Paste to replace (Ctrl+Shift+R). */
  async pasteReplace() {
    let text = this.clipboard;
    try {
      const fromSystem = await navigator.clipboard.readText();
      if (fromSystem.includes("pergel/layers")) text = fromSystem;
    } catch {
      // fall back to the in-app clipboard
    }
    if (!text) return this.notify("Nothing to paste");
    this.run((e) => e.pasteReplace(text));
  }

  resizeToFit() {
    this.run((e) => e.resizeToFit());
  }

  createComponent() {
    if (this.selection.length) this.run((e) => e.createComponent());
  }

  detachInstance() {
    if (this.selection.length) this.run((e) => e.detachInstance());
  }

  resetInstance() {
    this.run((e) => e.resetInstance());
  }

  goToMain() {
    this.run((e) => e.goToMain());
  }

  /** Places an instance of a component at a canvas point (the view centre by default). */
  insertInstance(main: number, x = -1, y = -1) {
    this.run((e) => e.createInstance(main, x, y));
  }

  /** Lists the document's components with thumbnails (for the Assets tab). */
  refreshComponents() {
    const engine = this.engine;
    if (!engine) return;
    this.components = engine.components().map((c) => ({ ...c, thumbnail: engine.thumbnail(c.id, 192) }));
    void import("./libraries.svelte").then((m) => m.libraries.refresh());
  }

  autoLayout(add = true) {
    if (this.selection.length) this.run((e) => e.autoLayout(add));
  }

  swapFillStroke() {
    if (this.selection.length) this.run((e) => e.swapFillStroke());
  }

  /** The value shared by every selected layer, or null when they differ ("Mixed"). */
  common<T>(get: (n: NodeInfo) => T): T | null {
    if (!this.selection.length) return null;
    const first = get(this.selection[0]);
    const key = JSON.stringify(first);
    return this.selection.every((n) => JSON.stringify(get(n)) === key) ? first : null;
  }

  selectAll() {
    this.run((e) => e.selectRelative("siblings"));
  }

  rename(id: number, name: string) {
    if (this.engine?.rename(id, name)) this.refreshAll();
  }

  zoomBy(factor: number) {
    const canvas = document.getElementById("pergel-canvas");
    if (!this.engine || !canvas) return;
    const r = canvas.getBoundingClientRect();
    this.engine.zoom(factor, r.width / 2, r.height / 2);
    this.invalidate();
  }

  zoomTo(level: number) {
    this.zoomBy(level / (this.engine?.zoomLevel() ?? 1));
  }

  fit() {
    this.engine?.fit();
    this.invalidate();
  }

  fitSelection() {
    this.engine?.fitSelection();
    this.invalidate();
  }

  benchmarking = $state(false);
  benchmark = $state<{ count: number; avg: number; p95: number; worst: number; frames: number } | null>(null);

  loadSample() {
    this.engine?.loadSample();
    this.fileName = "Untitled";
    this.expanded = new Set();
    this.refreshAll();
  }

  loadBenchmark(count: number) {
    this.engine?.loadBenchmark(count);
    this.expanded = new Set();
    this.fileName = `Benchmark · ${count.toLocaleString("en-US")} shapes`;
    this.refreshAll();
  }

  async runBenchmark() {
    const canvas = document.getElementById("pergel-canvas");
    if (!this.engine || !canvas || this.benchmarking) return;
    this.engine.fit();
    this.benchmark = null;
    this.benchmarking = true;
    const r = canvas.getBoundingClientRect();
    const samples: number[] = [];
    const duration = 6000;
    const t0 = performance.now();
    let prev = t0;
    await new Promise<void>((done) => {
      const step = (now: number) => {
        samples.push(now - prev);
        prev = now;
        const t = (now - t0) / duration;
        this.engine!.zoom(Math.exp(Math.sin(t * Math.PI * 4) * 0.035), r.width / 2, r.height / 2);
        this.engine!.pan(Math.cos(t * Math.PI * 6) * 12, Math.sin(t * Math.PI * 6) * 8);
        this.invalidate();
        if (now - t0 < duration) requestAnimationFrame(step);
        else done();
      };
      requestAnimationFrame(step);
    });
    this.benchmarking = false;
    const sorted = samples.slice(1).sort((a, b) => a - b);
    const avg = sorted.reduce((a, b) => a + b, 0) / sorted.length;
    this.benchmark = {
      count: this.layers.length,
      avg: Math.round(1000 / avg),
      p95: sorted[Math.floor(sorted.length * 0.95)],
      worst: sorted[sorted.length - 1],
      frames: sorted.length,
    };
    this.engine.fit();
    this.invalidate();
  }

  notify(message: string) {
    this.toast = message;
    setTimeout(() => {
      if (this.toast === message) this.toast = null;
    }, 2200);
  }
}

export const editor = new Editor();
