/** Colour maths for the colour picker (all channels 0–1 unless noted). */

export interface Hsv {
  h: number; // 0–360
  s: number;
  v: number;
}

export function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace("#", "").padEnd(6, "0");
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as [number, number, number];
}

export function rgbToHex([r, g, b]: [number, number, number]): string {
  const c = (v: number) =>
    Math.round(Math.min(1, Math.max(0, v)) * 255)
      .toString(16)
      .padStart(2, "0");
  return `#${c(r)}${c(g)}${c(b)}`.toUpperCase();
}

export function rgbToHsv([r, g, b]: [number, number, number]): Hsv {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const d = max - min;
  let h = 0;
  if (d) {
    if (max === r) h = ((g - b) / d) % 6;
    else if (max === g) h = (b - r) / d + 2;
    else h = (r - g) / d + 4;
    h *= 60;
    if (h < 0) h += 360;
  }
  return { h, s: max ? d / max : 0, v: max };
}

export function hsvToRgb({ h, s, v }: Hsv): [number, number, number] {
  const c = v * s;
  const x = c * (1 - Math.abs(((h / 60) % 2) - 1));
  const m = v - c;
  const [r, g, b] =
    h < 60 ? [c, x, 0] : h < 120 ? [x, c, 0] : h < 180 ? [0, c, x] : h < 240 ? [0, x, c] : h < 300 ? [x, 0, c] : [c, 0, x];
  return [r + m, g + m, b + m];
}

export function rgbToHsl([r, g, b]: [number, number, number]): [number, number, number] {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  const d = max - min;
  const s = d ? d / (1 - Math.abs(2 * l - 1)) : 0;
  return [rgbToHsv([r, g, b]).h, s, l];
}

export function hslToRgb([h, s, l]: [number, number, number]): [number, number, number] {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const v = l + c / 2;
  return hsvToRgb({ h, s: v ? (2 * (v - l)) / v : 0, v });
}

export function parseHex(raw: string): string | null {
  const m = /^#?([0-9a-f]{3}|[0-9a-f]{6})$/i.exec(raw.trim());
  if (!m) return null;
  const h = m[1].length === 3 ? m[1].replace(/./g, (c) => c + c) : m[1];
  return `#${h.toUpperCase()}`;
}

/** Packed ARGB (as the engine stores colours) to "#RRGGBB". */
export function argbHex(argb: number): string {
  return `#${((argb >>> 0) & 0xffffff).toString(16).padStart(6, "0").toUpperCase()}`;
}

/** Alpha of packed ARGB in percent. */
export function argbAlpha(argb: number): number {
  return Math.round((((argb >>> 0) >>> 24) / 255) * 100);
}

export function packArgb(hex: string, alpha: number): number {
  const rgb = parseInt(hex.replace("#", "").padEnd(6, "0").slice(0, 6), 16);
  const a = Math.round((Math.min(100, Math.max(0, alpha)) / 100) * 255);
  return ((a << 24) | rgb) >>> 0;
}

/** "rgba(…)" for packed ARGB. */
export function argbCss(argb: number): string {
  const v = argb >>> 0;
  return `rgba(${(v >> 16) & 255}, ${(v >> 8) & 255}, ${v & 255}, ${((v >>> 24) / 255).toFixed(3)})`;
}
