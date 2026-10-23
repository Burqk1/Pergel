import type { ExportSetting, NodeInfo } from "./engine";
import { editor } from "./editor.svelte";
import { saveAs } from "./files";

function toBase64(bytes: Uint8Array) {
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000) s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

/** "2x", "512w", "300h" for a setting (Figma's export size field). */
export function sizeLabel(e: ExportSetting) {
  if (e.width) return `${e.width}w`;
  if (e.height) return `${e.height}h`;
  return `${Math.round(e.scale * 100) / 100}x`;
}

export function parseSize(raw: string): Pick<ExportSetting, "scale" | "width" | "height"> | null {
  const s = raw.trim().toLowerCase();
  const v = parseFloat(s);
  if (!Number.isFinite(v) || v <= 0) return null;
  if (s.endsWith("w")) return { scale: 1, width: Math.round(v), height: null };
  if (s.endsWith("h")) return { scale: 1, width: null, height: Math.round(v) };
  return { scale: Math.min(4, v), width: null, height: null };
}

/** Exports one layer with one setting; returns false when the user cancelled. */
export async function exportLayer(n: NodeInfo, e: ExportSetting): Promise<boolean> {
  const engine = editor.engine;
  if (!engine) return false;
  const name = `${n.name}${e.suffix}`.replace(/[\\/:*?"<>|]/g, "_");
  if (e.format === "svg") return saveAs(name, "svg", { text: engine.svg(n.id) });
  if (e.format === "pdf") {
    const pdf = engine.pdf(n.id);
    if (!pdf) throw new Error("PDF export failed");
    return saveAs(name, "pdf", { base64: toBase64(pdf) });
  }
  const scale = e.width ? e.width / n.w : e.height ? e.height / n.h : e.scale;
  const shot = engine.capture(n.id, Math.max(1, Math.round(Math.max(n.w, n.h) * scale)));
  if (!shot) throw new Error("Nothing to export");
  if (e.format === "png") return saveAs(name, "png", { base64: shot.url.slice(shot.url.indexOf(",") + 1) });
  // JPG has no transparency: flatten onto white.
  const img = new Image();
  img.src = shot.url;
  await img.decode();
  const canvas = document.createElement("canvas");
  canvas.width = shot.width;
  canvas.height = shot.height;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#ffffff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img, 0, 0);
  const jpg = canvas.toDataURL("image/jpeg", 0.92);
  return saveAs(name, "jpg", { base64: jpg.slice(jpg.indexOf(",") + 1) });
}
