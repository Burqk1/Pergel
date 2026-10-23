import type { MenuItem } from "./ui/Menu.svelte";
import { editor } from "./editor.svelte";

/**
 * The layer context menu, in Figma's order. `at` is the canvas point that was right-clicked
 * (for "Paste here" and "Select layer"); `panel` adds the layers-panel extras (Rename).
 */
export function layerMenu(options: { at?: { x: number; y: number }; panel?: boolean } = {}): MenuItem[] {
  const sel = editor.selection;
  const engine = editor.engine;
  if (!engine) return [];
  const pasteHere: MenuItem = {
    label: "Paste here",
    shortcut: "Ctrl+V",
    disabled: !editor.clipboard,
    action: () => editor.paste(),
  };

  if (!sel.length) {
    return [
      pasteHere,
      "-",
      { label: "Select all", shortcut: "Ctrl+A", action: () => editor.selectAll() },
      { label: "Zoom to fit", shortcut: "Shift+1", action: () => editor.fit() },
    ];
  }

  const one = sel.length === 1 ? sel[0] : null;
  const container = sel.some((n) => (n.kind === "Group" || n.kind === "Frame") && n.children > 0);
  const frames = sel.every((n) => n.kind === "Frame");
  const instance = sel.some((n) => n.component === "instance");
  const inInstance = sel.some((n) => n.inInstance);
  const hidden = !sel[0].visible;
  const locked = sel[0].locked;
  const hasText = sel.some((n) => n.kind === "Text" || n.children > 0);

  const under = options.at ? engine.layersAt(options.at.x, options.at.y) : [];
  const selectLayer: MenuItem[] = under.length
    ? [
        {
          label: "Select layer",
          children: under.map((l) => ({
            label: l.name,
            checked: sel.some((n) => n.id === l.id),
            action: () => editor.select(l.id),
          })),
        },
      ]
    : [];

  const items: MenuItem[] = [
    { label: "Copy", shortcut: "Ctrl+C", action: () => editor.copy() },
    ...(options.at ? [pasteHere] : []),
    { label: "Paste to replace", shortcut: "Ctrl+Shift+R", disabled: !editor.clipboard || inInstance, action: () => editor.pasteReplace() },
    {
      label: "Copy/Paste as",
      children: [
        { label: "Copy as text", disabled: !hasText, action: () => editor.copyAsText() },
        {
          label: "Copy as code",
          children: [
            { label: "CSS", action: () => editor.copyAsCode("css") },
            { label: "Tailwind classes", action: () => editor.copyAsCode("tailwind") },
            { label: "HTML + CSS (all layers)", action: () => editor.copyAsCode("html") },
            { label: "React + Tailwind (all layers)", action: () => editor.copyAsCode("react") },
          ],
        },
        { label: "Copy as SVG", action: () => editor.copyAsSvg() },
        { label: "Copy as PNG", shortcut: "Ctrl+Shift+C", action: () => editor.copyAsPng() },
        "-",
        { label: "Copy properties", shortcut: "Ctrl+Alt+C", disabled: !one, action: () => editor.copyProperties() },
        { label: "Paste properties", shortcut: "Ctrl+Alt+V", disabled: !editor.hasProperties, action: () => editor.pasteProperties() },
      ],
    },
    ...selectLayer,
    "-",
    { label: "Bring to front", shortcut: "]", disabled: inInstance, action: () => editor.reorder("front") },
    { label: "Send to back", shortcut: "[", disabled: inInstance, action: () => editor.reorder("back") },
    "-",
    { label: "Group selection", shortcut: "Ctrl+G", disabled: inInstance, action: () => editor.group() },
    { label: "Frame selection", shortcut: "Ctrl+Alt+G", disabled: inInstance, action: () => editor.group(true) },
    { label: "Ungroup", shortcut: "Ctrl+Backspace", disabled: !container || inInstance, action: () => editor.ungroup() },
    ...(options.panel
      ? ([{ label: "Rename", shortcut: "Ctrl+R", disabled: sel.length !== 1, action: () => editor.renameSelection() }] as MenuItem[])
      : []),
    { label: "Flatten", shortcut: "Alt+Shift+F", disabled: inInstance, action: () => editor.boolean("flatten") },
    { label: "Outline stroke", shortcut: "Ctrl+Alt+O", disabled: inInstance || !sel.some((n) => n.stroke), action: () => editor.outlineStroke() },
    { label: "Offset path…", disabled: inInstance, action: () => (editor.offsetOpen = true) },
    { label: "Add annotation", disabled: sel.length !== 1, action: () => ((editor.rightTab = "design"), (editor.annotating = true)) },
    { label: "Type on path", disabled: inInstance || !sel.some((n) => n.kind === "Text"), action: () => editor.textOnPath() },
    { label: "Join paths", shortcut: "Ctrl+J", disabled: inInstance || !sel.some((n) => n.kind === "Vector"), action: () => editor.join() },
    { label: "Simplify vertices", disabled: inInstance || !sel.some((n) => n.kind === "Vector"), action: () => editor.simplify() },
    {
      label: "Boolean groups",
      disabled: inInstance || sel.length < 2,
      children: [
        { label: "Union selection", action: () => editor.boolean("union") },
        { label: "Subtract selection", action: () => editor.boolean("subtract") },
        { label: "Intersect selection", action: () => editor.boolean("intersect") },
        { label: "Exclude selection", action: () => editor.boolean("exclude") },
      ],
    },
    { label: sel.every((n) => n.mask) ? "Remove mask" : "Use as mask", shortcut: "Ctrl+Alt+M", disabled: inInstance, action: () => editor.useAsMask() },
    "-",
    { label: "Add auto layout", shortcut: "Shift+A", disabled: inInstance, action: () => editor.autoLayout(true) },
    {
      label: "More layout options",
      children: [
        { label: "Remove auto layout", shortcut: "Alt+Shift+A", disabled: !sel.some((n) => n.layout), action: () => editor.autoLayout(false) },
        "-",
        { label: "Resize to fit", shortcut: "Ctrl+Alt+Shift+R", disabled: !frames || !container, action: () => editor.resizeToFit() },
        { label: sel[0].lockRatio ? "Unlock aspect ratio" : "Lock aspect ratio", action: () => editor.patch({ lockRatio: !sel[0].lockRatio }) },
        "-",
        { label: "Set width to hug contents", action: () => editor.patch({ sizingH: "hug" }) },
        { label: "Set height to hug contents", action: () => editor.patch({ sizingV: "hug" }) },
        { label: "Set width to fill container", disabled: !sel.every((n) => n.parentFlow), action: () => editor.patch({ sizingH: "fill" }) },
        { label: "Set height to fill container", disabled: !sel.every((n) => n.parentFlow), action: () => editor.patch({ sizingV: "fill" }) },
      ],
    },
    { label: "Create component", shortcut: "Ctrl+Alt+K", disabled: inInstance, action: () => editor.createComponent() },
  ];
  if (instance) {
    items.push(
      { label: "Go to main component", disabled: !one?.main, action: () => editor.goToMain() },
      { label: "Reset instance", action: () => editor.resetInstance() },
      { label: "Detach instance", shortcut: "Ctrl+Alt+B", action: () => editor.detachInstance() },
    );
  }
  items.push(
    "-",
    { label: hidden ? "Show" : "Hide", shortcut: "Ctrl+Shift+H", action: () => editor.toggleFlag(-1, "visible") },
    { label: locked ? "Unlock" : "Lock", shortcut: "Ctrl+Shift+L", action: () => editor.toggleFlag(-1, "locked") },
    "-",
    { label: "Flip horizontal", shortcut: "Shift+H", disabled: inInstance, action: () => editor.flip(true) },
    { label: "Flip vertical", shortcut: "Shift+V", disabled: inInstance, action: () => editor.flip(false) },
  );
  return items;
}
