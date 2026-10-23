import { editor } from "./editor.svelte";
import { workspace } from "./workspace.svelte";

/** A command for Quick actions (Ctrl+K). */
export interface Command {
  label: string;
  group: string;
  shortcut?: string;
  enabled?: () => boolean;
  run: () => void;
}

const hasSelection = () => editor.selection.length > 0;
const many = () => editor.selection.length > 1;

export const COMMANDS: Command[] = [
  // Tools
  { group: "Tools", label: "Move", shortcut: "V", run: () => editor.setTool("move") },
  { group: "Tools", label: "Hand", shortcut: "H", run: () => editor.setTool("hand") },
  { group: "Tools", label: "Scale", shortcut: "K", run: () => editor.setTool("scale") },
  { group: "Tools", label: "Frame", shortcut: "F", run: () => editor.setTool("frame") },
  { group: "Tools", label: "Section", shortcut: "Shift+S", run: () => editor.setTool("section") },
  { group: "Tools", label: "Slice", shortcut: "S", run: () => editor.setTool("slice") },
  { group: "Tools", label: "Rectangle", shortcut: "R", run: () => editor.setTool("rectangle") },
  { group: "Tools", label: "Ellipse", shortcut: "O", run: () => editor.setTool("ellipse") },
  { group: "Tools", label: "Line", shortcut: "L", run: () => editor.setTool("line") },
  { group: "Tools", label: "Arrow", shortcut: "Shift+L", run: () => editor.setTool("arrow") },
  { group: "Tools", label: "Polygon", run: () => editor.setTool("polygon") },
  { group: "Tools", label: "Star", run: () => editor.setTool("star") },
  { group: "Tools", label: "Text", shortcut: "T", run: () => editor.setTool("text") },
  { group: "Tools", label: "Pen", shortcut: "P", run: () => editor.setTool("pen") },
  { group: "Tools", label: "Pencil", shortcut: "Shift+P", run: () => editor.setTool("pencil") },
  { group: "Tools", label: "Place image…", shortcut: "Ctrl+Shift+K", run: () => editor.placeImageDialog() },
  // File
  { group: "File", label: "New design file", shortcut: "Ctrl+T", run: () => void workspace.newFile() },
  { group: "File", label: "Back to files", shortcut: "Ctrl+O", run: () => workspace.goHome() },
  { group: "File", label: "Import from Figma…", run: () => (editor.figmaImport = true) },
  { group: "File", label: "Add page", run: () => editor.addPage() },
  { group: "File", label: "Show version history", run: () => (editor.versionsOpen = true) },
  { group: "File", label: "Save to version history…", shortcut: "Ctrl+Alt+S", run: () => (editor.versionsOpen = true) },
  { group: "File", label: "Local variables…", run: () => (editor.variablesOpen = true) },
  // Edit
  { group: "Edit", label: "Undo", shortcut: "Ctrl+Z", run: () => editor.undo() },
  { group: "Edit", label: "Redo", shortcut: "Ctrl+Y", run: () => editor.redo() },
  { group: "Edit", label: "Select all", shortcut: "Ctrl+A", run: () => editor.selectAll() },
  { group: "Edit", label: "Select matching layers", shortcut: "Ctrl+Alt+A", enabled: hasSelection, run: () => editor.selectMatching() },
  { group: "Edit", label: "Select inverse", shortcut: "Ctrl+Shift+A", enabled: hasSelection, run: () => editor.selectInverse() },
  { group: "Edit", label: "Find layers", shortcut: "Ctrl+F", run: () => (editor.focusLayerSearch = true) },
  { group: "Edit", label: "Find and replace…", shortcut: "Ctrl+H", run: () => (editor.findOpen = true) },
  { group: "File", label: "Branches: create, review, merge…", run: () => (editor.branchOpen = true) },
  { group: "File", label: "New slide deck", run: () => void import("./publish").then((m) => m.newSlideDeck()) },
  { group: "File", label: "Export slides as PDF", run: () => void import("./publish").then((m) => m.exportSlidesPdf()) },
  { group: "File", label: "Publish frames as website…", run: () => void import("./publish").then((m) => m.publishSite()) },
  { group: "Tools", label: "Sticky note", run: () => editor.setTool("sticky") },
  { group: "Edit", label: "Pick color", shortcut: "I", run: () => void editor.pickColor() },
  { group: "Edit", label: "Duplicate", shortcut: "Ctrl+D", enabled: hasSelection, run: () => editor.duplicate() },
  { group: "Edit", label: "Copy as SVG", enabled: hasSelection, run: () => editor.copyAsSvg() },
  { group: "Edit", label: "Copy as PNG", shortcut: "Ctrl+Shift+C", enabled: hasSelection, run: () => editor.copyAsPng() },
  { group: "Edit", label: "Copy as code (React + Tailwind)", enabled: hasSelection, run: () => editor.copyAsCode("react") },
  { group: "Edit", label: "Copy as code (HTML + CSS)", enabled: hasSelection, run: () => editor.copyAsCode("html") },
  // Object
  { group: "Object", label: "Group selection", shortcut: "Ctrl+G", enabled: hasSelection, run: () => editor.group() },
  { group: "Object", label: "Frame selection", shortcut: "Ctrl+Alt+G", enabled: hasSelection, run: () => editor.group(true) },
  { group: "Object", label: "Ungroup", shortcut: "Ctrl+Backspace", enabled: hasSelection, run: () => editor.ungroup() },
  { group: "Object", label: "Add auto layout", shortcut: "Shift+A", enabled: hasSelection, run: () => editor.autoLayout(true) },
  { group: "Object", label: "Remove auto layout", shortcut: "Alt+Shift+A", enabled: hasSelection, run: () => editor.autoLayout(false) },
  { group: "Object", label: "Tidy up", shortcut: "Ctrl+Alt+Shift+T", enabled: hasSelection, run: () => editor.align("tidy") },
  { group: "Object", label: "Pack", enabled: hasSelection, run: () => editor.align("pack") },
  { group: "Object", label: "Join paths", shortcut: "Ctrl+J", enabled: hasSelection, run: () => editor.join() },
  { group: "Text", label: "Type on path / detach", enabled: hasSelection, run: () => editor.textOnPath() },
  { group: "Object", label: "Distribute horizontal spacing", shortcut: "Alt+Shift+H", enabled: hasSelection, run: () => editor.align("distributeX") },
  { group: "Object", label: "Distribute vertical spacing", shortcut: "Alt+Shift+V", enabled: hasSelection, run: () => editor.align("distributeY") },
  { group: "Object", label: "Create component", shortcut: "Ctrl+Alt+K", enabled: hasSelection, run: () => editor.createComponent() },
  { group: "Object", label: "Detach instance", shortcut: "Ctrl+Alt+B", enabled: hasSelection, run: () => editor.detachInstance() },
  { group: "Object", label: "Use as mask", shortcut: "Ctrl+Alt+M", enabled: hasSelection, run: () => editor.useAsMask() },
  { group: "Object", label: "Union selection", enabled: many, run: () => editor.boolean("union") },
  { group: "Object", label: "Subtract selection", enabled: many, run: () => editor.boolean("subtract") },
  { group: "Object", label: "Intersect selection", enabled: many, run: () => editor.boolean("intersect") },
  { group: "Object", label: "Exclude selection", enabled: many, run: () => editor.boolean("exclude") },
  { group: "Object", label: "Flatten", shortcut: "Ctrl+E", enabled: hasSelection, run: () => editor.boolean("flatten") },
  { group: "Object", label: "Outline stroke", shortcut: "Ctrl+Shift+O", enabled: hasSelection, run: () => editor.outlineStroke() },
  { group: "Object", label: "Offset path…", enabled: hasSelection, run: () => (editor.offsetOpen = true) },
  { group: "Object", label: "Simplify vertices", enabled: hasSelection, run: () => editor.simplify() },
  { group: "Object", label: "Resize to fit", shortcut: "Ctrl+Alt+Shift+R", enabled: hasSelection, run: () => editor.resizeToFit() },
  { group: "Object", label: "Bring to front", shortcut: "]", enabled: hasSelection, run: () => editor.reorder("front") },
  { group: "Object", label: "Send to back", shortcut: "[", enabled: hasSelection, run: () => editor.reorder("back") },
  { group: "Object", label: "Rotate 180°", enabled: hasSelection, run: () => editor.rotateBy(180) },
  { group: "Object", label: "Rotate 90° left", enabled: hasSelection, run: () => editor.rotateBy(90) },
  { group: "Object", label: "Rotate 90° right", enabled: hasSelection, run: () => editor.rotateBy(-90) },
  { group: "Object", label: "Round to pixel", enabled: hasSelection, run: () => editor.roundToPixel() },
  { group: "Object", label: "Combine as variants", enabled: hasSelection, run: () => editor.combineVariants() },
  { group: "Object", label: "Edit vector points", shortcut: "Enter", enabled: hasSelection, run: () => editor.editVector() },
  { group: "Object", label: "Flip horizontal", shortcut: "Shift+H", enabled: hasSelection, run: () => editor.flip(true) },
  { group: "Object", label: "Flip vertical", shortcut: "Shift+V", enabled: hasSelection, run: () => editor.flip(false) },
  { group: "Object", label: "Show/hide selection", shortcut: "Ctrl+Shift+H", enabled: hasSelection, run: () => editor.toggleFlag(-1, "visible") },
  { group: "Object", label: "Lock/unlock selection", shortcut: "Ctrl+Shift+L", enabled: hasSelection, run: () => editor.toggleFlag(-1, "locked") },
  // View
  { group: "View", label: "Zoom to fit", shortcut: "Shift+1", run: () => editor.fit() },
  { group: "View", label: "Layout guides", shortcut: "Shift+G", run: () => editor.toggleGrids() },
  { group: "View", label: "Rulers", shortcut: "Shift+R", run: () => (editor.showRulers = !editor.showRulers) },
  { group: "View", label: "Next frame", shortcut: "N", run: () => editor.nextFrame(1) },
  { group: "View", label: "Previous frame", shortcut: "Shift+N", run: () => editor.nextFrame(-1) },
  { group: "View", label: "Next page", shortcut: "PgDn", run: () => editor.stepPage(1) },
  { group: "View", label: "Previous page", shortcut: "PgUp", run: () => editor.stepPage(-1) },
  { group: "View", label: "Zoom in", shortcut: "Ctrl++", run: () => editor.zoomBy(2) },
  { group: "View", label: "Zoom out", shortcut: "Ctrl+-", run: () => editor.zoomBy(0.5) },
  { group: "View", label: "Show/hide UI", shortcut: "Ctrl+\\", run: () => (editor.uiHidden = !editor.uiHidden) },
  { group: "View", label: "Dev Mode", shortcut: "Shift+D", run: () => (editor.devMode = !editor.devMode) },
  { group: "View", label: "Present prototype", shortcut: "Ctrl+Alt+Enter", run: () => editor.present() },
];
