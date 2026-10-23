import { editor } from "./editor.svelte";
import { workspace } from "./workspace.svelte";
import type { FigmaReport } from "./engine";

// Figma import through Figma's REST API with the user's personal access token (Figma → Settings
// → Security → Personal access tokens, scope "File content: read"). The token stays on this
// machine and is only sent to api.figma.com.

const API = "https://api.figma.com/v1";
const TOKEN_KEY = "pergel.figmaToken";

export function savedToken(): string {
  try {
    return localStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

export function rememberToken(token: string, remember: boolean) {
  try {
    if (remember) localStorage.setItem(TOKEN_KEY, token);
    else localStorage.removeItem(TOKEN_KEY);
  } catch {
    // private mode
  }
}

/** The file key (and node id, when the link points at a layer) of a figma.com link. */
export function parseFigmaUrl(url: string): { key: string; node: string | null } | null {
  const m = url.trim().match(/figma\.com\/(?:design|file|proto)\/([A-Za-z0-9]+)(?:\/branch\/([A-Za-z0-9]+))?/);
  if (!m) return /^[A-Za-z0-9]{10,}$/.test(url.trim()) ? { key: url.trim(), node: null } : null;
  const node = url.match(/[?&]node-id=([^&]+)/)?.[1];
  return { key: m[2] ?? m[1], node: node ? decodeURIComponent(node).replace("-", ":") : null };
}

async function api<T>(path: string, token: string): Promise<T> {
  const res = await fetch(`${API}${path}`, { headers: { "X-Figma-Token": token } });
  if (res.status === 403) throw new Error("Figma refused the token (403). Check it has the \"File content: read\" scope.");
  if (res.status === 404) throw new Error("File not found (404). Check the link and that your account can open it.");
  if (!res.ok) throw new Error(`Figma API error ${res.status}: ${(await res.text()).slice(0, 200)}`);
  return (await res.json()) as T;
}

function base64(bytes: Uint8Array) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** Collects the image fill refs used in a node tree. */
function imageRefs(node: unknown, out: Set<string>) {
  if (!node || typeof node !== "object") return;
  const n = node as { fills?: { type?: string; imageRef?: string }[]; children?: unknown[] };
  for (const f of n.fills ?? []) if (f.type === "IMAGE" && f.imageRef) out.add(f.imageRef);
  for (const c of n.children ?? []) imageRefs(c, out);
}

export interface ImportProgress {
  step: string;
  done?: number;
  total?: number;
}

/**
 * Downloads a Figma file (all pages, or the layer the link points at), converts it and opens it
 * as a new Pergel file. Returns the import report.
 */
export async function importFigma(url: string, token: string, progress: (p: ImportProgress) => void): Promise<FigmaReport> {
  const engine = editor.engine;
  if (!engine) throw new Error("The engine is still loading");
  const target = parseFigmaUrl(url);
  if (!target) throw new Error("That is not a Figma file link (figma.com/design/…)");
  progress({ step: "Downloading the file from Figma…" });
  type FileJson = { name: string; document: { children: unknown[] } };
  let file: FileJson;
  if (target.node) {
    // Just the linked layer, on a page of its own.
    const r = await api<{ name: string; nodes: Record<string, { document: unknown } | null> }>(
      `/files/${target.key}/nodes?ids=${encodeURIComponent(target.node)}&geometry=paths`,
      token,
    );
    const doc = Object.values(r.nodes)[0]?.document;
    if (!doc) throw new Error("The linked layer was not found in the file");
    const isPage = (doc as { type?: string }).type === "CANVAS";
    file = { name: r.name, document: { children: isPage ? [doc] : [{ type: "CANVAS", name: "Imported", children: [doc] }] } };
  } else {
    file = await api<FileJson>(`/files/${target.key}?geometry=paths`, token);
  }
  const refs = new Set<string>();
  imageRefs(file.document, refs);
  const images: Record<string, string> = {};
  if (refs.size) {
    progress({ step: "Downloading images…", done: 0, total: refs.size });
    const { meta } = await api<{ meta: { images: Record<string, string | null> } }>(`/files/${target.key}/images`, token);
    const todo = [...refs].filter((r) => meta.images[r]);
    let done = 0;
    const worker = async () => {
      while (todo.length) {
        const ref = todo.pop();
        if (!ref) break;
        try {
          const res = await fetch(meta.images[ref] as string);
          if (res.ok) images[ref] = base64(new Uint8Array(await res.arrayBuffer()));
        } catch {
          // counted as missing in the report
        }
        progress({ step: "Downloading images…", done: ++done, total: refs.size });
      }
    };
    await Promise.all(Array.from({ length: 6 }, worker));
  }
  progress({ step: "Converting…" });
  const out = engine.figmaDocument(file, images);
  if (out.error || !out.document || !out.report) throw new Error(out.error ?? "Conversion failed");
  progress({ step: "Opening…" });
  await workspace.createFrom(file.name || "Figma import", JSON.stringify(out.document));
  return out.report;
}
