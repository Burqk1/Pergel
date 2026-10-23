import { editor } from "./editor.svelte";
import { runTool } from "./mcp";

// Plugins: JavaScript run in a Web Worker (no access to the page), talking to the document only
// through the `pergel` API — the same tools the MCP server offers — so a plugin can't break the
// editor. One run is one undo step.

export interface Plugin {
  id: string;
  name: string;
  code: string;
  builtin?: boolean;
  /** A Figma plugin (runs against the `figma` API), with its UI's HTML. */
  figma?: boolean;
  html?: string;
}

export interface RunResult {
  ok: boolean;
  result?: unknown;
  error?: string;
  logs: string[];
  ms: number;
}

const KEY = "pergel.plugins";

/** The API inside the worker. Every call is async and resolves with the tool's result. */
const WORKER = `
const pending = new Map();
let seq = 0;
const call = (method, args) => new Promise((resolve, reject) => {
  const id = ++seq;
  pending.set(id, { resolve, reject });
  postMessage({ type: "call", id, method, args });
});
const tool = async (name, args = {}) => call("tool", { name, args });
const pergel = {
  tool,
  selection: () => call("selection"),
  metadata: async (nodeId) => (await tool("get_metadata", nodeId ? { nodeId: String(nodeId) } : {})).xml,
  node: (nodeId) => tool("get_node", { nodeId: String(nodeId) }),
  create: async (nodes, parentId) => (await tool("create_nodes", { nodes: Array.isArray(nodes) ? nodes : [nodes], parentId: parentId == null ? undefined : String(parentId) })).created,
  update: (nodeId, properties) => tool("update_node", { nodeId: String(nodeId), ...properties }),
  remove: (ids) => tool("delete_nodes", { nodeIds: [].concat(ids).map(String) }),
  select: (ids) => tool("select_nodes", { nodeIds: [].concat(ids).map(String), zoom: false }),
  designContext: (nodeId) => tool("get_design_context", nodeId ? { nodeId: String(nodeId) } : {}),
  screenshot: (nodeId) => tool("get_screenshot", nodeId ? { nodeId: String(nodeId) } : {}),
  searchIcons: (query, limit) => tool("search_icons", { query, limit }),
  variables: () => tool("get_variable_defs", { page: true }),
  notify: (message) => call("notify", { message: String(message) }),
};
const text = (a) => a.map((x) => (typeof x === "string" ? x : JSON.stringify(x))).join(" ");
self.onmessage = async (e) => {
  const m = e.data;
  if (m.type === "reply") {
    const p = pending.get(m.id);
    pending.delete(m.id);
    if (p) m.ok ? p.resolve(m.result) : p.reject(new Error(m.error));
    return;
  }
  if (m.type !== "run") return;
  const log = (...a) => postMessage({ type: "log", text: text(a) });
  const console = { log, info: log, warn: log, error: log };
  try {
    const AsyncFunction = Object.getPrototypeOf(async function () {}).constructor;
    const fn = new AsyncFunction("pergel", "console", m.code);
    const result = await fn(pergel, console);
    postMessage({ type: "done", ok: true, result: result === undefined ? null : JSON.parse(JSON.stringify(result)) });
  } catch (err) {
    postMessage({ type: "done", ok: false, error: String((err && err.message) || err) });
  }
};
`;

export const BUILTIN: Plugin[] = [
  {
    id: "figma-rectangles",
    name: "Rectangle Creator (Figma plugin)",
    builtin: true,
    figma: true,
    // Figma's own "Rectangle Creator" starter plugin, unchanged.
    code: `figma.showUI(__html__);

figma.ui.onmessage = (msg) => {
  if (msg.type === "create-rectangles") {
    const nodes = [];
    for (let i = 0; i < msg.count; i++) {
      const rect = figma.createRectangle();
      rect.x = i * 150;
      rect.fills = [{ type: "SOLID", color: { r: 1, g: 0.5, b: 0 } }];
      figma.currentPage.appendChild(rect);
      nodes.push(rect);
    }
    figma.currentPage.selection = nodes;
    figma.viewport.scrollAndZoomIntoView(nodes);
  }
  figma.closePlugin();
};`,
    html: `<h2>Rectangle Creator</h2>
<p>Count: <input id="count" value="5"></p>
<button id="create">Create</button>
<button id="cancel">Cancel</button>
<script>
document.getElementById("create").onclick = () => {
  const count = parseInt(document.getElementById("count").value, 10);
  parent.postMessage({ pluginMessage: { type: "create-rectangles", count } }, "*");
};
document.getElementById("cancel").onclick = () => {
  parent.postMessage({ pluginMessage: { type: "cancel" } }, "*");
};
</script>`,
  },
  {
    id: "rename",
    name: "Rename selection",
    builtin: true,
    code: `// Renames the selected layers "Item 1", "Item 2"… in order.
const sel = await pergel.selection();
if (!sel.selection.length) return pergel.notify("Select some layers first");
let i = 1;
for (const n of sel.selection) await pergel.update(n.id, { name: "Item " + i++ });
pergel.notify("Renamed " + (i - 1) + " layers");`,
  },
  {
    id: "lorem",
    name: "Lorem ipsum",
    builtin: true,
    code: `// Fills the selected text layers with placeholder copy.
const words = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua".split(" ");
const sel = await pergel.selection();
let n = 0;
for (const layer of sel.selection) {
  if (layer.type !== "text") continue;
  const count = 4 + Math.floor(Math.random() * 8);
  const text = Array.from({ length: count }, () => words[Math.floor(Math.random() * words.length)]).join(" ");
  await pergel.update(layer.id, { text: text[0].toUpperCase() + text.slice(1) + "." });
  n++;
}
pergel.notify(n ? "Filled " + n + " text layers" : "Select text layers");`,
  },
  {
    id: "palette",
    name: "Random palette",
    builtin: true,
    code: `// Gives each selected shape a colour from a harmonious palette.
const sel = await pergel.selection();
const base = Math.random() * 360;
const hsl = (h, s, l) => {
  const a = s * Math.min(l, 1 - l);
  const f = (n) => { const k = (n + h / 30) % 12; return Math.round(255 * (l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1)))); };
  return "#" + [f(0), f(8), f(4)].map((v) => v.toString(16).padStart(2, "0")).join("");
};
let i = 0;
for (const layer of sel.selection) {
  if (layer.type === "text" || layer.type === "group") continue;
  await pergel.update(layer.id, { fill: hsl((base + i++ * 37) % 360, 0.7, 0.6) });
}
pergel.notify("Coloured " + i + " layers");`,
  },
  {
    id: "audit",
    name: "Audit: unnamed layers",
    builtin: true,
    code: `// Lists layers that still have default names (Rectangle 3, Frame 12…).
const xml = await pergel.metadata();
const names = [...xml.matchAll(/name="([^"]+)"/g)].map((m) => m[1]);
const generic = names.filter((n) => /^(Frame|Rectangle|Ellipse|Group|Vector|Line|Arrow|Polygon|Star|Section) \\d+$/.test(n));
console.log(generic.length + " of " + names.length + " layers have default names");
return generic;`,
  },
];

export function loadPlugins(): Plugin[] {
  try {
    const saved = JSON.parse(localStorage.getItem(KEY) ?? "[]") as Plugin[];
    return [...BUILTIN, ...saved.filter((p) => p && p.id && typeof p.code === "string")];
  } catch {
    return [...BUILTIN];
  }
}

export function savePlugins(list: Plugin[]) {
  try {
    localStorage.setItem(KEY, JSON.stringify(list.filter((p) => !p.builtin)));
  } catch {
    // Storage full or blocked: plugins stay for this session.
  }
}

/** Runs plugin code; `onLog` streams console output. Times out after `timeout` ms. */
export function runScript(code: string, onLog?: (line: string) => void, timeout = 60_000): Promise<RunResult> {
  const engine = editor.engine;
  const started = performance.now();
  const logs: string[] = [];
  if (!engine) return Promise.resolve({ ok: false, error: "The engine is still loading", logs, ms: 0 });
  const url = URL.createObjectURL(new Blob([WORKER], { type: "text/javascript" }));
  const worker = new Worker(url);
  engine.begin();
  return new Promise<RunResult>((resolve) => {
    let finished = false;
    const finish = (r: Omit<RunResult, "logs" | "ms">) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      worker.terminate();
      URL.revokeObjectURL(url);
      engine.commit();
      editor.refreshAll();
      resolve({ ...r, logs, ms: Math.round(performance.now() - started) });
    };
    const timer = setTimeout(() => finish({ ok: false, error: `Timed out after ${timeout / 1000} s` }), timeout);
    worker.onerror = (e) => finish({ ok: false, error: e.message || "Plugin error" });
    worker.onmessage = async (e: MessageEvent) => {
      const m = e.data as { type: string; id?: number; method?: string; args?: Record<string, unknown>; text?: string; ok?: boolean; result?: unknown; error?: string };
      if (m.type === "log" && m.text !== undefined) {
        logs.push(m.text);
        onLog?.(m.text);
      } else if (m.type === "done") {
        finish({ ok: !!m.ok, result: m.result, error: m.error });
      } else if (m.type === "call") {
        let reply: { ok: boolean; result?: unknown; error?: string };
        try {
          if (m.method === "notify") {
            editor.notify(String(m.args?.message ?? ""));
            reply = { ok: true, result: null };
          } else if (m.method === "selection") {
            reply = await runTool("get_selection", {});
          } else {
            const { name, args } = (m.args ?? {}) as { name: string; args: Record<string, unknown> };
            if (name === "run_script") throw new Error("Scripts can't start scripts");
            reply = await runTool(name, args ?? {});
          }
        } catch (err) {
          reply = { ok: false, error: String(err) };
        }
        if (!finished) worker.postMessage({ type: "reply", id: m.id, ...reply });
      }
    };
    worker.postMessage({ type: "run", code });
  });
}
