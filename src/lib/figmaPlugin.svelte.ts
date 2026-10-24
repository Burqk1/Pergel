import { editor } from "./editor.svelte";
import { Sandbox } from "./figmaSandbox";

// Figma plugin compatibility: plugins run in a sandbox (figmaSandbox.ts, a separate JavaScript
// engine) with a `figma` object with the plugin API's everyday parts (figmaPrelude.js). Every
// property read or write there is a call to `host` below, with JSON in and out, which runs an
// engine tool on Pergel's document. `figma.showUI` opens the plugin's HTML in a sandboxed
// iframe that talks through `pluginMessage`, as in Figma.

export interface FigmaPluginFiles {
  name: string;
  code: string;
  html: string;
}

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
let sandbox: Sandbox | null = null;
let pollSelection = 0;
let refreshQueued = false;
const timers = new Map<number, number>();

/** Tools that only read; the others refresh the UI after the plugin's code runs. */
const READS = new Set(["get_node_details", "list_children", "get_node", "get_selection", "get_metadata"]);

function queueRefresh() {
  if (refreshQueued) return;
  refreshQueued = true;
  queueMicrotask(() => {
    refreshQueued = false;
    editor.refreshAll();
  });
}

/** The app side of the plugin API: what `__host(op, args)` does in the sandbox. */
function host(name: string, done: (message?: string) => void) {
  const storageKey = (key: string) => `pergel.figma.${name}.${key}`;
  return (op: string, args: unknown): unknown => {
    const a = (args ?? {}) as Record<string, unknown>;
    const engine = editor.engine;
    if (!engine) throw new Error("The engine is still loading");
    switch (op) {
      case "agent": {
        const tool = String(a.tool);
        // Scripts and disk access are not part of the plugin API.
        if (tool === "run_script" || tool === "download_assets") throw new Error(`${tool} is not available to plugins`);
        const r = engine.agent(tool, (a.args ?? {}) as Record<string, unknown>);
        if (!r.ok) throw new Error(r.error ?? `${tool} failed`);
        if (!READS.has(tool)) queueRefresh();
        return r.result;
      }
      case "log":
        console.log(`[${name}]`, String(args));
        return null;
      case "notify":
        editor.notify(String(args));
        return null;
      case "showUI":
        Object.assign(figmaPlugin, {
          name: (a.title as string) ?? name,
          html: String(a.html ?? ""),
          width: Number(a.width) || 300,
          height: Number(a.height) || 200,
          visible: a.visible ?? true,
          open: true,
        });
        return null;
      case "ui":
        if (a.method === "postMessage") frame?.contentWindow?.postMessage({ pluginMessage: a.msg }, "*");
        else if (a.method === "close") figmaPlugin.open = false;
        else if (a.method === "resize") Object.assign(figmaPlugin, { width: Number(a.width) || figmaPlugin.width, height: Number(a.height) || figmaPlugin.height });
        else if (a.method === "show") figmaPlugin.visible = true;
        else if (a.method === "hide") figmaPlugin.visible = false;
        return null;
      case "close":
        // After the current call returns to the sandbox.
        setTimeout(() => done(args === null ? undefined : String(args)), 0);
        return null;
      case "pageName":
        return editor.pages.find((p) => p.current)?.name ?? "Page 1";
      case "fileName":
        return editor.fileName;
      case "selection":
        return editor.selection.map((n) => String(n.id));
      case "setSelection": {
        const r = engine.agent("select_nodes", { nodeIds: args as string[], zoom: false });
        editor.refreshSelection();
        return r.ok;
      }
      case "group": {
        engine.agent("select_nodes", { nodeIds: args as string[], zoom: false });
        editor.refreshSelection();
        editor.group();
        return String(editor.selection[0]?.id ?? (args as string[])[0]);
      }
      case "zoom":
        return editor.view.zoom;
      case "fonts":
        return editor.fonts.map((f) => f.name);
      case "storageGet":
        try {
          const v = localStorage.getItem(storageKey(String(args)));
          return v === null ? null : JSON.parse(v);
        } catch {
          return null;
        }
      case "storageSet":
        try {
          localStorage.setItem(storageKey(String(a.key)), JSON.stringify(a.value));
        } catch {
          // storage unavailable
        }
        return null;
      case "export": {
        const id = Number(a.id);
        if (a.format === "SVG") return { svg: engine.svg(id) ?? "" };
        const shot = engine.capture(id, 2048);
        if (!shot) throw new Error("Nothing to export");
        return { png: shot.url.slice(shot.url.indexOf(",") + 1) };
      }
      case "setTimeout": {
        const id = Number(a.id);
        clearTimeout(timers.get(id));
        timers.set(
          id,
          window.setTimeout(() => {
            timers.delete(id);
            inSandbox(() => sandbox?.callGlobal("__fire", id));
          }, Number(a.ms) || 0),
        );
        return null;
      }
      case "clearTimeout":
        clearTimeout(timers.get(Number(a.id)));
        timers.delete(Number(a.id));
        return null;
      default:
        throw new Error(`Unknown plugin call ${op}`);
    }
  };
}

/** Runs sandbox code for an event as one undo step, reporting what it throws. */
function inSandbox(f: () => void) {
  editor.engine?.begin();
  try {
    f();
  } catch (err) {
    editor.notify(`Plugin error: ${err instanceof Error ? err.message : String(err)}`);
  } finally {
    editor.engine?.commit();
    editor.refreshAll();
  }
}

/** The iframe of the open plugin's UI (set by the panel). */
export function attachFrame(el: HTMLIFrameElement | null) {
  frame = el;
}

window.addEventListener("message", (e) => {
  if (!frame || e.source !== frame.contentWindow || !sandbox) return;
  const msg = (e.data as { pluginMessage?: unknown } | null)?.pluginMessage;
  if (msg === undefined) return;
  // Each message the UI sends is one undo step.
  let json: string;
  try {
    json = JSON.stringify(msg);
  } catch {
    return;
  }
  inSandbox(() => sandbox?.callGlobal("__uiMessage", json));
});

let closer: (() => void) | null = null;

/** Runs a Figma plugin (its main code and UI). Resolves when it calls figma.closePlugin(). */
export async function runFigmaPlugin(p: FigmaPluginFiles): Promise<void> {
  closer?.();
  let finished = false;
  let resolveRun: () => void = () => {};
  const ended = new Promise<void>((r) => (resolveRun = r));
  const done = (message?: string) => {
    if (finished) return;
    finished = true;
    clearInterval(pollSelection);
    timers.forEach((t) => clearTimeout(t));
    timers.clear();
    try {
      sandbox?.callGlobal("__dispatch", "close");
    } catch {
      // closing anyway
    }
    sandbox?.dispose();
    sandbox = null;
    figmaPlugin.open = false;
    closer = null;
    if (message) editor.notify(message);
    editor.refreshAll();
    resolveRun();
  };
  closer = () => done();
  try {
    sandbox = await Sandbox.create(host(p.name, done));
  } catch (err) {
    editor.notify(`The plugin sandbox could not start: ${String(err)}`);
    done();
    return ended;
  }
  let last = editor.selection.map((n) => n.id).join(",");
  pollSelection = window.setInterval(() => {
    const now = editor.selection.map((n) => n.id).join(",");
    if (now !== last) {
      last = now;
      inSandbox(() => sandbox?.callGlobal("__dispatch", "selectionchange"));
    }
  }, 300);
  editor.engine?.begin();
  try {
    // Plugins without a UI that never close themselves end when their code settles.
    sandbox
      .run(p.code, p.html)
      .catch((err) => editor.notify(`Plugin error: ${err instanceof Error ? err.message : String(err)}`))
      .finally(() => {
        if (!figmaPlugin.open) setTimeout(() => done(), 0);
      });
  } catch (err) {
    editor.notify(`Plugin error: ${err instanceof Error ? err.message : String(err)}`);
    done();
  } finally {
    editor.engine?.commit();
    editor.refreshAll();
  }
  return ended;
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
