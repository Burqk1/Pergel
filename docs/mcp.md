# Pergel MCP server

The desktop app runs an MCP server while it is open, so AI agents (Claude Code, Claude Desktop,
Cursor…) can look at the design you have open, implement it as code, and edit it.

- Address: `http://127.0.0.1:3855/mcp` (Streamable HTTP, local only; requests whose `Host` is not
  `localhost`, `127.0.0.1` or `::1` are rejected, which guards against DNS rebinding).
- Asset files: `http://127.0.0.1:3855/assets/<file>` (icons and images from the last design read).
- Main menu → **Copy Claude Code setup command** copies:

  ```sh
  claude mcp add --transport http pergel http://127.0.0.1:3855/mcp
  ```

Tool names follow Figma's Dev Mode MCP server, so prompts and agents written for Figma work here.
Layer ids are strings such as `"12"` (Figma-style `"0:12"` is accepted too). Tools with an
optional `nodeId` use the current selection when it is omitted. Positions are relative to the
parent frame. Tools work on the page shown in Pergel (`get_pages`, `switch_page`).

## Design to code: why the result matches

Agents reproducing a design tend to redraw icons themselves, swap in an icon library and guess
images. Pergel prevents that:

1. **Assets are files.** `get_design_context` finds every icon, logo, illustration (vector
   artwork, exported as SVG) and image fill (the original PNG/JPG) in the layer, deduplicates them
   (the same icon used five times is one file) and names them after the layer or component
   (`search.svg`, `hero-photo.jpg`). With `assets_dir` it writes them straight into the project;
   otherwise they are served at `http://127.0.0.1:3855/assets/…`.
2. **The code already uses them.** The reference code references each asset with `<img src>` (or a
   CSS background when other layers sit on the image), so there is nothing left to redraw.
3. **Explicit rules.** The answer starts with what to reuse and what to generate: assets are final;
   layout, text, colours, gradients and shadows are code with exact values; text is verbatim.
4. **Tokens, not magic numbers.** Properties bound to variables come out as CSS custom properties
   (`bg-[var(--color-primary,#7c3aed)]`) with a `:root` block of their values.
5. **Your components, not copies.** With Code Connect (`add_code_connect_map`) a Pergel component
   or component set maps to a component of the codebase; the reference code then writes
   `<Button variant="ghost">Cancel</Button>` with its import for every instance.
6. **Everything in one call.** Code, asset table, a screenshot of the design, the layer tree, fonts,
   reused components (build once), every text string and the design tokens.
7. **Verification.** `compare_to_design` takes a screenshot of the implementation and returns a
   match score, a side-by-side with the differences in red, and the design layers where they are
   (tolerant to 1 px anti-aliasing shifts).

## Reading

| Tool | Returns |
|---|---|
| `get_metadata` | The layer tree as XML: type, id, name, x, y, width, height, text characters, auto layout, image fills, gradients, component links. Whole page without `nodeId`. |
| `get_screenshot` | A PNG of a layer, the selection, or the whole page (`maxSize`, default 1024 px). |
| `get_design_context` | Implementation rules, assets (files), variables, Code Connect components, interactions, React + Tailwind (or `framework: "html"`) reference code, a screenshot, the layer tree, fonts, components, texts and tokens. `assets_dir` writes the asset files into the project, `assets_url` sets the prefix the code uses. |
| `download_assets` | Writes a layer's icons/illustrations (SVG) and images (original files) into `dir`. |
| `compare_to_design` | Match score, side-by-side diff image and the layers where an implementation screenshot (`image_path` or `image_base64`) differs from the design. |
| `get_variable_defs` | Variables the layer uses (`{"color/primary": "#7C3AED"}` in the active mode), collections and modes, plus raw colours, type styles, radii and spacing with counts. `page: true` lists all variables. |
| `search_icons` | Icon ids from Iconify (200,000+ open-source icons: lucide, mdi, tabler, heroicons, phosphor, brand logos…). |
| `get_selection` | The selected layers. |
| `search_design_system` | Finds main components (on every page, with variants and Code Connect), styles and variables by name, so a new screen reuses the file's own parts and tokens. |
| `import_figma` | Imports a figma.com link (a file, or one frame with `?node-id=`) into a new file, using the token saved in Pergel. |
| `get_comments` / `add_comment` / `resolve_comment` | Comment threads as a to-do list: read them, reply as Claude, resolve them. |
| `get_libraries` / `use_library_component` / `publish_library` | Team libraries: what is published, place a library component (it stays linked for updates), publish this file's components. |
| `create_new_file` | Opens a new, empty design file in a new tab, so a new design doesn't touch the user's open file. |
| `run_script` | Runs JavaScript against the document (like Figma's `use_figma`): `pergel.selection()`, `create(specs)`, `update(id, props)`, `remove(ids)`, `tool(name, args)`… Sandboxed in a worker; one undo step; console output and the return value come back. |
| `get_code_connect_suggestions` | Scans a codebase (`.tsx`/`.jsx`/`.vue`/`.svelte`, cva variants included) and proposes `add_code_connect_map` calls for the file's components, with variant → prop mappings. |
| `generate_diagram` | Mermaid flowchart → editable shapes, labels and arrows in ranked layout (`graph TD`/`LR`, `[ ]`, `( )`, `{ }`, `([ ])`, `(( ))`, `-->`, `---`, `-.->`, `==>`, `-->|label|`, `A & B`). |
| `get_components` | Main components (instance counts, variant values) and component sets with their properties. |
| `get_code_connect_map` | Code Connect mappings (Pergel component → codebase component, import, prop and text mappings). |
| `get_pages` | The file's pages. |

## Editing

Every edit is one undo step in Pergel.

| Tool | Does |
|---|---|
| `create_nodes` | Builds layers from node specs (below), optionally inside `parentId`, and selects them. |
| `update_node` | Changes properties of a layer (same names as a node spec). |
| `delete_nodes` | Deletes layers. |
| `select_nodes` | Selects layers and zooms to them. |
| `create_instance` | Places an instance of a main component. |
| `set_variables` | Creates/updates variables in a collection with per-mode values (Light/Dark), aliases (`"{color/bg}"`), and the active mode; bound layers update. |
| `bind_variable` | Binds fill, stroke, strokeWidth, radius, opacity, gap, padding, width, height, fontSize, text or visible of layers to a variable (or unbinds). |
| `switch_page` | Shows another page. |
| `set_variant` | Switches instances to another variant (`{"Size": "Large"}`), keeping their text. |
| `set_interactions` | Prototype links: on click/hover/after delay → navigate to a screen, back, a URL, or change an instance to another variant (interactive components), with transitions. |
| `add_code_connect_map` | Maps a main component / component set to a codebase component (name, import, variant → props, text layer → prop). |

Layers inside an instance can be restyled but not moved, resized or deleted, as in Figma.

## Node spec

```jsonc
{
  "type": "frame",              // frame | section | rectangle | ellipse | text | group | component | icon | svg | image | line | arrow
  "name": "Card",
  "x": 0, "y": 0,               // relative to the parent frame; ignored inside auto layout
  "width": 320, "height": 200,  // omit on auto-layout frames to hug; on text to fit the text
  "fill": "#FFFFFF",            // #RRGGBB, #RRGGBBAA or "none"
  "gradient": { "type": "linear", "angle": 180, "stops": [{ "color": "#7C3AED" }, { "color": "#2563EB" }] },
  "stroke": { "color": "#E5E7EB", "width": 1, "align": "inside" },
  "effects": [{ "type": "drop-shadow", "color": "#0000001F", "x": 0, "y": 8, "blur": 24 }],
  "radius": 12, "opacity": 1, "rotation": 0, "clip": true,
  "layout": { "direction": "vertical", "gap": 12, "padding": [16, 20], "justify": "start", "align": "center" },
  "sizing": { "horizontal": "fill", "vertical": "hug" },
  "text": "Hello", "fontSize": 16, "fontWeight": 600, "lineHeight": "150%", "textAlign": "left",
  "bind": { "fill": "color/surface", "radius": "radius/md" },
  "scroll": "vertical",         // frames: overflow scrolling (Present scrolls; code gets overflow-y: auto)
  "children": []
}
```

Real artwork instead of shapes:

```jsonc
{ "type": "icon", "icon": "lucide:arrow-right", "width": 24, "color": "#111827" }   // fetched from Iconify
{ "type": "svg", "svg": "<svg …>…</svg>", "width": 120 }                             // or "path": "C:/…/logo.svg"
{ "type": "image", "src": "C:/…/photo.jpg", "width": 400, "fit": "fill", "radius": 16 } // or an https URL / base64 "image"
```

Connectors and diagrams:

```jsonc
{ "type": "arrow", "points": [[0, 0], [120, 0], [120, 80]], "stroke": { "color": "#0D99FF", "width": 2 } }
{ "type": "line", "x": 0, "y": 40, "width": 200, "startPoint": "circle", "endPoint": "triangle-arrow" }
// end points: none | line-arrow | triangle-arrow | reversed-triangle | circle | diamond
```

Lines and arrows export as SVG files that include the stroke and arrowheads. Their `<img>`
gets a negative margin so it lines up with the layer.

Components take `"properties": [{ "name": "Label", "type": "text" | "boolean" | "instance-swap", "default": … }]`.
Their layers take `"property": { "text": "Label", "visible": "Show icon", "swap": "Icon" }`.
`create_instance` and `update_node` take `"properties": { "Label": "Buy now" }`. Text takes
`"list": "bullet" | "numbered"` and `"features": { "tnum": 1 }`. Any layer takes `"fills": [...]`
(more fills above `fill`). `"type": "sticky"` makes a FigJam note. `set_interactions` also takes
`overlay`, `close-overlay`, `set-variable`, `conditional` (with `then`/`else` action lists) and
`scroll-to`.

`update_node` also takes `note` (an annotation), `readyForDev`, `scroll`, `startPoint` and `endPoint`.

## In the app

SVG and image files can be dropped on the canvas, pasted (SVG markup or copied images) or placed
with **Ctrl+Shift+K**. SVGs become editable vector layers (paths, strokes, gradients, embedded
images); images become rectangles with an image fill. Images are stored inside the `.pergel` file.
**Import from Figma** (Home, or the main menu) brings a whole Figma file — pages, vectors, images,
text, auto layout, components — in through Figma's API with a personal access token.
