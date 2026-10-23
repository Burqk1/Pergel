import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Engine } from "./engine";

// Fonts beyond the bundled Inter: the computer's installed fonts (desktop app), then open-source
// fonts from Fontsource (Google Fonts and more) for families the computer doesn't have.

export interface SystemFamily {
  name: string;
  weights: number[];
  italic: boolean;
}

let systemList: Promise<SystemFamily[]> | null = null;
const loaded = new Set<string>();
const failed = new Set<string>();

/** Installed font families (empty in the browser). */
export function systemFonts(): Promise<SystemFamily[]> {
  systemList ??= isTauri() ? invoke<SystemFamily[]>("system_fonts").catch(() => []) : Promise.resolve([]);
  return systemList;
}

async function loadSystem(engine: Engine, family: string): Promise<boolean> {
  try {
    const data = new Uint8Array(await invoke<ArrayBuffer>("system_font_data", { family }));
    let ok = false;
    let at = 0;
    const view = new DataView(data.buffer, data.byteOffset, data.byteLength);
    while (at + 4 <= data.length) {
      const len = view.getUint32(at, true);
      at += 4;
      // Registered under the family name the document uses, so text finds it.
      ok = engine.addFont(data.subarray(at, at + len), family) || ok;
      at += len;
    }
    return ok;
  } catch {
    return false;
  }
}

const slug = (family: string) => family.trim().toLowerCase().replace(/\s+/g, "-");

/** A family from Fontsource: the variable font when there is one, else the static weights. */
async function loadFontsource(engine: Engine, family: string, weights: number[], italic: boolean): Promise<boolean> {
  const base = `https://cdn.jsdelivr.net/fontsource/fonts/${slug(family)}`;
  const get = async (url: string) => {
    try {
      const res = await fetch(url);
      return res.ok ? new Uint8Array(await res.arrayBuffer()) : null;
    } catch {
      return null;
    }
  };
  const styles = italic ? ["normal", "italic"] : ["normal"];
  let ok = false;
  for (const style of styles) {
    const variable = await get(`${base}:vf@latest/latin-wght-${style}.ttf`);
    if (variable && engine.addFont(variable, family)) {
      ok = true;
      continue;
    }
    const wanted = [...new Set([...weights, 400, 700])];
    const files = await Promise.all(wanted.map((w) => get(`${base}@latest/latin-${w}-${style}.ttf`)));
    for (const f of files) if (f && engine.addFont(f, family)) ok = true;
  }
  return ok;
}

/**
 * Loads every font the document uses that isn't available yet. Returns the families that
 * were loaded (callers refresh the UI when it is not empty) and those still missing.
 */
export async function ensureFonts(engine: Engine): Promise<{ loaded: string[]; missing: string[] }> {
  const need = engine.usedFonts().filter((f) => !f.available && !failed.has(f.family.toLowerCase()));
  if (!need.length) return { loaded: [], missing: [] };
  const installed = await systemFonts();
  const done: string[] = [];
  const missing: string[] = [];
  for (const f of need) {
    const key = f.family.toLowerCase();
    if (loaded.has(key)) continue;
    const sys = installed.find((s) => s.name.toLowerCase() === key);
    const ok = (sys && (await loadSystem(engine, f.family))) || (await loadFontsource(engine, f.family, f.weights, f.italic));
    if (ok) {
      loaded.add(key);
      done.push(f.family);
    } else {
      failed.add(key);
      missing.push(f.family);
    }
  }
  return { loaded: done, missing };
}

/** Loads one family on demand (font picker). */
export async function loadFamily(engine: Engine, family: string): Promise<boolean> {
  const key = family.toLowerCase();
  if (loaded.has(key) || engine.fonts().some((f) => f.name.toLowerCase() === key)) return true;
  const installed = await systemFonts();
  const sys = installed.find((s) => s.name.toLowerCase() === key);
  const ok = (sys && (await loadSystem(engine, family))) || (await loadFontsource(engine, family, [400, 700], true));
  if (ok) loaded.add(key);
  return !!ok;
}
