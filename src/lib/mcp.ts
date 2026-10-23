import { invoke, isTauri } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { PAGE } from "./engine";
import { editor } from "./editor.svelte";
import { runScript } from "./plugins";
import { workspace } from "./workspace.svelte";

/** Tools that change the document; the UI refreshes after them. */
const MUTATING = new Set(["import_figma", "use_library_component", "add_comment", "resolve_comment", "generate_diagram", "create_nodes", "update_node", "delete_nodes", "select_nodes", "create_instance", "switch_page", "set_variables", "bind_variable", "set_variant", "set_interactions", "add_code_connect_map"]);
const ICONIFY = "https://api.iconify.design";

export interface McpStatus {
  url: string;
  running: boolean;
  error: string | null;
}

type Reply = { ok: boolean; result?: unknown; error?: string };
type Spec = Record<string, unknown> & { children?: Spec[] };

function parseId(v: unknown): number | null {
  if (typeof v === "number") return v;
  if (typeof v !== "string") return null;
  const n = Number(v.split(/[:-]/).pop());
  return Number.isFinite(n) ? n : null;
}

function screenshot(args: Record<string, unknown>): Reply {
  const engine = editor.engine;
  if (!engine) return { ok: false, error: "The engine is still loading" };
  const requested = parseId(args.nodeId);
  const id = requested ?? editor.selection[0]?.id ?? PAGE;
  const max = Math.min(2048, Math.max(64, Number(args.maxSize) || 1024));
  const shot = engine.capture(id, max);
  if (!shot) return { ok: false, error: requested === null ? "Nothing to capture" : `No layer with id ${requested}` };
  const label =
    id === PAGE ? "the whole page" : (editor.layers.find((l) => l.id === id)?.name ?? `layer ${id}`);
  return {
    ok: true,
    result: { png: shot.url.slice(shot.url.indexOf(",") + 1), width: shot.width, height: shot.height, label },
  };
}

function base64(bytes: Uint8Array) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** An Iconify icon as SVG markup ("lucide:arrow-right", "mdi-home" or "logos:github-icon"). */
export async function fetchIcon(id: string): Promise<string> {
  const m = id.trim().match(/^([a-z0-9-]+?)[:/]([a-z0-9-]+)$/i);
  if (!m) throw new Error(`"${id}" is not an icon id; use "set:name" such as "lucide:arrow-right" (see search_icons)`);
  try {
    const res = await fetch(`${ICONIFY}/${m[1]}/${m[2]}.svg?height=none`);
    const svg = res.ok ? await res.text() : "";
    if (svg.includes("<svg")) return svg;
  } catch {
    // fall back to the JSON API below
  }
  // The icon set's JSON: build the SVG from the icon body.
  const res = await fetch(`${ICONIFY}/${m[1]}.json?icons=${m[2]}`);
  const data = res.ok ? ((await res.json()) as { width?: number; height?: number; icons?: Record<string, { body: string; width?: number; height?: number }>; aliases?: Record<string, { parent: string }> }) : null;
  const name = data?.icons?.[m[2]] ? m[2] : data?.aliases?.[m[2]]?.parent;
  const icon = name ? data?.icons?.[name] : undefined;
  if (!icon) throw new Error(`Icon "${id}" was not found; find ids with search_icons`);
  const w = icon.width ?? data?.width ?? 16;
  const h = icon.height ?? data?.height ?? 16;
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}">${icon.body}</svg>`;
}

/** Fetches what the engine cannot: icon ids and image URLs in node specs. */
async function resolveSpecs(specs: Spec[], warnings: string[]) {
  for (const spec of specs) {
    if (Array.isArray(spec.children)) await resolveSpecs(spec.children, warnings);
    const type = String(spec.type ?? "").toLowerCase();
    if (typeof spec.icon === "string" && !spec.svg) {
      try {
        spec.svg = await fetchIcon(spec.icon);
        spec.name ??= spec.icon.split(/[:/]/).pop();
      } catch (e) {
        warnings.push(String(e instanceof Error ? e.message : e));
      }
    }
    const src = typeof spec.src === "string" ? spec.src : null;
    if (src && !spec.image && !spec.svg && /^(https?:|data:)/.test(src)) {
      try {
        const res = await fetch(src);
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const bytes = new Uint8Array(await res.arrayBuffer());
        const isSvg = type === "svg" || (res.headers.get("content-type") ?? "").includes("svg") || /\.svg(\?|$)/i.test(src);
        if (isSvg) spec.svg = new TextDecoder().decode(bytes);
        else spec.image = base64(bytes);
      } catch (e) {
        warnings.push(`Could not download ${src} (${e}); save it to a file and pass its path as "src"`);
      }
    }
  }
}

export async function searchIcons(args: Record<string, unknown>): Promise<Reply> {
  const q = String(args.query ?? "").trim();
  if (!q) return { ok: false, error: "Pass a query" };
  const limit = Math.min(100, Math.max(1, Number(args.limit) || 24));
  const params = new URLSearchParams({ query: q, limit: String(Math.max(limit, 32)) });
  if (typeof args.prefixes === "string" && args.prefixes) params.set("prefixes", args.prefixes);
  try {
    const res = await fetch(`${ICONIFY}/search?${params}`);
    const data = (await res.json()) as {
      icons?: string[];
      total?: number;
      collections?: Record<string, { name?: string; license?: { title?: string } }>;
    };
    const icons = (data.icons ?? []).slice(0, limit);
    const sets = Object.fromEntries(
      Object.entries(data.collections ?? {}).map(([k, v]) => [k, `${v.name ?? k}${v.license?.title ? ` (${v.license.title})` : ""}`]),
    );
    return {
      ok: true,
      result: { icons, total: data.total ?? icons.length, sets, use: '{ "type": "icon", "icon": "<id>", "width": 24, "color": "#111827" }' },
    };
  } catch (e) {
    return { ok: false, error: `Icon search needs an internet connection (${e})` };
  }
}

async function loadImage(b64: string): Promise<ImageBitmap> {
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  return createImageBitmap(new Blob([bytes]));
}

function canvas(w: number, h: number) {
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) throw new Error("No 2D canvas");
  return { c, ctx };
}

/** Pixel comparison of an implementation screenshot with the rendered design. */
async function compare(args: Record<string, unknown>): Promise<Reply> {
  const engine = editor.engine;
  if (!engine) return { ok: false, error: "The engine is still loading" };
  const info = engine.agent("get_node", { nodeId: args.nodeId });
  if (!info.ok) return info;
  const node = info.result as { id: string; name: string; width: number; height: number; absX: number; absY: number };
  let shot: ImageBitmap;
  try {
    shot = await loadImage(String(args.image ?? ""));
  } catch {
    return { ok: false, error: "The screenshot is not a PNG or JPEG image" };
  }
  const scale = shot.width / Math.max(1, node.width);
  const design = engine.capture(Number(node.id), Math.round(Math.max(node.width, node.height) * scale));
  if (!design) return { ok: false, error: "Could not render the design" };
  const designImg = await loadImage(design.url.slice(design.url.indexOf(",") + 1));
  const w = shot.width;
  const designH = Math.round(node.height * scale);
  const h = Math.min(shot.height, designH);
  const draw = (img: ImageBitmap, dh: number) => {
    const { c, ctx } = canvas(w, h);
    ctx.fillStyle = "#fff";
    ctx.fillRect(0, 0, w, h);
    ctx.drawImage(img, 0, 0, w, dh);
    return { c, data: ctx.getImageData(0, 0, w, h).data };
  };
  const a = draw(designImg, designH);
  const b = draw(shot, shot.height);
  const diff = canvas(w, h);
  const out = diff.ctx.createImageData(w, h);
  const GRID = 12;
  const cells = new Float64Array(GRID * GRID);
  const cellPx = new Float64Array(GRID * GRID);
  let bad = 0;
  for (let y = 0; y < h; y++) {
    const row = Math.min(GRID - 1, Math.floor((y / h) * GRID)) * GRID;
    for (let x = 0; x < w; x++) {
      const i = (y * w + x) * 4;
      // Anti-aliasing and sub-pixel text placement differ between renderers: a pixel only
      // counts as different when no neighbour within 1 px matches it.
      let d = 255;
      for (let dy = -1; dy <= 1 && d > 40; dy++) {
        const yy = y + dy;
        if (yy < 0 || yy >= h) continue;
        for (let dx = -1; dx <= 1 && d > 40; dx++) {
          const xx = x + dx;
          if (xx < 0 || xx >= w) continue;
          const j = (yy * w + xx) * 4;
          d = Math.min(
            d,
            Math.max(Math.abs(a.data[j] - b.data[i]), Math.abs(a.data[j + 1] - b.data[i + 1]), Math.abs(a.data[j + 2] - b.data[i + 2])),
          );
        }
      }
      const cell = row + Math.min(GRID - 1, Math.floor((x / w) * GRID));
      cellPx[cell]++;
      if (d > 40) {
        bad++;
        cells[cell]++;
        out.data[i] = 255;
        out.data[i + 1] = 40;
        out.data[i + 2] = 40;
        out.data[i + 3] = 255;
      } else {
        const grey = (b.data[i] + b.data[i + 1] + b.data[i + 2]) / 3;
        out.data[i] = out.data[i + 1] = out.data[i + 2] = grey;
        out.data[i + 3] = 70;
      }
    }
  }
  diff.ctx.putImageData(out, 0, 0);
  const match = 100 * (1 - bad / Math.max(1, w * h));

  // Where it differs: the worst cells, mapped back to design layers.
  const worst = [...cells.keys()]
    .map((k) => ({ k, r: cells[k] / Math.max(1, cellPx[k]) }))
    .filter((c) => c.r > 0.04)
    .sort((p, q) => q.r - p.r)
    .slice(0, 6);
  const points = worst.map(({ k }) => {
    const cx = ((k % GRID) + 0.5) / GRID;
    const cy = (Math.floor(k / GRID) + 0.5) / GRID;
    return [node.absX + cx * node.width, node.absY + (cy * h) / scale];
  });
  const hits =
    (engine.agent("layers_at", { nodeId: node.id, points }).result as
      | { layers: ({ id: string; name: string; type: string } | null)[] }
      | undefined)?.layers ?? [];
  const lines = worst.map((c, i) => {
    const l = hits[i];
    const [px, py] = points[i];
    const where = `around x ${Math.round(px - node.absX)}, y ${Math.round(py - node.absY)}`;
    return `- ${Math.round(c.r * 100)}% of pixels differ ${where}${l ? ` — layer "${l.name}" (${l.type}, id ${l.id})` : ""}`;
  });
  const notes: string[] = [];
  if (Math.abs(shot.height - designH) > Math.max(4, designH * 0.02)) {
    notes.push(
      `Height differs: the design is ${Math.round(node.height)} px tall, your screenshot is ${Math.round(shot.height / scale)} px at the same width (only the overlapping part was compared).`,
    );
  }
  const report = [
    `Match: ${match.toFixed(1)}% of pixels within tolerance (${match >= 95 ? "good" : "keep fixing"}).`,
    `Design "${node.name}" ${Math.round(node.width)}×${Math.round(node.height)} px; screenshot ${shot.width}×${shot.height} px (scale ${Math.round(scale * 100) / 100}x).`,
    ...notes,
    lines.length ? `Biggest differences:\n${lines.join("\n")}` : "No region stands out.",
    "Check them in the side-by-side image: wrong or missing icons/images (use the design's asset files), font family/weight/size, spacing and colours.",
  ].join("\n");

  // Side by side, at most 1800 px wide.
  const k = Math.min(1, 1800 / (w * 3 + 16));
  const sw = Math.round(w * k);
  const sh = Math.round(h * k);
  const side = canvas(sw * 3 + 16, sh);
  side.ctx.fillStyle = "#1e1e1e";
  side.ctx.fillRect(0, 0, side.c.width, sh);
  side.ctx.drawImage(a.c, 0, 0, sw, sh);
  side.ctx.drawImage(b.c, sw + 8, 0, sw, sh);
  side.ctx.fillStyle = "#fff";
  side.ctx.fillRect(sw * 2 + 16, 0, sw, sh);
  side.ctx.drawImage(b.c, sw * 2 + 16, 0, sw, sh);
  side.ctx.drawImage(diff.c, sw * 2 + 16, 0, sw, sh);
  const url = side.c.toDataURL("image/png");
  return { ok: true, result: { match, report, sideBySide: url.slice(url.indexOf(",") + 1) } };
}

async function handle(tool: string, args: Record<string, unknown>): Promise<Reply> {
  const engine = editor.engine;
  if (!engine) return { ok: false, error: "The engine is still loading" };
  // Renders must use the design's real fonts.
  if (tool === "get_screenshot" || tool === "compare_to_design" || tool === "get_design_context") await editor.ensureDocFonts();
  if (tool === "get_screenshot") return screenshot(args);
  if (tool === "search_icons") return searchIcons(args);
  if (tool === "compare_to_design") return compare(args);
  if (tool === "import_figma") {
    // A design someone shared as a figma.com link: into a new Pergel file (token saved in the app).
    const { importFigma, savedToken } = await import("./figma");
    const token = String(args.token ?? "") || savedToken();
    if (!token) return { ok: false, error: "No Figma access token saved. In Pergel: File → Import from Figma, paste a personal access token (figma.com → Settings → Security) and tick Remember, then try again." };
    try {
      const report = await importFigma(String(args.url ?? ""), token, () => {});
      editor.refreshAll();
      return { ok: true, result: { report, note: "Opened in a new tab. Call get_metadata / get_design_context to work with it." } };
    } catch (e) {
      return { ok: false, error: String(e instanceof Error ? e.message : e) };
    }
  }
  if (tool === "get_libraries") {
    const { libraries } = await import("./libraries.svelte");
    libraries.refresh();
    return {
      ok: true,
      result: {
        libraries: Object.values(libraries.all).map((l) => ({ id: l.id, name: l.name, version: l.version, components: l.components.map((c) => ({ key: c.key, name: c.name, set: c.set, width: c.w, height: c.h })) })),
        used: libraries.refs,
        updates: libraries.updates.length,
        hint: "Place one with use_library_component { library, component }; publish this file's components with publish_library.",
      },
    };
  }
  if (tool === "use_library_component") {
    const { libraries } = await import("./libraries.svelte");
    const want = String(args.library ?? "").toLowerCase();
    const lib = Object.values(libraries.all).find((l) => l.id === args.library || l.name.toLowerCase() === want) ?? (want ? undefined : Object.values(libraries.all)[0]);
    if (!lib) return { ok: false, error: `No library ${args.library ?? ""} (see get_libraries)` };
    const at = typeof args.x === "number" && typeof args.y === "number" ? { x: args.x as number, y: args.y as number } : undefined;
    const id = libraries.use(lib.id, String(args.component ?? ""), at);
    if (id < 0) return { ok: false, error: `No component ${args.component} in ${lib.name}` };
    return { ok: true, result: { instanceId: String(id), library: lib.name } };
  }
  if (tool === "publish_library") {
    const { libraries } = await import("./libraries.svelte");
    const n = libraries.publish();
    return n ? { ok: true, result: { published: n } } : { ok: false, error: "No components to publish" };
  }
  if (tool === "create_new_file") {
    // A fresh design file in a new tab; later tools work on it, the user's other files stay as they are.
    const name = String(args.name ?? "").trim() || "Untitled";
    await workspace.newFile(name);
    editor.refreshAll();
    return { ok: true, result: { name, note: "A new empty file is open in a new tab; build in it with create_nodes, generate_diagram or run_script." } };
  }
  if (tool === "run_script") {
    // Figma's use_figma: one script, many operations, one undo step.
    const r = await runScript(String(args.code ?? ""), undefined, Math.min(300_000, Number(args.timeoutMs) || 60_000));
    const logs = r.logs.length ? `\nLogs:\n${r.logs.join("\n")}` : "";
    return r.ok ? { ok: true, result: { result: r.result ?? null, logs: r.logs, ms: r.ms } } : { ok: false, error: `${r.error}${logs}` };
  }
  if (editor.editing >= 0) editor.finishEditing();
  const warnings: string[] = [];
  if (tool === "create_nodes" && Array.isArray(args.nodes)) await resolveSpecs(args.nodes as Spec[], warnings);
  const reply = engine.agent(tool, args);
  if (warnings.length && reply.ok && reply.result && typeof reply.result === "object") {
    const r = reply.result as { warnings?: string[] };
    r.warnings = [...warnings, ...(r.warnings ?? [])];
  }
  if (tool === "create_nodes" || tool === "update_node" || tool === "generate_diagram") await editor.ensureDocFonts();
  if (MUTATING.has(tool)) {
    editor.refreshAll();
    if (editor.leftTab === "assets") editor.refreshComponents();
  }
  return reply;
}

/** Answers MCP tool calls from Pergel's built-in server (desktop app only). */
export async function startMcpBridge() {
  if (!isTauri()) return;
  await listen<{ id: number; tool: string; args: Record<string, unknown> }>("mcp-request", async (event) => {
    const { id, tool, args } = event.payload;
    let reply: Reply;
    try {
      reply = await handle(tool, args ?? {});
    } catch (e) {
      reply = { ok: false, error: String(e) };
    }
    // The Agents panel's activity log.
    const target = args?.nodeId ?? (Array.isArray(args?.nodeIds) ? (args.nodeIds as unknown[]).join(", ") : "");
    const detail = reply.ok ? (target ? `layer ${target}` : "") : (reply.error ?? "failed");
    editor.mcpLog = [{ time: Date.now(), tool, ok: reply.ok, detail }, ...editor.mcpLog].slice(0, 200);
    await invoke("mcp_respond", { id, reply });
  });
  editor.mcp = await invoke<McpStatus | null>("mcp_status");
  if (!editor.mcp) setTimeout(async () => (editor.mcp = await invoke<McpStatus | null>("mcp_status")), 1500);
}

/** Runs a tool the way the MCP server does (dev checks and in-app features). */
export const runTool = handle;
