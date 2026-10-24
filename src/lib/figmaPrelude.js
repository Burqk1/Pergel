// Figma's plugin API, run inside the plugin sandbox (QuickJS; see figmaSandbox.ts). The sandbox
// has no window, DOM or app access: everything goes through `__host(op, json)`, which takes and
// returns JSON text, so no object of the app ever reaches plugin code.

const host = (op, args) => {
  const r = JSON.parse(__host(op, JSON.stringify(args === undefined ? null : args)));
  if (!r.ok) throw new Error(r.error);
  return r.value;
};
const call = (tool, args) => host("agent", { tool, args });

globalThis.console = {
  log: (...a) => host("log", a.map(String).join(" ")),
  info: (...a) => host("log", a.map(String).join(" ")),
  warn: (...a) => host("log", a.map(String).join(" ")),
  error: (...a) => host("log", a.map(String).join(" ")),
  debug: () => {},
};

// Timers run on the app's clock.
const timers = new Map();
let nextTimer = 1;
globalThis.setTimeout = (fn, ms = 0, ...args) => {
  const id = nextTimer++;
  timers.set(id, { fn, args, every: 0 });
  host("setTimeout", { id, ms: Number(ms) || 0 });
  return id;
};
globalThis.setInterval = (fn, ms = 0, ...args) => {
  const id = nextTimer++;
  timers.set(id, { fn, args, every: Math.max(4, Number(ms) || 0) });
  host("setTimeout", { id, ms: Math.max(4, Number(ms) || 0) });
  return id;
};
globalThis.clearTimeout = globalThis.clearInterval = (id) => {
  timers.delete(id);
  host("clearTimeout", { id });
};
globalThis.__fire = (id) => {
  const t = timers.get(id);
  if (!t) return;
  if (t.every) host("setTimeout", { id, ms: t.every });
  else timers.delete(id);
  t.fn(...t.args);
};

const B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
function fromBase64(s) {
  const clean = s.replace(/[^A-Za-z0-9+/]/g, "");
  const out = new Uint8Array(Math.floor((clean.length * 3) / 4));
  let o = 0;
  for (let i = 0; i < clean.length; i += 4) {
    const n = (B64.indexOf(clean[i]) << 18) | (B64.indexOf(clean[i + 1]) << 12) | ((B64.indexOf(clean[i + 2]) & 63) << 6) | (B64.indexOf(clean[i + 3]) & 63);
    if (o < out.length) out[o++] = (n >> 16) & 255;
    if (o < out.length) out[o++] = (n >> 8) & 255;
    if (o < out.length) out[o++] = n & 255;
  }
  return out;
}
function utf8(s) {
  const out = [];
  for (const ch of s) {
    let c = ch.codePointAt(0);
    if (c < 0x80) out.push(c);
    else if (c < 0x800) out.push(0xc0 | (c >> 6), 0x80 | (c & 63));
    else if (c < 0x10000) out.push(0xe0 | (c >> 12), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
    else out.push(0xf0 | (c >> 18), 0x80 | ((c >> 12) & 63), 0x80 | ((c >> 6) & 63), 0x80 | (c & 63));
  }
  return new Uint8Array(out);
}

const hex2 = (v) => Math.round(Math.min(1, Math.max(0, v)) * 255).toString(16).padStart(2, "0");
const toHex = (c, a = 1) => `#${hex2(c.r)}${hex2(c.g)}${hex2(c.b)}${a < 1 ? hex2(a) : ""}`.toUpperCase();
const toRgb = (hex) => {
  const h = hex.replace("#", "");
  return { r: parseInt(h.slice(0, 2), 16) / 255, g: parseInt(h.slice(2, 4), 16) / 255, b: parseInt(h.slice(4, 6), 16) / 255 };
};

const TYPES = { frame: "FRAME", rectangle: "RECTANGLE", ellipse: "ELLIPSE", text: "TEXT", vector: "VECTOR", group: "GROUP", component: "COMPONENT", instance: "INSTANCE", section: "SECTION", slice: "SLICE" };
const WEIGHTS = { thin: 100, extralight: 200, light: 300, regular: 400, medium: 500, semibold: 600, bold: 700, extrabold: 800, black: 900 };
const ALIGN_IN = { left: "LEFT", center: "CENTER", right: "RIGHT", justify: "JUSTIFIED" };
const FLOW_AXIS = { start: "MIN", center: "CENTER", end: "MAX", "space-between": "SPACE_BETWEEN", baseline: "BASELINE" };

// The same layer is always the same object, as in Figma.
const cache = new Map();
const pluginData = new Map();
const wrap = (id) => {
  const key = String(id);
  let n = cache.get(key);
  if (!n) {
    n = new FigmaNode(key);
    cache.set(key, n);
  }
  return n;
};
const kids = (id) => call("list_children", id === undefined ? {} : { nodeId: id }).children;

function walk(ids, out, cb) {
  for (const id of ids) {
    const n = wrap(id);
    if (!cb || cb(n)) out.push(n);
    walk(kids(id), out, cb);
  }
  return out;
}

class FigmaNode {
  constructor(id) {
    this.id = id;
  }
  get info() {
    return call("get_node_details", { nodeId: this.id });
  }
  set(props) {
    call("update_node", { nodeId: this.id, ...props });
  }
  get removed() {
    try {
      call("get_node", { nodeId: this.id });
      return false;
    } catch {
      return true;
    }
  }
  get type() {
    return TYPES[this.info.type] ?? "FRAME";
  }
  get name() {
    return this.info.name;
  }
  set name(v) {
    this.set({ name: v });
  }
  get x() {
    return this.info.relX;
  }
  set x(v) {
    this.set({ x: v });
  }
  get y() {
    return this.info.relY;
  }
  set y(v) {
    this.set({ y: v });
  }
  get width() {
    return this.info.w;
  }
  get height() {
    return this.info.h;
  }
  resize(w, h) {
    this.set({ width: w, height: h });
  }
  resizeWithoutConstraints(w, h) {
    this.resize(w, h);
  }
  get rotation() {
    return this.info.rotation;
  }
  set rotation(v) {
    this.set({ rotation: v });
  }
  get opacity() {
    const o = this.info.opacity;
    return o > 1 ? o / 100 : o;
  }
  set opacity(v) {
    this.set({ opacity: v });
  }
  get visible() {
    return this.info.visible;
  }
  set visible(v) {
    this.set({ visible: v });
  }
  get locked() {
    return this.info.locked;
  }
  get cornerRadius() {
    return this.info.radius;
  }
  set cornerRadius(v) {
    this.set({ radius: v });
  }
  get fills() {
    const i = this.info;
    return i.fillOn ? [{ type: "SOLID", color: toRgb(i.fill), opacity: i.fillOpacity / 100, visible: true }] : [];
  }
  set fills(list) {
    const p = list.filter((f) => f.visible !== false).at(-1);
    if (!p) this.set({ fill: "none" });
    else if (p.type === "SOLID") this.set({ fill: toHex(p.color, p.opacity ?? 1) });
  }
  get strokes() {
    const s = this.info.stroke;
    return s ? [{ type: "SOLID", color: toRgb(s.color), opacity: s.opacity / 100 }] : [];
  }
  set strokes(list) {
    const p = list.at(-1);
    if (!p) this.set({ stroke: "none" });
    else this.set({ stroke: { color: toHex(p.color, p.opacity ?? 1), width: this.info.stroke?.width ?? 1 } });
  }
  get strokeWeight() {
    return this.info.stroke?.width ?? 0;
  }
  set strokeWeight(v) {
    const s = this.info.stroke;
    this.set({ stroke: { color: s ? s.color : "#000000", width: v } });
  }
  // Text.
  get characters() {
    return this.info.text?.content ?? "";
  }
  set characters(v) {
    this.set({ text: v });
  }
  get fontSize() {
    return this.info.text?.size ?? 0;
  }
  set fontSize(v) {
    this.set({ fontSize: v });
  }
  get fontName() {
    const t = this.info.text;
    return { family: t?.family ?? "Inter", style: `${t?.weightName ?? "Regular"}${t?.italic ? " Italic" : ""}` };
  }
  set fontName(f) {
    const key = f.style.toLowerCase().replace(/italic|\s/g, "");
    this.set({ fontFamily: f.family, fontWeight: WEIGHTS[key] ?? 400 });
  }
  get textAlignHorizontal() {
    return ALIGN_IN[this.info.text?.align ?? "left"] ?? "LEFT";
  }
  set textAlignHorizontal(v) {
    this.set({ textAlign: v === "JUSTIFIED" ? "justify" : v.toLowerCase() });
  }
  // Auto layout.
  get layoutMode() {
    const l = this.info.layout;
    return !l ? "NONE" : l.flow === "vertical" ? "VERTICAL" : "HORIZONTAL";
  }
  set layoutMode(v) {
    if (v !== "NONE") {
      const l = this.info.layout;
      this.set({ layout: { direction: v === "VERTICAL" ? "vertical" : "horizontal", gap: l?.gap ?? 0, padding: l?.padding ?? [0, 0, 0, 0] } });
    }
  }
  setLayout(change) {
    const l = this.info.layout;
    if (!l) return;
    this.set({ layout: { direction: l.flow, gap: change.gap ?? l.gap, padding: change.padding ?? l.padding, justify: l.justify, align: l.align } });
  }
  get itemSpacing() {
    return this.info.layout?.gap ?? 0;
  }
  set itemSpacing(v) {
    this.setLayout({ gap: v });
  }
  pad(i) {
    return this.info.layout?.padding[i] ?? 0;
  }
  setPad(i, v) {
    const p = [...(this.info.layout?.padding ?? [0, 0, 0, 0])];
    p[i] = v;
    this.setLayout({ padding: p });
  }
  get paddingTop() {
    return this.pad(0);
  }
  set paddingTop(v) {
    this.setPad(0, v);
  }
  get paddingRight() {
    return this.pad(1);
  }
  set paddingRight(v) {
    this.setPad(1, v);
  }
  get paddingBottom() {
    return this.pad(2);
  }
  set paddingBottom(v) {
    this.setPad(2, v);
  }
  get paddingLeft() {
    return this.pad(3);
  }
  set paddingLeft(v) {
    this.setPad(3, v);
  }
  get primaryAxisAlignItems() {
    return FLOW_AXIS[this.info.layout?.justify ?? "start"] ?? "MIN";
  }
  get counterAxisAlignItems() {
    return FLOW_AXIS[this.info.layout?.align ?? "start"] ?? "MIN";
  }
  // Tree.
  get parent() {
    const p = this.info.parent;
    return p >= 0 ? wrap(p) : page;
  }
  get children() {
    return kids(this.id).map(wrap);
  }
  appendChild(child) {
    call("move_node", { nodeId: child.id, parentId: this.id });
  }
  insertChild(index, child) {
    call("move_node", { nodeId: child.id, parentId: this.id, index });
  }
  remove() {
    call("delete_nodes", { nodeIds: [this.id] });
    cache.delete(this.id);
  }
  clone() {
    return wrap(call("clone_node", { nodeId: this.id }).created.id);
  }
  findAll(cb) {
    return walk(kids(this.id), [], cb);
  }
  findOne(cb) {
    return this.findAll(cb)[0] ?? null;
  }
  findChildren(cb) {
    return this.children.filter((n) => !cb || cb(n));
  }
  findAllWithCriteria(c) {
    return this.findAll((n) => !c.types || c.types.includes(n.type));
  }
  // Components.
  get mainComponent() {
    const m = this.info.mainId;
    return m ? wrap(m) : null;
  }
  createInstance() {
    return wrap(call("create_instance", { componentId: this.id }).created.id);
  }
  // Data and export.
  setPluginData(key, value) {
    pluginData.set(this.id, { ...(pluginData.get(this.id) ?? {}), [key]: String(value) });
  }
  getPluginData(key) {
    return pluginData.get(this.id)?.[key] ?? "";
  }
  async exportAsync(opts) {
    const r = host("export", { id: this.id, format: opts?.format ?? "PNG" });
    return r.svg !== undefined ? utf8(r.svg) : fromBase64(r.png);
  }
  toString() {
    return `[${this.type} ${this.id}]`;
  }
}

function create(spec) {
  return wrap(call("create_nodes", { nodes: [spec] }).created[0].id);
}

const page = {
  type: "PAGE",
  id: "page",
  get name() {
    return host("pageName");
  },
  get selection() {
    return host("selection").map(wrap);
  },
  set selection(nodes) {
    host("setSelection", nodes.map((n) => n.id));
  },
  get children() {
    return kids().map(wrap);
  },
  findAll(cb) {
    return walk(kids(), [], cb);
  },
  findOne(cb) {
    return this.findAll(cb)[0] ?? null;
  },
  findAllWithCriteria(c) {
    return this.findAll((n) => !c.types || c.types.includes(n.type));
  },
  appendChild(child) {
    call("move_node", { nodeId: child.id });
  },
};

const listeners = {};
const ui = {
  onmessage: null,
  postMessage(msg) {
    host("ui", { method: "postMessage", msg });
  },
  on(type, cb) {
    if (type === "message") ui.onmessage = cb;
  },
  close() {
    host("ui", { method: "close" });
  },
  resize(width, height) {
    host("ui", { method: "resize", width, height });
  },
  show() {
    host("ui", { method: "show" });
  },
  hide() {
    host("ui", { method: "hide" });
  },
};

const exists = (id) => {
  try {
    call("get_node", { nodeId: String(id) });
    return true;
  } catch {
    return false;
  }
};

globalThis.figma = {
  apiVersion: "1.0.0",
  editorType: "figma",
  command: "",
  mixed: Symbol("figma.mixed"),
  currentPage: page,
  root: { type: "DOCUMENT", get name() { return host("fileName"); }, children: [page] },
  ui,
  showUI(html, opts = {}) {
    host("showUI", { html: String(html), width: opts.width, height: opts.height, visible: opts.visible, title: opts.title });
  },
  closePlugin(message) {
    host("close", message === undefined ? null : String(message));
  },
  notify(message) {
    host("notify", String(message));
    return { cancel() {} };
  },
  createRectangle: () => create({ type: "rectangle", width: 100, height: 100, fill: "#D9D9D9" }),
  createEllipse: () => create({ type: "ellipse", width: 100, height: 100, fill: "#D9D9D9" }),
  createFrame: () => create({ type: "frame", width: 100, height: 100, fill: "#FFFFFF" }),
  createComponent: () => create({ type: "component", width: 100, height: 100 }),
  createText: () => create({ type: "text", text: "" }),
  createLine: () => create({ type: "line", width: 100 }),
  getNodeById: (id) => (exists(id) ? wrap(id) : null),
  getNodeByIdAsync: async (id) => (exists(id) ? wrap(id) : null),
  loadFontAsync: async () => undefined,
  listAvailableFontsAsync: async () => host("fonts").map((family) => ({ fontName: { family, style: "Regular" } })),
  group(nodes) {
    return wrap(host("group", nodes.map((n) => n.id)));
  },
  viewport: {
    scrollAndZoomIntoView(nodes) {
      call("select_nodes", { nodeIds: nodes.map((n) => n.id), zoom: true });
    },
    get zoom() {
      return host("zoom");
    },
  },
  clientStorage: {
    async getAsync(key) {
      return host("storageGet", String(key)) ?? undefined;
    },
    async setAsync(key, value) {
      host("storageSet", { key: String(key), value });
    },
  },
  on(event, cb) {
    (listeners[event] ??= []).push(cb);
  },
  off(event, cb) {
    listeners[event] = (listeners[event] ?? []).filter((x) => x !== cb);
  },
  util: {
    rgb: (hex) => toRgb(hex),
    solidPaint: (hex) => ({ type: "SOLID", color: toRgb(hex) }),
  },
};

// Entry points for the app: events, and messages from the plugin's UI.
globalThis.__dispatch = (event) => {
  for (const cb of listeners[event] ?? []) cb();
};
globalThis.__uiMessage = (json) => {
  ui.onmessage?.(JSON.parse(json));
};
