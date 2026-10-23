import { editor } from "./editor.svelte";

// Figma plugin compatibility: a `figma` object with the plugin API's everyday parts, run on
// Pergel's document. The API is synchronous, so plugins run in the page (like Figma's main
// thread) and every property read or write is an engine tool call. `figma.showUI` opens the
// plugin's HTML in a sandboxed iframe that talks through `pluginMessage`, as in Figma.

export interface FigmaPluginFiles {
  name: string;
  code: string;
  html: string;
}

type Info = Record<string, unknown> & {
  type: string;
  name: string;
  relX: number;
  relY: number;
  w: number;
  h: number;
  rotation: number;
  radius: number;
  opacity: number;
  visible: boolean;
  locked: boolean;
  fill: string;
  fillOpacity: number;
  fillOn: boolean;
  stroke: { color: string; opacity: number; width: number } | null;
  parent: number;
  children: string[];
  mainId: string | null;
  text: { content: string; size: number; family: string; weight: number; weightName: string; italic: boolean; align: string } | null;
  layout: { flow: string; gap: number; padding: [number, number, number, number]; justify: string; align: string } | null;
};

type RGB = { r: number; g: number; b: number };
type SolidPaint = { type: "SOLID"; color: RGB; opacity?: number; visible?: boolean };

/** The plugin that is open, and its UI panel. */
export const figmaPlugin = $state<{ name: string; html: string; width: number; height: number; visible: boolean; open: boolean }>({
  name: "",
  html: "",
  width: 300,
  height: 200,
  visible: false,
  open: false,
});

let frame: HTMLIFrameElement | null = null;
let onUiMessage: ((msg: unknown) => void) | null = null;
const listeners: Record<string, Array<(arg?: unknown) => void>> = {};
let pollSelection = 0;
let refreshQueued = false;

function call<T = Record<string, unknown>>(tool: string, args: Record<string, unknown>): T {
  const engine = editor.engine;
  if (!engine) throw new Error("The engine is still loading");
  const r = engine.agent(tool, args);
  if (!r.ok) throw new Error(r.error ?? `${tool} failed`);
  if (!["get_node_details", "list_children", "get_node", "get_selection", "get_metadata"].includes(tool)) queueRefresh();
  return r.result as T;
}

function queueRefresh() {
  if (refreshQueued) return;
  refreshQueued = true;
  queueMicrotask(() => {
    refreshQueued = false;
    editor.refreshAll();
  });
}

const hex2 = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0");
const toHex = (c: RGB, a = 1) => `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}${a < 1 ? hex2(a) : ""}`.toUpperCase();
const toRgb = (hex: string): RGB => {
  const h = hex.replace("#", "");
  return { r: parseInt(h.slice(0, 2), 16) / 255, g: parseInt(h.slice(2, 4), 16) / 255, b: parseInt(h.slice(4, 6), 16) / 255 };
};

const TYPES: Record<string, string> = {
  frame: "FRAME",
  rectangle: "RECTANGLE",
  ellipse: "ELLIPSE",
  text: "TEXT",
  vector: "VECTOR",
  group: "GROUP",
  component: "COMPONENT",
  instance: "INSTANCE",
  section: "SECTION",
  slice: "SLICE",
};

const WEIGHTS: Record<string, number> = { thin: 100, extralight: 200, light: 300, regular: 400, medium: 500, semibold: 600, bold: 700, extrabold: 800, black: 900 };
const ALIGN_IN: Record<string, string> = { left: "LEFT", center: "CENTER", right: "RIGHT", justify: "JUSTIFIED" };
const FLOW_AXIS: Record<string, string> = { start: "MIN", center: "CENTER", end: "MAX", "space-between": "SPACE_BETWEEN", baseline: "BASELINE" };

const cache = new Map<string, FigmaNode>();
const pluginData = new Map<string, Record<string, string>>();
const wrap = (id: string | number) => {
  const key = String(id);
  let n = cache.get(key);
  if (!n) {
    n = new FigmaNode(key);
    cache.set(key, n);
  }
  return n;
};

function walk(ids: string[], out: FigmaNode[], cb?: (n: FigmaNode) => boolean) {
  for (const id of ids) {
    const n = wrap(id);
    if (!cb || cb(n)) out.push(n);
    walk(call<{ children: string[] }>("list_children", { nodeId: id }).children, out, cb);
  }
  return out;
}

class FigmaNode {
  constructor(readonly id: string) {}
  private get info(): Info {
    return call<Info>("get_node_details", { nodeId: this.id });
  }
  private set(props: Record<string, unknown>) {
    call("update_node", { nodeId: this.id, ...props });
  }
  get removed() {
    return !editor.engine?.agent("get_node", { nodeId: this.id }).ok;
  }
  get type() {
    return TYPES[this.info.type] ?? "FRAME";
  }
  get name() {
    return this.info.name;
  }
  set name(v: string) {
    this.set({ name: v });
  }
  get x() {
    return this.info.relX;
  }
  set x(v: number) {
    this.set({ x: v });
  }
  get y() {
    return this.info.relY;
  }
  set y(v: number) {
    this.set({ y: v });
  }
  get width() {
    return this.info.w;
  }
  get height() {
    return this.info.h;
  }
  resize(w: number, h: number) {
    this.set({ width: w, height: h });
  }
  resizeWithoutConstraints(w: number, h: number) {
    this.resize(w, h);
  }
  get rotation() {
    return this.info.rotation;
  }
  set rotation(v: number) {
    this.set({ rotation: v });
  }
  get opacity() {
    const o = this.info.opacity;
    return o > 1 ? o / 100 : o;
  }
  set opacity(v: number) {
    this.set({ opacity: v });
  }
  get visible() {
    return this.info.visible;
  }
  set visible(v: boolean) {
    this.set({ visible: v });
  }
  get locked() {
    return this.info.locked;
  }
  get cornerRadius() {
    return this.info.radius;
  }
  set cornerRadius(v: number) {
    this.set({ radius: v });
  }
  get fills(): SolidPaint[] {
    const i = this.info;
    return i.fillOn ? [{ type: "SOLID", color: toRgb(i.fill), opacity: i.fillOpacity / 100, visible: true }] : [];
  }
  set fills(list: SolidPaint[]) {
    const p = list.filter((f) => f.visible !== false).at(-1);
    if (!p) this.set({ fill: "none" });
    else if (p.type === "SOLID") this.set({ fill: toHex(p.color, p.opacity ?? 1) });
  }
  get strokes(): SolidPaint[] {
    const s = this.info.stroke;
    return s ? [{ type: "SOLID", color: toRgb(s.color), opacity: s.opacity / 100 }] : [];
  }
  set strokes(list: SolidPaint[]) {
    const p = list.at(-1);
    if (!p) this.set({ stroke: "none" });
    else this.set({ stroke: { color: toHex(p.color, p.opacity ?? 1), width: this.info.stroke?.width ?? 1 } });
  }
  get strokeWeight() {
    return this.info.stroke?.width ?? 0;
  }
  set strokeWeight(v: number) {
    const s = this.info.stroke;
    this.set({ stroke: { color: s ? s.color : "#000000", width: v } });
  }
  // Text.
  get characters() {
    return this.info.text?.content ?? "";
  }
  set characters(v: string) {
    this.set({ text: v });
  }
  get fontSize() {
    return this.info.text?.size ?? 0;
  }
  set fontSize(v: number) {
    this.set({ fontSize: v });
  }
  get fontName() {
    const t = this.info.text;
    return { family: t?.family ?? "Inter", style: `${t?.weightName ?? "Regular"}${t?.italic ? " Italic" : ""}` };
  }
  set fontName(f: { family: string; style: string }) {
    const key = f.style.toLowerCase().replace(/italic|\s/g, "");
    this.set({ fontFamily: f.family, fontWeight: WEIGHTS[key] ?? 400 });
  }
  get textAlignHorizontal() {
    return ALIGN_IN[this.info.text?.align ?? "left"] ?? "LEFT";
  }
  set textAlignHorizontal(v: string) {
    this.set({ textAlign: v === "JUSTIFIED" ? "justify" : v.toLowerCase() });
  }
  // Auto layout.
  get layoutMode() {
    const l = this.info.layout;
    return !l ? "NONE" : l.flow === "vertical" ? "VERTICAL" : "HORIZONTAL";
  }
  set layoutMode(v: string) {
    if (v !== "NONE") {
      const l = this.info.layout;
      this.set({ layout: { direction: v === "VERTICAL" ? "vertical" : "horizontal", gap: l?.gap ?? 0, padding: l?.padding ?? [0, 0, 0, 0] } });
    }
  }
  private setLayout(change: { gap?: number; padding?: [number, number, number, number] }) {
    const l = this.info.layout;
    if (!l) return;
    this.set({ layout: { direction: l.flow, gap: change.gap ?? l.gap, padding: change.padding ?? l.padding, justify: l.justify, align: l.align } });
  }
  get itemSpacing() {
    return this.info.layout?.gap ?? 0;
  }
  set itemSpacing(v: number) {
    this.setLayout({ gap: v });
  }
  private pad(i: number) {
    return this.info.layout?.padding[i] ?? 0;
  }
  private setPad(i: number, v: number) {
    const p = [...(this.info.layout?.padding ?? [0, 0, 0, 0])] as [number, number, number, number];
    p[i] = v;
    this.setLayout({ padding: p });
  }
  get paddingTop() {
    return this.pad(0);
  }
  set paddingTop(v: number) {
    this.setPad(0, v);
  }
  get paddingRight() {
    return this.pad(1);
  }
  set paddingRight(v: number) {
    this.setPad(1, v);
  }
  get paddingBottom() {
    return this.pad(2);
  }
  set paddingBottom(v: number) {
    this.setPad(2, v);
  }
  get paddingLeft() {
    return this.pad(3);
  }
  set paddingLeft(v: number) {
    this.setPad(3, v);
  }
  get primaryAxisAlignItems() {
    return FLOW_AXIS[this.info.layout?.justify ?? "start"] ?? "MIN";
  }
  get counterAxisAlignItems() {
    return FLOW_AXIS[this.info.layout?.align ?? "start"] ?? "MIN";
  }
  // Tree.
  get parent(): FigmaNode | typeof page {
    const p = this.info.parent;
    return p >= 0 ? wrap(p) : page;
  }
  get children() {
    return call<{ children: string[] }>("list_children", { nodeId: this.id }).children.map(wrap);
  }
  appendChild(child: FigmaNode) {
    call("move_node", { nodeId: child.id, parentId: this.id });
  }
  insertChild(index: number, child: FigmaNode) {
    call("move_node", { nodeId: child.id, parentId: this.id, index });
  }
  remove() {
    call("delete_nodes", { nodeIds: [this.id] });
    cache.delete(this.id);
  }
  clone() {
    return wrap(call<{ created: { id: string } }>("clone_node", { nodeId: this.id }).created.id);
  }
  findAll(cb?: (n: FigmaNode) => boolean) {
    return walk(call<{ children: string[] }>("list_children", { nodeId: this.id }).children, [], cb);
  }
  findOne(cb: (n: FigmaNode) => boolean) {
    return this.findAll(cb)[0] ?? null;
  }
  findChildren(cb?: (n: FigmaNode) => boolean) {
    return this.children.filter((n) => !cb || cb(n));
  }
  findAllWithCriteria(c: { types?: string[] }) {
    return this.findAll((n) => !c.types || c.types.includes(n.type));
  }
  // Components.
  get mainComponent(): FigmaNode | null {
    const m = this.info.mainId;
    return m ? wrap(m) : null;
  }
  createInstance() {
    return wrap(call<{ created: { id: string } }>("create_instance", { componentId: this.id }).created.id);
  }
  // Data and export.
  setPluginData(key: string, value: string) {
    pluginData.set(this.id, { ...(pluginData.get(this.id) ?? {}), [key]: value });
  }
  getPluginData(key: string) {
    return pluginData.get(this.id)?.[key] ?? "";
  }
  async exportAsync(opts?: { format?: string }) {
    if (opts?.format === "SVG") return new TextEncoder().encode(editor.engine?.svg(Number(this.id)) ?? "");
    const shot = editor.engine?.capture(Number(this.id), 2048);
    if (!shot) throw new Error("Nothing to export");
    const b64 = shot.url.slice(shot.url.indexOf(",") + 1);
    return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  }
  toString() {
    return `[${this.type} ${this.id}]`;
  }
}

function create(spec: Record<string, unknown>) {
  const created = call<{ created: { id: string }[] }>("create_nodes", { nodes: [spec] }).created;
  return wrap(created[0].id);
}

const page = {
  type: "PAGE",
  id: "page",
  get name() {
    return editor.pages.find((p) => p.current)?.name ?? "Page 1";
  },
  get selection() {
    return editor.selection.map((n) => wrap(n.id));
  },
  set selection(nodes: FigmaNode[]) {
    call("select_nodes", { nodeIds: nodes.map((n) => n.id), zoom: false });
    editor.refreshSelection();
  },
  get children() {
    return call<{ children: string[] }>("list_children", {}).children.map(wrap);
  },
  findAll(cb?: (n: FigmaNode) => boolean) {
    return walk(call<{ children: string[] }>("list_children", {}).children, [], cb);
  },
  findOne(cb: (n: FigmaNode) => boolean) {
    return this.findAll(cb)[0] ?? null;
  },
  findAllWithCriteria(c: { types?: string[] }) {
    return this.findAll((n) => !c.types || c.types.includes(n.type));
  },
  appendChild(child: FigmaNode) {
    call("move_node", { nodeId: child.id });
  },
};

function makeFigma(name: string, html: string, done: () => void) {
  const ui = {
    onmessage: null as ((msg: unknown) => void) | null,
    postMessage(msg: unknown) {
      frame?.contentWindow?.postMessage({ pluginMessage: msg }, "*");
    },
    on(type: string, cb: (msg: unknown) => void) {
      if (type === "message") ui.onmessage = cb;
    },
    close() {
      figmaPlugin.open = false;
    },
    resize(w: number, h: number) {
      figmaPlugin.width = w;
      figmaPlugin.height = h;
    },
    show() {
      figmaPlugin.visible = true;
    },
    hide() {
      figmaPlugin.visible = false;
    },
  };
  onUiMessage = (msg) => ui.onmessage?.(msg);
  return {
    apiVersion: "1.0.0",
    editorType: "figma",
    command: "",
    mixed: Symbol("figma.mixed"),
    currentPage: page,
    root: { type: "DOCUMENT", name: editor.fileName, children: [page] },
    ui,
    showUI(markup: string, opts: { width?: number; height?: number; visible?: boolean; title?: string } = {}) {
      Object.assign(figmaPlugin, { name: opts.title ?? name, html: markup, width: opts.width ?? 300, height: opts.height ?? 200, visible: opts.visible ?? true, open: true });
    },
    closePlugin(message?: string) {
      if (message) editor.notify(message);
      done();
    },
    notify(message: string) {
      editor.notify(String(message));
      return { cancel() {} };
    },
    createRectangle: () => create({ type: "rectangle", width: 100, height: 100, fill: "#D9D9D9" }),
    createEllipse: () => create({ type: "ellipse", width: 100, height: 100, fill: "#D9D9D9" }),
    createFrame: () => create({ type: "frame", width: 100, height: 100, fill: "#FFFFFF" }),
    createComponent: () => create({ type: "component", width: 100, height: 100 }),
    createText: () => create({ type: "text", text: "" }),
    createLine: () => create({ type: "line", width: 100 }),
    getNodeById: (id: string) => (editor.engine?.agent("get_node", { nodeId: String(id) }).ok ? wrap(id) : null),
    getNodeByIdAsync: async (id: string) => (editor.engine?.agent("get_node", { nodeId: String(id) }).ok ? wrap(id) : null),
    loadFontAsync: async () => undefined,
    listAvailableFontsAsync: async () => editor.fonts.map((f) => ({ fontName: { family: f.name, style: "Regular" } })),
    group(nodes: FigmaNode[]) {
      call("select_nodes", { nodeIds: nodes.map((n) => n.id), zoom: false });
      editor.refreshSelection();
      editor.group();
      return wrap(editor.selection[0]?.id ?? nodes[0].id);
    },
    viewport: {
      scrollAndZoomIntoView(nodes: FigmaNode[]) {
        call("select_nodes", { nodeIds: nodes.map((n) => n.id), zoom: true });
      },
      get zoom() {
        return editor.view.zoom;
      },
    },
    clientStorage: {
      async getAsync(key: string) {
        try {
          const v = localStorage.getItem(`pergel.figma.${name}.${key}`);
          return v === null ? undefined : JSON.parse(v);
        } catch {
          return undefined;
        }
      },
      async setAsync(key: string, value: unknown) {
        try {
          localStorage.setItem(`pergel.figma.${name}.${key}`, JSON.stringify(value));
        } catch {
          // storage unavailable
        }
      },
    },
    on(event: string, cb: (arg?: unknown) => void) {
      (listeners[event] ??= []).push(cb);
    },
    off(event: string, cb: (arg?: unknown) => void) {
      listeners[event] = (listeners[event] ?? []).filter((x) => x !== cb);
    },
    util: {
      rgb: (hex: string) => toRgb(hex),
      solidPaint: (hex: string) => ({ type: "SOLID", color: toRgb(hex) }),
    },
  };
}

/** The iframe of the open plugin's UI (set by the panel). */
export function attachFrame(el: HTMLIFrameElement | null) {
  frame = el;
}

window.addEventListener("message", (e) => {
  if (!frame || e.source !== frame.contentWindow) return;
  const msg = (e.data as { pluginMessage?: unknown } | null)?.pluginMessage;
  if (msg === undefined || !onUiMessage) return;
  // Each message the UI sends is one undo step.
  editor.engine?.begin();
  try {
    onUiMessage(msg);
  } catch (err) {
    editor.notify(`Plugin error: ${String(err)}`);
  } finally {
    editor.engine?.commit();
    editor.refreshAll();
  }
});

let closer: (() => void) | null = null;

/** Runs a Figma plugin (its main code and UI). Resolves when it calls figma.closePlugin(). */
export function runFigmaPlugin(p: FigmaPluginFiles): Promise<void> {
  closer?.();
  return new Promise<void>((resolve) => {
    let finished = false;
    const done = () => {
      if (finished) return;
      finished = true;
      clearInterval(pollSelection);
      for (const cb of listeners.close ?? []) cb();
      for (const k of Object.keys(listeners)) delete listeners[k];
      figmaPlugin.open = false;
      onUiMessage = null;
      closer = null;
      cache.clear();
      editor.refreshAll();
      resolve();
    };
    closer = done;
    const figma = makeFigma(p.name, p.html, done);
    let last = editor.selection.map((n) => n.id).join(",");
    pollSelection = window.setInterval(() => {
      const now = editor.selection.map((n) => n.id).join(",");
      if (now !== last) {
        last = now;
        for (const cb of listeners.selectionchange ?? []) cb();
      }
    }, 300);
    editor.engine?.begin();
    try {
      const fn = new Function("figma", "__html__", "__uiFiles__", p.code);
      const out = fn(figma, p.html, { main: p.html });
      // Plugins without a UI that never close themselves end when their code settles.
      Promise.resolve(out).finally(() => {
        if (!figmaPlugin.open) setTimeout(done, 0);
      });
    } catch (err) {
      editor.notify(`Plugin error: ${String(err)}`);
      done();
    } finally {
      editor.engine?.commit();
      editor.refreshAll();
    }
  });
}

/** Closes the open plugin (the UI's close button). */
export function closeFigmaPlugin() {
  closer?.();
}

/** Reads a Figma plugin from its folder's files (manifest.json, its main script and UI). */
export async function readFigmaPlugin(files: File[]): Promise<FigmaPluginFiles> {
  const byName = (path: string) => {
    const want = path.replace(/^\.\//, "").toLowerCase();
    return files.find((f) => {
      const rel = (f.webkitRelativePath || f.name).toLowerCase();
      return rel === want || rel.endsWith(`/${want}`) || f.name.toLowerCase() === want.split("/").pop();
    });
  };
  const manifestFile = byName("manifest.json");
  if (!manifestFile) throw new Error("No manifest.json: pick the plugin's folder (or its manifest.json, code and UI files)");
  const manifest = JSON.parse(await manifestFile.text()) as { name?: string; main?: string; ui?: string | Record<string, string> };
  const main = manifest.main && byName(manifest.main);
  if (!main) throw new Error(`The plugin's main script (${manifest.main ?? "?"}) is missing`);
  const uiPath = typeof manifest.ui === "string" ? manifest.ui : manifest.ui ? Object.values(manifest.ui)[0] : undefined;
  const ui = uiPath ? byName(uiPath) : undefined;
  return { name: manifest.name ?? main.name, code: await main.text(), html: ui ? await ui.text() : "" };
}
