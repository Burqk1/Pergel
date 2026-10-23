# Figma feature inventory (observed)

Observed on 2026-10-17 in Figma Desktop 126.9.10 (Windows) through Windows UI Automation, the accessibility API that screen readers use, plus screenshots. Nothing was decompiled. This is the checklist Pergel's feature work follows; ✅ marks what Pergel already has.

## Window layout

- **Navigation bar** (left-most rail, icon + label): Main menu, File, Agents, Assets (local components ✅), Tools, Variables.
- **Left sidebar**: file name (+ "Edit file menu" chevron), Minimize UI, project link ("Drafts"); Pages (Find, Add new page, "Ready for dev" badge per page); Layers ✅ (Collapse layers, tree with expand arrows, drag to reorder, thumbnails for frames, lock/visibility toggles on hover, resize handle).
- **Right sidebar**: Multiplayer tools (avatar), Present + Prototype view, Share; tabs Design / Prototype; zoom % with view options.
- **Bottom toolbar** (floating): tool groups + "Toolbelt mode" switch.
- **Help/zoom toolbar** (bottom-right "?").

## Toolbar ✅ (layout) — tools

| Group | Items |
|---|---|
| Move tools | Move ✅, Hand tool ✅, Scale |
| Region tools | Frame ✅, Section, Slice |
| Shape tools | Rectangle ✅, Line, Arrow, Ellipse ✅, Polygon, Star, Image/video… |
| Creation tools | Pen, Pencil |
| Type tools | Text ✅, Text on path |
| Comment tools | Comment, Annotation, Measurement |
| — | Actions (Ctrl+K) |
| Toolbelt mode | Draw, Design, Motion, Dev Mode |

## Main menu

- **Actions…** Ctrl+K
- **File**: New Design, New, Place image/video… Ctrl+Shift+K, Duplicate, Save local copy…, Save to version history… Ctrl+Alt+S, Show version history, Export… Ctrl+Shift+E, Export frames to PDF…, Create branch…
- **Edit**: Undo Ctrl+Z ✅, Redo Ctrl+Y ✅, Copy as ▸, Paste over selection Ctrl+Shift+V, Paste to replace Ctrl+Shift+R, Duplicate Ctrl+D ✅, Delete ✅, Find Ctrl+F, Find next / previous, Find and replace…, Set default properties, Copy properties Ctrl+Alt+C, Paste properties Ctrl+Alt+V, Pick color I, Select all Ctrl+A ✅, Select matching layers Ctrl+Alt+A, Select none Esc ✅, Select inverse Ctrl+Shift+A, Select all with ▸
- **View**: Pixel grid Shift+', Layout guides Shift+G, Rulers Shift+R, Show slices, Comments Shift+C, Annotations Shift+Y, Outlines ▸, Pixel preview Ctrl+Shift+P, Mask outlines, Frame outlines, Memory usage, Additional labels, Minimize left navigation bar, Minimize UI Ctrl+Shift+\, Show/Hide UI Ctrl+\, Multiplayer cursors Ctrl+Alt+\, Switch to Draw, Switch to Dev Mode Shift+D, Panels ▸, Zoom in Ctrl++ ✅, Zoom out Ctrl+- ✅, Zoom to 100% Ctrl+0 ✅ (Pergel: Shift+0), Zoom to fit Shift+1 ✅, Zoom to selection Shift+2 ✅, Previous/Next page PgUp/PgDn, Zoom to previous/next frame Shift+N / N, Find previous/next frame Home/End
- **Object**: Frame selection Ctrl+Alt+G ✅, Group selection Ctrl+G ✅, Ungroup Ctrl+Backspace ✅, Wrap in new section Ctrl+S, Convert to section, Convert to frame, Restore default thumbnail, Use as mask Ctrl+Alt+M, Add auto layout Shift+A ✅, More layout options ▸ (Suggest auto layout Ctrl+Alt+Shift+A, Remove all auto layout (Alt+Shift+A ✅), Lock/Unlock aspect ratio, Resize to fit Ctrl+Alt+Shift+R, Set width/height to hug contents ✅, Set width/height to fill container ✅), Create component Ctrl+Alt+K ✅, Slots ▸ (Convert to slot Ctrl+Shift+S, Convert to frame, Wrap in new slot, Reset slot), Reset instance ✅, Detach instance Ctrl+Alt+B ✅, Main component ▸ (Go to main ✅), Bring to front ] ✅, Bring forward Ctrl+] ✅, Send backward Ctrl+[ ✅, Send to back [ ✅, Flip horizontal Shift+H ✅, Flip vertical Shift+V ✅, Rotate 180°, Rotate 90° left/right, Flatten Alt+Shift+F, Outline stroke Ctrl+Alt+O, Boolean groups ▸, Show/Hide selection Ctrl+Shift+H ✅, Lock/Unlock selection Ctrl+Shift+L ✅, Hide other layers, Collapse layers Alt+L ✅, Remove fill Alt+/ ✅, Remove stroke Shift+/ ✅, Swap fill and stroke Shift+X ✅, Remove interactions, Delete contents
- **Text**: Bold Ctrl+B, Italic Ctrl+I, Underline Ctrl+U, Strikethrough Ctrl+Shift+X, Create link Ctrl+Shift+U, Bulleted list Ctrl+Shift+8, Numbered list Ctrl+Shift+7, Alignment ▸ (left Ctrl+Alt+L, center Ctrl+Alt+T, right Ctrl+Alt+R, justified Ctrl+Alt+J, top, middle, bottom), Adjust ▸, Case ▸, Text direction ▸, Spell check ▸, Show text suggestions
- **Arrange**: Round to pixel, Align left Alt+A ✅, Align horizontal centers Alt+H ✅, Align right Alt+D ✅, Align top Alt+W ✅, Align vertical centers Alt+V ✅, Align bottom Alt+S ✅, Tidy up Ctrl+Alt+Shift+T, Pack horizontal / vertical, Distribute horizontal spacing Alt+Shift+H ✅, Distribute vertical spacing Alt+Shift+V ✅, Distribute left / horizontal centers / right / top / vertical centers / bottom
- **Vector**: Join selection Ctrl+J, Smooth join selection, Delete and heal selection Shift+Backspace, Split vector, Simplify vector, Offset vector
- **Plugins**: Run last plugin Ctrl+Alt+P, Development ▸ (New plugin…, Import plugin from manifest…, View API documentation, Show/Hide console, Use developer VM, Hot reload plugin), Manage plugins…
- **Widgets**: Development ▸ (New widget…, Import widget from manifest…, Show/Hide console, Use developer VM), Manage widgets…, Select all widgets
- **Preferences** (toggles): Snap to geometry, Snap to objects, Snap to pixel grid Ctrl+Shift+', Keep tool selected after use, Use alternate zoom handling, Highlight layers on hover, Rename duplicated layers, Show dimensions on objects, Hide canvas UI during changes, Use smart quotes/symbols, Flip objects while resizing, Keyboard zooms into selection, Invert zoom direction, Ctrl+click opens right click menus, Use number keys for opacity ✅, Use old shortcuts for outlines, Use Ctrl+Alt+↑/↓ to rotate layers, Play audio notifications in AI chat, Open links in desktop app, Show text suggestions, Show tool suggestions, Show Agents on canvas, Use scroll wheel zoom, Right-click and drag to pan; plus Theme ▸, Color profile…, Keyboard layout…, Accessibility settings…, Nudge amount…
- **Libraries**, **AI balance**
- **Help and account**: Help page, Keyboard shortcuts Ctrl+Shift+?, Support forum, Video tutorials, Release notes, Legal summary, Account settings, Log out

## Canvas context menu (layer selected)

Copy Ctrl+C ✅, Paste here ✅, Paste to replace Ctrl+Shift+R, Copy/Paste as ▸, Send to Figma Make, Find similar designs, Add motion, Bring to front ] ✅, Send to back [ ✅, Convert to section, Group selection Ctrl+G ✅, Frame selection Ctrl+Alt+G ✅, Ungroup Ctrl+Backspace ✅, Flatten Alt+Shift+F, Outline stroke Ctrl+Alt+O, Set as thumbnail, Use as mask Ctrl+Alt+M, Add auto layout Shift+A ✅, More layout options ▸, Create component Ctrl+Alt+K ✅, Plugins ▸, Widgets ▸, Show/Hide Ctrl+Shift+H ✅, Lock/Unlock Ctrl+Shift+L ✅, Flip horizontal Shift+H, Flip vertical Shift+V

## Right sidebar — Design tab

**Nothing selected**: Page (background color + opacity % + visibility) ✅, Styles (+ Create style), Export (+).

**Frame selected** (in order):

1. Header: Frame ▾ (Frame Dimension Presets) ✅ (with presets menu), Toggle ready for dev status, Ready for dev actions, Create component, More actions, "Copy to Figma Slides"
2. **Position**: Alignment (left, h-centers, right, top, v-centers, bottom, more) ✅ UI; Position X ✅, Y ✅; Rotation field ✅, Rotate 90° right ✅, Flip horizontal ✅, Flip vertical ✅
3. **Layout**: Resize to fit, Toggle auto layout ✅; Flow: Freeform / Vertical / Horizontal / Wrap / Grid ✅; gap, padding, 9-point alignment, Hug/Fill/Fixed ✅; Dimensions W ✅, H ✅, Lock aspect ratio ✅; Clip content ✅
4. **Appearance** ✅: Hide (eye), Apply blend mode, Opacity, Corner radius, Individual corners
5. **Fill**: Apply styles and variables, Add fill, rows (swatch ✅, hex ✅, opacity % ✅, visibility, remove ✅); Show in exports
6. **Stroke**: styles/variables, Add stroke fill ✅, rows (swatch ✅, hex ✅, % ✅, visibility, remove ✅); Stroke align (Inside/Center/Outside) ✅, Stroke weight ✅, Advanced stroke settings, Individual strokes, End points
7. **Effects**: Apply styles, Add effect
8. **Selection colors** (+N)
9. **Layout guide**: Apply styles, Add layout guide, rows (settings, type e.g. "16 columns", visibility, remove)
10. **Export**: Add export settings, rows (scale or W/H constraint, file type PNG/JPG/SVG/PDF, advanced settings, remove), Export <layer>, Preview

Visual details to copy: small tertiary sub-labels above field groups ("Alignment", "Position", "Rotation", "Flow", "Dimensions", "Opacity", "Corner radius", "Weight", "End points"); field prefixes inside inputs (X, Y, W, H); section headers with right-aligned icon buttons; full-width secondary button under the header.

**Text selected** ✅: Typography (font family, weight/style, size + presets, line height Auto/px/%, letter spacing %/px, align left/center/right/justify, vertical top/middle/bottom, type settings: decoration + case), Resizing (auto width / auto height / fixed), Fill = text colour.

## Right sidebar — Prototype tab

Flow starting point (+), Interactions (Add action), Scroll behavior → Overflow (No scrolling, …), prototype settings; onboarding cards ("Creating a connection", "Running your prototype"); Figma Make upsell.

## Not yet observed

Effects editor, color picker, Variables modal, Assets/Tools/Agents panels, Dev Mode panel, Motion mode, zoom/view options menu, edit-file menu.

## Round 2 (2026-10-17): details found on a second pass

**Desktop shell.** Custom title bar with tabs: Home tab (house icon), one tab per open file (file-type
icon, name, ✕), "+" new file, right side: "Open and recently closed tabs" ▾ and window controls.
App menu: New Window Ctrl+Shift+N, New Tab Ctrl+T, Open File Browser Ctrl+O, Open File URL From
Clipboard, Close Window Ctrl+Shift+W, Close Tab Ctrl+W, Reopen Closed Tab Ctrl+Shift+T, Recently
Closed Tabs ▸, Plugins, Interface Scale, Preferences, Help, About, Check for Updates, Exit.

**Home (file browser).** Sidebar: account ▾, notifications, Search, Recents, Community; team:
Drafts, All folders, Resources, Trash, Admin; Starred; projects. Header: back/forward, title,
New Design/FigJam/Slides/Make buttons. Tabs: Recently viewed / Shared files / Shared folders;
filters (organizations, file type), grid/list toggle. Card: thumbnail, file-type icon, name,
"Edited 2 hours ago", avatars, star toggle on hover. Card menu: Show in folder, Open, Create new
branch, Add to favorites, Copy link, Share, Duplicate, Show version history, Rename, Move file…,
Move to trash, Remove from recent.

**Left navigation rail.** Figma menu, File, Agents, Assets, Tools (plugin/AI tool browser with
search, Source/Category filters), Variables (full-width editor: Collections list, Create/Import).

**Layers panel.** Each row: type icon, thumbnail, name; lock/eye on hover; "Pan to node" per row.
Row context menu adds Rename Ctrl+R and Rename layers (AI) to the canvas menu.

**Canvas context menu (layer).** Copy, Paste here, Paste to replace, Copy/Paste as ▸ (Copy as
text, Copy as code ▸ (CSS, CSS (all layers), iOS, Android), Copy as SVG, Copy as PNG Ctrl+Shift+C,
Copy properties Ctrl+Alt+C, Paste properties Ctrl+Alt+V, Paste animation), Send to Figma Make,
Find similar designs, Add motion ▸, Select layer ▸ (layers under the cursor), Bring to front,
Send to back, Group, Frame, Flatten, Wrap with transform, Outline stroke, Use as mask, Add auto
layout, Create component, Plugins ▸ (Development, Manage plugins…), Widgets ▸, Show/Hide,
Lock/Unlock, Flip horizontal/vertical.

**Right panel (text selected, Design).** Header: layer type ("Text") with component/style icon
and ▾. Sub-labels: Alignment; Position (X, Y, Constraints); Rotation (field, Rotate 90° right,
Flip horizontal, Flip vertical); Layout → Resizing (Auto width / Auto height / Fixed size),
Dimensions (W, H, Lock aspect ratio); Appearance → Opacity (field + slider + Hide eye), Blend mode
dropdown; Typography (Apply styles; Font family; Font style; Font size; Line height; Letter
spacing; Switch text direction; Alignment horizontal + vertical; Type settings); Fill rows
"FFFFFF / 100% · Solid" with eye and remove; Stroke; Effects; Export. Each section header has
Apply styles, Add (+) and a collapse ▾.

**Frame selected (UI3).** Alignment row incl. distribute; Position with constraints toggle;
Rotation; Layout → Flow (Freeform / Vertical / Horizontal / Grid), Dimensions, Clip content;
Appearance → Opacity + slider, Corner radius + slider + individual corners, Blend mode;
Fill rows; Stroke rows + Weight (field + slider) + Position (inside/center/outside), Join,
End points, Width profile; Effects rows (e.g. "Background blur · Uniform · 16"); Selection colors
(swatches +N); Layout guide; Export.

**Color picker.** Tabs Custom / Libraries, New style or variable (+), Close. Paint type: Solid,
Gradient, Pattern, Image, Video, Shader; blend mode; Check color contrast. Saturation/brightness
square, Sample color (eyedropper), Hue slider, Opacity slider, Color format (Hex/RGB/HSL/HSB/CSS)
+ value + opacity %. Swatch set selector ("On this page") with the document's colours.

**Contextual toolbar** (above the main toolbar when layers are selected): Create component,
Create link, Edit object, Use as mask, Boolean operations (Union ▾), Select matching layers.
Toolbelt mode: Draw (Pen, Pencil, Brush…), Design, Motion, Dev Mode.

## Round 3 (2026-10-17): built since round 2

MCP, design to code:
- Assets as files: icons, illustrations and image fills, deduplicated, written to the project or served at `/assets`.
- Implementation rules in every design context.
- Screenshot comparison (`compare_to_design`), Iconify icon search, icon/svg/image node specs.
- Variables (`set_variables`, `bind_variable`, CSS custom properties).
- Code Connect (`add_code_connect_map`, `get_code_connect_map`).
- Variants (`set_variant`) and prototype interactions (`set_interactions`).
- Pages (`get_pages`, `switch_page`), annotations, styles, layout grids.
- Prompts: `implement_design`, `design_screen`.

Editor:
- Vectors and SVG import (drop, paste, Ctrl+Shift+K).
- Gradients (linear, radial, angular, diamond) and image fills (fill, fit, crop, tile).
- Line, arrow, polygon and star tools; pen tool and vector edit mode.
- Boolean operations, flatten, outline stroke, use as mask.
- Multiple pages.
- Import from Figma (REST API: pages, vectors, images, text with mixed styles, auto layout, components and instances, variants, constraints, grids, interactions).
- Local variables with modes; text, colour and effect styles; component sets (variants).
- Constraints; prototype tab with Present.
- Contextual toolbar; quick actions (Ctrl+K); layer search (Ctrl+F); hide UI (Ctrl+\).
- System fonts plus Fontsource fallback.
- Smart guides and snapping; layout guides (Shift+G); rulers and guides (Shift+R).
- Version history; dashed strokes; annotations (comment tool).
- Select matching/inverse, next/previous frame (N), PgUp/PgDn pages, pick colour (I), Ctrl+B/I/U on text and text ranges.

Added after round 3:
- Section tool (Shift+S): titled, non-clipping frames that organise the canvas. Frames inside a
  section get labels, and Figma `SECTION` nodes import as sections.
- Scale tool (K): handles, or a factor with an anchor in the panel. Strokes, radii, text, effects
  and auto-layout spacing scale with the layer.
- Pencil (Shift+P) with smoothing.
- Find and replace (Ctrl+H).
- Tidy up (Ctrl+Alt+Shift+T).
- Stroke end points: line arrow, triangle, reversed triangle, circle, diamond. The arrow tool now
  makes a line with an arrowhead, as in Figma.
- Alt-hover measurement; text resizing control; Ready for dev.
- Layer thumbnails; navigation rail (File, Agents, Assets, Tools, Variables).
- Prototype overflow scrolling (vertical, horizontal, both): Present scrolls the whole content,
  hotspots past the fold work, code gets `overflow-y: auto`, and Figma `overflowDirection` imports.
- Image adjustments (exposure, contrast, saturation, temperature, tint, highlights, shadows): one
  colour matrix shared by the renderer, SVG (`feColorMatrix`), code (CSS `filter`) and Figma import.
- Individual strokes per side (top, right, bottom, left): rendered, exported to SVG, inset shadows
  or borders in code, `individualStrokeWeights` imported.
- Offset path (dialog: distance, miter/round/bevel corners; a new layer outside or inside) and
  Simplify vertices (refit with fewer smooth anchors). Both are in the layer menu and Ctrl+K.
- Interactive components: "Change to" another variant on click, hover (reverts when the pointer
  leaves) or after a delay; Present previews it without touching the document (`Scene::rollback`).
  Figma `CHANGE_TO` actions import. MCP `set_interactions` takes `change-to`.
- MCP `get_code_connect_suggestions` (codebase scan → mapping proposals).
- MCP `search_design_system` and `generate_diagram` (Mermaid flowcharts, as in Figma's MCP).
- MCP: `line`/`arrow`/`section` specs. `update_node` takes `note`, `readyForDev`, `startPoint` and
  `endPoint`.

Round 4 (2026-10-18): the rest of the list.
- Slice tool (S): an export area, drawn as a dashed outline, not rendered; PNG export preset.
  Codegen and assets skip slices, and Figma `SLICE` nodes import.
- Pack: shelf packing, tallest first, with a gap of at most 24. Join paths (Ctrl+J): open paths
  chain at their nearest ends; a single open path is closed instead.
- Type on path: text along a vector outline (Skia RSXform per glyph), aligned left, centre or
  right. It exports as an SVG `<textPath>`, so code gets an asset.
- Plugins: JavaScript in a Web Worker sandbox with the `pergel` API (the MCP tools). Built-ins:
  rename, lorem ipsum, palette, audit. Import a .js file; one run is one undo step. MCP
  `run_script` (Figma's `use_figma`).
- Live collaboration (multiplayer): Share → Start live session opens a WebSocket relay on the
  local network (port 3856, token in the link; MCP stays on 127.0.0.1). It shows cursors with
  names and the avatars of the people in the session. The whole document syncs and the last
  writer wins: two people editing the same moment keep the later change. There is no per-node
  merge (CRDT).

Round 5 (2026-10-23): the gaps from the comparison.
- Component properties: boolean, text and instance swap. They are defined on a component or set,
  linked on its layers, and set per instance. They import from Figma, and Code Connect writes them
  as JSX props.
- Several fills per layer (solid, gradient, image), drawn in order. CSS gets stacked backgrounds.
- Prototyping:
  - Open overlay: center, top, bottom, left or right, closing on a click outside, darkening the
    background.
  - Close overlay, Set variable, Conditional (if/else), Scroll to.
  - Present re-renders with the variables the prototype set.
- Text: bulleted and numbered lists, and OpenType features (tnum, liga, smcp, zero). Code gets
  `<ul>`/`<ol>` and `font-feature-settings`.
- Comments: threads pinned to layers or to the canvas, with replies, resolve and delete. They are
  saved in the file and sync in live sessions. MCP has `get_comments`, `add_comment` and
  `resolve_comment`.
- Figma plugins: a `figma` API shim. It covers nodes and properties, `create*`, `selection`,
  `findAll`, `showUI` (a sandboxed iframe with `pluginMessage`), `clientStorage` and
  `exportAsync`. A plugin imports from its folder (manifest.json). Figma's "Rectangle Creator"
  sample runs unchanged.
- Multiplayer merge: three-way, per top-level layer, with stable uids. Edits in different frames
  all survive; when both sides change the same frame, the later one wins.
- Branching: create a branch, review its changes (added, changed, deleted), merge into the main
  file. Versions are saved before and after the merge.
- Team libraries: publish, use (a managed copy linked to the library), update (instances keep
  their text). MCP has `get_libraries`, `use_library_component` and `publish_library`.
- FigJam, Slides and Sites, lite: sticky notes (a tool and a `sticky` spec), a new slide deck,
  slides exported as one PDF, and frames published as a linked HTML site.
- Figma links: paste one on the canvas to import it. MCP `import_figma` takes a link someone
  shared.

Still different from Figma:
- No cloud or accounts. Libraries are shared on one computer, live sessions on the local network.
- Multiplayer merges per top-level layer, not per property.
- Only plugins that use the supported API subset run.
- No widgets.
- FigJam, Slides and Sites are lite versions inside the design editor.
