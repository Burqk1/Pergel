import { invoke, isTauri } from "@tauri-apps/api/core";
import { editor } from "./editor.svelte";
import { saveAs } from "./files";
import { workspace } from "./workspace.svelte";

// Slides and Sites, Pergel-style: a deck is a file of 16:9 frames (Present plays it, one PDF
// exports it); a site is the page's top-level frames written out as linked HTML pages.

const W = 1920;
const H = 1080;

function slide(i: number, title: string, body: string, dark = false) {
  return {
    name: `Slide ${i + 1}`,
    kind: "Frame",
    bounds: { x: i * (W + 100), y: 0, w: W, h: H },
    fill: dark ? 0xff111827 : 0xffffffff,
    is_main: false,
    children: [
      {
        name: title,
        kind: "Text",
        bounds: { x: i * (W + 100) + 160, y: 380, w: 1600, h: 140 },
        fill: dark ? 0xffffffff : 0xff111827,
        text: { content: title, family: "Inter", weight: 700, size: 112, resize: "auto-height" },
      },
      {
        name: body,
        kind: "Text",
        bounds: { x: i * (W + 100) + 160, y: 560, w: 1600, h: 60 },
        fill: dark ? 0xffcbd5e1 : 0xff4b5563,
        text: { content: body, family: "Inter", weight: 400, size: 44, resize: "auto-height" },
      },
    ],
  };
}

/** A new file with a 3-slide 16:9 deck to start from. */
export async function newSlideDeck() {
  const doc = {
    format: "pergel/document",
    version: 2,
    pages: [
      {
        name: "Slides",
        layers: [
          slide(0, "Title of the talk", "Your name · date", true),
          slide(1, "One idea per slide", "Add frames at 1920 × 1080 for more slides; Present (▷) plays them left to right."),
          slide(2, "Thank you", "Questions?", true),
        ],
      },
    ],
  };
  await workspace.createFrom("Slide deck", JSON.stringify(doc));
  editor.notify("New deck: Present (Ctrl+Alt+Enter) plays it, Export slides as PDF saves it");
}

/** The page's top-level frames in reading order (rows top to bottom, then left to right). */
function frames() {
  const list = editor.engine?.screens() ?? [];
  const pos = new Map<number, { x: number; y: number }>();
  for (const s of list) {
    const n = editor.engine?.agent("get_node", { nodeId: String(s.id) });
    const r = n?.ok ? (n.result as { absX: number; absY: number }) : { absX: 0, absY: 0 };
    pos.set(s.id, { x: r.absX, y: r.absY });
  }
  return [...list].sort((a, b) => {
    const pa = pos.get(a.id)!;
    const pb = pos.get(b.id)!;
    return Math.abs(pa.y - pb.y) > 200 ? pa.y - pb.y : pa.x - pb.x;
  });
}

function base64(bytes: Uint8Array) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** Every top-level frame as one page of a PDF. */
export async function exportSlidesPdf() {
  const engine = editor.engine;
  const list = frames();
  if (!engine || !list.length) return editor.notify("No frames to export");
  await editor.ensureDocFonts();
  const bytes = engine.pdfPages(list.map((f) => f.id));
  if (!bytes) return editor.notify("Could not make the PDF");
  if (await saveAs(editor.fileName || "Slides", "pdf", { base64: base64(bytes) })) editor.notify(`Exported ${list.length} slide${list.length === 1 ? "" : "s"}`);
}

const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "page";

function page(title: string, body: string, nav: string, fonts: string[]) {
  const families = fonts.map((f) => `family=${encodeURIComponent(f)}:wght@100..900`).join("&");
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${title.replace(/</g, "&lt;")}</title>
${fonts.length ? `<link rel="stylesheet" href="https://fonts.googleapis.com/css2?${families}&display=swap">` : ""}
<style>body{margin:0;background:#f3f4f6;font-family:Inter,system-ui,sans-serif}main{display:flex;justify-content:center;padding:32px 0}nav{display:flex;gap:16px;justify-content:center;padding:12px;font:14px system-ui}nav a{color:#2563eb;text-decoration:none}</style>
</head>
<body>
${nav}
<main>
${body}
</main>
</body>
</html>
`;
}

/** Publishes the page's top-level frames as a static site (index.html + one page per frame). */
export async function publishSite() {
  const engine = editor.engine;
  const list = frames();
  if (!engine || !list.length) return editor.notify("No frames to publish");
  await editor.ensureDocFonts();
  const files: { path: string; text?: string; base64?: string }[] = [];
  const names = list.map((f, i) => (i === 0 ? "index.html" : `${slug(f.name)}.html`));
  const nav = list.length > 1 ? `<nav>${list.map((f, i) => `<a href="${names[i]}">${f.name.replace(/</g, "&lt;")}</a>`).join("")}</nav>` : "";
  const assets = new Map<string, string>();
  list.forEach((f, i) => {
    const r = engine.agent("get_design_context", { nodeId: String(f.id), framework: "html", assetBase: "assets/" });
    if (!r.ok) return;
    const ctx = r.result as { code: string; fonts?: string[]; assets?: { file: string; data?: string }[] };
    for (const a of ctx.assets ?? []) if (a.data) assets.set(a.file, a.data);
    files.push({ path: names[i], text: page(f.name, ctx.code, nav, ctx.fonts ?? []) });
  });
  for (const [file, data] of assets) files.push({ path: `assets/${file}`, base64: data });
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const dir = await open({ directory: true, title: "Folder for the site" });
    if (!dir || Array.isArray(dir)) return;
    const sep = dir.includes("\\") ? "\\" : "/";
    for (const f of files) await invoke("write_file", { path: `${dir}${sep}${f.path.replace(/\//g, sep)}`, base64: f.base64 ?? null, text: f.text ?? null });
    editor.notify(`Published ${list.length} page${list.length === 1 ? "" : "s"} to ${dir}`);
    return files.length;
  }
  // Browser: one self-contained page (assets inlined).
  let html = files[0]?.text ?? "";
  for (const [file, data] of assets) {
    const mime = file.endsWith(".svg") ? "image/svg+xml" : file.endsWith(".jpg") ? "image/jpeg" : "image/png";
    html = html.split(`assets/${file}`).join(`data:${mime};base64,${data}`);
  }
  await saveAs(editor.fileName || "site", "html", { text: html });
  return files.length;
}
