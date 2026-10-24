# Figma vs Pergel (2026-10-18)

Pergel's features, MCP server and performance compared with Figma Design (desktop 126.9.13) and
Figma's MCP server. Figma's side comes from its UI as observed (`figma-inventory.md`), the tools
its MCP server lists, and the measurements below. No Figma file was edited.

## Features

✓ = there, ◐ = partly, ✗ = missing.

| Area | Figma | Pergel |
|---|---|---|
| Frames, groups, sections, slices | ✓ | ✓ |
| Shapes, pen, pencil, vector edit, boolean, flatten, outline stroke, offset path, simplify, join | ✓ | ✓ |
| Text: mixed styles, resizing, lists, OpenType features, type on path | ✓ | ✓ |
| Fills: solid, gradients (4 kinds), images with adjustments, multiple fills per layer | ✓ | ✓ |
| Strokes: align, dashes, end points, individual sides; effects (shadows, blurs) | ✓ | ✓ |
| Auto layout (flex, wrap, grid), constraints, layout grids, rulers, guides, smart guides, measure | ✓ | ✓ |
| Components, instances, overrides, variants | ✓ | ✓ |
| Component properties (boolean, instance swap, text) | ✓ | ✓ (no slots) |
| Variables with modes, styles (text, colour, effect) | ✓ | ✓ |
| Prototyping: navigate, back, URL, transitions, scroll, interactive components | ✓ | ✓ |
| Prototyping: overlays, conditionals and variables in prototypes | ✓ | ✓ (no video) |
| Multiplayer | ✓ (cloud, per-property merge) | ◐ (LAN live session: cursors, follow and spotlight, merge per top-level layer by last change, per-person undo) |
| Comments, branching, team libraries | ✓ | ✓ (libraries shared on one computer; comments sync in live sessions) |
| Cloud files and accounts | ✓ | ✗ (local files, version history) |
| Plugins | ✓ (Figma plugin API, community) | ◐ (own `pergel` API, and Figma plugins that use the supported API subset) |
| FigJam, Slides, Sites | ✓ | ◐ (sticky notes and Mermaid diagrams; slide decks with PDF export; frames published as HTML) |
| Widgets, Make (AI) | ✓ | ✗ |
| Dev Mode: inspect, code, ready for dev, annotations, Code Connect | ✓ | ✓ (code panel, MCP) |
| Export PNG, JPG, SVG, PDF; copy as code | ✓ | ✓ |
| Import from Figma | — | ✓ (REST API: pages, vectors, images, text, auto layout, components, variants, prototypes) |
| Works offline, own files on disk | ◐ | ✓ |

## MCP server

| Need | Figma MCP | Pergel MCP |
|---|---|---|
| Design → code | `get_design_context` (reference code) | `get_design_context`: rules, an asset table with files, code with CSS variables and Code Connect components, texts, interactions, annotations, screenshot |
| Real icons and images instead of redrawn ones | `download_assets` | `download_assets`, assets written to the project or served at `/assets`; rule 1 "ASSETS ARE FINAL" in every context |
| Check the implementation | ✗ | `compare_to_design`: match %, diff image, the layers that differ |
| Icons that aren't in the file | ✗ | `search_icons`: 200,000+ Iconify icons, placed as real vectors |
| Screenshot, metadata, variables | ✓ | ✓ (`get_variable_defs` with modes) |
| Code Connect | map, suggestions, send mappings | map, `get_code_connect_suggestions` (scans .tsx/.jsx/.vue/.svelte and cva variants) |
| Search the design system | `search_design_system` | `search_design_system` (components on every page, styles, variables) |
| Write designs | `use_figma` (plugin JS), `generate_figma_design` | `run_script` (sandboxed JS, one undo step) plus structured `create_nodes` / `update_node` / `delete_nodes` / `create_instance` / `set_variant` / `set_variables` / `bind_variable` / `set_interactions` |
| Diagrams | `generate_diagram` (FigJam) | `generate_diagram` (Mermaid → editable layers on the canvas) |
| New file | `create_new_file` | `create_new_file`, `import_figma` (a shared figma.com link) |
| Pages | ✗ | `get_pages`, `switch_page` |
| Prompts | skills | `implement_design`, `design_screen` |
| Libraries | `get_libraries` | `get_libraries`, `use_library_component`, `publish_library` |
| Comments | ✗ | `get_comments`, `add_comment`, `resolve_comment` |
| AI images, shaders, video, FigJam | ✓ | ✗ |
| Runs | Figma's cloud (account, rate limits) | locally, on 127.0.0.1:3855 (no account, no limits) |

## Performance

Machine: Windows 11, NVIDIA RTX 3080, 8 logical cores. The machine was not idle: Discord,
Spotify and Steam were running, at 77% CPU load. Pergel's figures use the release WASM build.

### Pergel renderer

Headless Chromium (ANGLE/D3D11) on a 744×680 canvas. Median of 3 rounds, after a warm-up, with
`gl.finish()` (so GPU time counts).

| Layers | Build scene | Whole scene in view | Zoomed in 4× (visible layers) |
|---|---|---|---|
| 1,000 | 1 ms | 1.2 ms/frame (826 fps) | 0.16 ms (118 visible) |
| 10,000 | 13 ms | 8.9 ms (113 fps) | 1.3 ms (1,074 visible) |
| 100,000 | 139 ms | 22 ms (45 fps) | 11.6 ms (86 fps, 10,481 visible) |

| Operation | Time |
|---|---|
| Hit test (pick) on 100,000 layers | < 1 µs |
| Save / open a 10,000-layer document (3.75 MB JSON) | 18–21 ms / 43–64 ms |
| `create_nodes`, 1,000 layers (in the browser) | 51–72 ms |
| `run_script` creating 10,000 layers through the live MCP server (desktop app) | 1.37 s |
| `get_metadata` / `get_design_context` / `get_screenshot`, 1,000 layers | 1–3 ms / 6–10 ms / 38–48 ms |
| Page screenshot through the live MCP server (10,000 layers) | 343 ms round trip |
| WASM heap with 100,000 layers | 109 MB |

### Desktop apps

| | Figma 126.9.13 | Pergel (debug build) |
|---|---|---|
| On disk | 365 MB per version (882 MB with the 3 versions installed) | 18.7 MB debug exe + 11.4 MB engine (4.3 MB gzipped); a release build is smaller |
| Start → window / ready | window after 2.3 s | MCP server after 0.6 s, engine answering about 1 s later |
| Memory (working set / private) | 3,065 MB / 2,693 MB in 11 processes (its last session restored, with the "Web Developer Portfolio Hero Section" community file open) | 528 MB / 318 MB with a 53-layer file; 589 MB / 369 MB with a 10,000-layer file (8 processes, most of it WebView2) |
| Memory ceiling | 2 GB per file tab ([Figma forum](https://forum.figma.com/archive-21/figma-memory-alert-31203)) | the WASM heap grows as needed (wasm32: up to 4 GB) |
| Renderer | C++ → WASM, WebGPU with WebGL fallback ([Figma blog](https://www.figma.com/blog/figma-rendering-powered-by-webgpu/)) | Rust → WASM, Skia on WebGL2, an LOD mesh for zoomed-out views |

Not measured: a frame-rate comparison on the same file in Figma. Scripting Figma's canvas would
mean creating or editing a file in the user's account, so it was left for the user to approve.
It can be run from a new draft through Figma's MCP (`use_figma`).
