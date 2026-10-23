import type { NodeInfo } from "./engine";

// CSS / Tailwind for one layer's own box and text styles (Dev Mode panel, "Copy as code ▸ CSS").

export const px = (v: number) => `${Math.round(v * 100) / 100}px`;

export function lineHeightCss(t: NonNullable<NodeInfo["text"]>) {
  const l = t.lineHeight;
  return l.unit === "auto" ? "normal" : l.unit === "percent" ? `${l.value}%` : px(l.value);
}

export function textCss(n: NodeInfo, color: string) {
  const t = n.text!;
  const family = t.family.includes(" ") ? `"${t.family}"` : t.family;
  const lines = [
    `color: ${color};`,
    `font-family: ${family}, sans-serif;`,
    `font-size: ${px(t.size)};`,
    `font-weight: ${t.weight};`,
    `line-height: ${lineHeightCss(t)};`,
  ];
  if (t.italic) lines.push("font-style: italic;");
  if (t.letterSpacing) lines.push(`letter-spacing: ${Math.round(t.letterSpacing * 100) / 10000}em;`);
  if (t.align !== "left") lines.push(`text-align: ${t.align};`);
  if (t.decoration !== "none") lines.push(`text-decoration: ${t.decoration === "underline" ? "underline" : "line-through"};`);
  if (t.case !== "original") {
    lines.push(`text-transform: ${{ upper: "uppercase", lower: "lowercase", title: "capitalize" }[t.case]};`);
  }
  if (t.resize !== "auto-width") lines.push(`width: ${px(n.w)};`);
  return lines.join("\n");
}

export function css(n: NodeInfo) {
  const alpha =
    n.fillOpacity < 100 ? Math.round((n.fillOpacity / 100) * 255).toString(16).padStart(2, "0").toUpperCase() : "";
  if (n.text) return textCss(n, `${n.fill}${alpha}`);
  const lines = [`width: ${px(n.w)};`, `height: ${px(n.h)};`];
  lines.push(`background: ${n.fill}${alpha};`);
  if (n.kind === "Ellipse") lines.push("border-radius: 50%;");
  else if (n.radius > 0) lines.push(`border-radius: ${px(n.radius)};`);
  if (n.stroke) lines.push(`border: ${px(n.stroke.width)} solid ${n.stroke.color};`);
  return lines.join("\n");
}

export function tailwindText(n: NodeInfo) {
  const t = n.text!;
  const cls = [`text-[${px(t.size)}]`, `font-[${t.weight}]`, `text-[${n.fill}]`];
  if (n.fillOpacity < 100) cls[2] += `/${n.fillOpacity}`;
  cls.push(`font-[${t.family.replace(/ /g, "_")}]`);
  if (t.lineHeight.unit !== "auto") cls.push(`leading-[${lineHeightCss(t)}]`);
  if (t.letterSpacing) cls.push(`tracking-[${Math.round(t.letterSpacing * 100) / 10000}em]`);
  if (t.italic) cls.push("italic");
  if (t.align !== "left") cls.push(`text-${t.align}`);
  if (t.decoration === "underline") cls.push("underline");
  if (t.decoration === "strikethrough") cls.push("line-through");
  if (t.case !== "original") cls.push({ upper: "uppercase", lower: "lowercase", title: "capitalize" }[t.case]);
  if (t.resize !== "auto-width") cls.push(`w-[${px(n.w)}]`);
  const content = t.content.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/\n/g, "<br />");
  return `<p class="${cls.join(" ")}">${content}</p>`;
}

export function tailwind(n: NodeInfo) {
  if (n.text) return tailwindText(n);
  const cls = [`w-[${px(n.w)}]`, `h-[${px(n.h)}]`, `bg-[${n.fill}]`];
  if (n.fillOpacity < 100) cls[2] += `/${n.fillOpacity}`;
  if (n.kind === "Ellipse") cls.push("rounded-full");
  else if (n.radius > 0) cls.push(`rounded-[${px(n.radius)}]`);
  if (n.stroke) cls.push(`border-[${px(n.stroke.width)}]`, `border-[${n.stroke.color}]`);
  return `<div class="${cls.join(" ")}" />`;
}
