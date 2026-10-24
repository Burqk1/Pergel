//! Pergel's MCP server (Streamable HTTP on 127.0.0.1). Tool names follow Figma's Dev Mode MCP
//! server so agents and prompts written for it work here too. Each call is forwarded to the
//! editor window, which runs it against the open document and replies.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use rmcp::handler::server::{router::prompt::PromptRouter, router::tool::ToolRouter, wrapper::Parameters};
use rmcp::model::{CallToolResult, ContentBlock, Implementation, ServerCapabilities, ServerConfig};
use rmcp::transport::streamable_http_server::{
    StreamableHttpServerConfig, StreamableHttpService, session::local::LocalSessionManager,
};
use rmcp::model::{PromptMessage, Role};
use rmcp::{ErrorData, ServerHandler, prompt, prompt_handler, prompt_router, schemars, tool, tool_handler, tool_router};
use axum::extract::{Path as AxumPath, State};
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use tokio::sync::oneshot;

use crate::access::{Access, Use};

pub const PORT: u16 = 3855;
const TIMEOUT: Duration = Duration::from_secs(45);

/// Forwards tool calls to the webview and waits for its answer.
pub struct Bridge {
    app: AppHandle,
    pending: Mutex<HashMap<u64, oneshot::Sender<Value>>>,
    next: AtomicU64,
    /// Asset files served at /assets/<file> (mime type, bytes), from the last design reads.
    assets: Mutex<HashMap<String, (String, Vec<u8>)>>,
    /// Folders agents may read and write.
    access: Access,
}

#[derive(Clone, Serialize)]
pub struct Status {
    pub url: String,
    pub running: bool,
    pub error: Option<String>,
}

impl Bridge {
    pub fn new(app: AppHandle) -> Self {
        let access = Access::new(app.clone());
        Self { app, pending: Mutex::new(HashMap::new()), next: AtomicU64::new(1), assets: Mutex::new(HashMap::new()), access }
    }

    async fn call(&self, tool: &str, args: Value) -> Result<Value, String> {
        let id = self.next.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = oneshot::channel();
        self.pending.lock().map_err(|e| e.to_string())?.insert(id, tx);
        // Scripts set their own limit (up to 5 minutes); everything else answers quickly.
        let limit = if tool == "import_figma" {
            Duration::from_secs(300)
        } else if tool == "run_script" {
            let ms = args.get("timeoutMs").and_then(Value::as_u64).unwrap_or(60_000).min(300_000);
            Duration::from_millis(ms + 5_000)
        } else {
            TIMEOUT
        };
        // The window runs calls one at a time; one still queued when this side gives up is
        // skipped there instead of running late.
        let wait = limit.as_millis() as u64;
        self.app
            .emit_to("main", "mcp-request", json!({ "id": id, "tool": tool, "args": &args, "timeoutMs": wait }))
            .map_err(|e| format!("Pergel's window is not available: {e}"))?;
        let reply = match tokio::time::timeout(limit, rx).await {
            Ok(Ok(reply)) => reply,
            _ => {
                self.pending.lock().ok().map(|mut p| p.remove(&id));
                return Err("Pergel did not answer in time, so the call may still have run. Check the document (get_metadata) before retrying. Is a document open in the Pergel window?".into());
            }
        };
        if reply.get("ok").and_then(Value::as_bool) == Some(true) {
            Ok(reply.get("result").cloned().unwrap_or(Value::Null))
        } else {
            Err(reply.get("error").and_then(Value::as_str).unwrap_or("Unknown error").to_string())
        }
    }

    pub fn respond(&self, id: u64, reply: Value) {
        if let Some(tx) = self.pending.lock().ok().and_then(|mut p| p.remove(&id)) {
            let _ = tx.send(reply);
        }
    }
}

fn text(v: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![ContentBlock::text(v)]))
}

fn failure(message: String) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::error(vec![ContentBlock::text(message)]))
}

fn pretty(v: &Value) -> String {
    serde_json::to_string_pretty(v).unwrap_or_default()
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageArgs {
    #[schemars(description = "Layer id to describe. Omit for the whole page.")]
    pub node_id: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotArgs {
    #[schemars(description = "Layer id. Defaults to the selection, or the whole page when nothing is selected.")]
    pub node_id: Option<String>,
    #[schemars(description = "Longest side of the image in pixels (64–2048, default 1024).")]
    pub max_size: Option<u32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateArgs {
    #[schemars(description = "Frame or group to create the layers in. Omit to place them on the canvas.")]
    pub parent_id: Option<String>,
    #[schemars(description = "Layers to create; see the tool description for the node spec format.")]
    pub nodes: Vec<Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateArgs {
    #[schemars(description = "Layer id to change.")]
    pub node_id: String,
    #[schemars(description = "Properties to change, same names as in a create_nodes spec (x, y, width, height, fill, stroke, radius, opacity, rotation, name, visible, layout, sizing, text, fontSize, fontWeight, fontFamily, lineHeight, letterSpacing, textAlign, note, scroll, startPoint, endPoint) plus readyForDev (true/false). Positions are relative to the parent frame.")]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IdsArgs {
    #[schemars(description = "Layer ids.")]
    pub node_ids: Vec<String>,
    #[schemars(description = "select_nodes only: zoom the view to the layers (default true).")]
    pub zoom: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstanceArgs {
    #[schemars(description = "Main component id (from get_components).")]
    pub component_id: String,
    #[schemars(description = "Frame to place the instance in. Omit for the canvas.")]
    pub parent_id: Option<String>,
    #[schemars(description = "Position relative to the parent. Omit to place it in view.")]
    pub x: Option<f32>,
    pub y: Option<f32>,
}

/// The node spec format, shared by the server instructions and create_nodes.
macro_rules! node_spec {
    () => {
        r##"Each node spec is a JSON object:
{
  "type": "frame" | "section" | "rectangle" | "ellipse" | "text" | "group" | "component" | "icon" | "svg" | "image" | "line" | "arrow" | "sticky",   (required; sticky = FigJam note with `text`)
  "name": "Card",
  "x": 0, "y": 0,                 position relative to the parent frame (ignored inside auto layout)
  "width": 320, "height": 200,    omit on auto-layout frames to hug, and on text to size to the text
  "fill": "#RRGGBB" | "#RRGGBBAA" | "none",
  "gradient": { "type": "linear" | "radial" | "angular" | "diamond", "angle": 180, "stops": [{ "color": "#7C3AED", "position": 0 }, { "color": "#2563EB", "position": 1 }] },
  "stroke": { "color": "#E5E7EB", "width": 1, "align": "inside" | "center" | "outside" },
  "effects": [{ "type": "drop-shadow" | "inner-shadow" | "layer-blur" | "background-blur", "color": "#00000026", "x": 0, "y": 4, "blur": 12, "spread": 0 }],
  "radius": 12, "corners": [tl, tr, br, bl], "opacity": 0.9, "rotation": 0, "clip": true,
  "layout": {                     auto layout (frames)
    "direction": "horizontal" | "vertical" | "wrap" | "grid",
    "gap": 12, "counterGap": 12, "padding": 16 | [v, h] | [t, r, b, l],
    "justify": "start" | "center" | "end" | "space-between",
    "align": "start" | "center" | "end" | "baseline",
    "columns": 3, "rows": 2       (grid)
  },
  "sizing": { "horizontal": "fixed" | "hug" | "fill", "vertical": "fixed" | "hug" | "fill" },
  "absolute": false,              inside auto layout: keep its own position
  "constraints": { "horizontal": "left" | "right" | "left-right" | "center" | "scale", "vertical": "top" | "bottom" | "top-bottom" | "center" | "scale" },
  "text": "Hello", "fontFamily": "Inter", "fontSize": 16, "fontWeight": 400..900, "italic": false,
  "lineHeight": 24 | "150%" | "auto", "letterSpacing": 0 (percent), "textAlign": "left" | "center" | "right" | "justify",
  "verticalAlign": "top" | "middle" | "bottom", "autoResize": "width" | "height" | "none",
  "bind": { "fill": "color/primary", "radius": "radius/md" },   properties driven by variables (see set_variables)
  "note": "Opens the checkout sheet",   an annotation for developers
  "layoutGrids": [{ "kind": "columns" | "rows" | "grid", "count": 12, "gutter": 24, "margin": 80 }],   frames' layout guides
  "scroll": "vertical" | "horizontal" | "both",   frames: content past the frame scrolls (prototype + overflow auto in code)
  "points": [[0, 0], [120, 0], [120, 80]],   line / arrow: the polyline, relative to the parent (or use x, y, width, height)
  "startPoint": "none", "endPoint": "line-arrow" | "triangle-arrow" | "reversed-triangle" | "circle" | "diamond",   line / arrow ends
  "children": [ ...more node specs... ]
}
Real artwork — never draw icons out of rectangles and ellipses:
  icon:  { "type": "icon", "icon": "lucide:arrow-right", "width": 24, "color": "#111827" }   any Iconify id; find ids with search_icons
  svg:   { "type": "svg", "svg": "<svg …>…</svg>", "width": 120 }   or "path": "C:/…/logo.svg"
  image: { "type": "image", "src": "C:/…/photo.jpg" | "https://…/photo.jpg", "width": 400, "height": 240, "fit": "fill" | "fit" | "crop" | "tile", "radius": 16 }
Text defaults to black Inter 16, rectangles and ellipses to #D9D9D9, frames to white."##
    };
}

const NODE_SPEC: &str = node_spec!();

const GUIDE: &str = "Follow these rules; they are what makes the result match the design:
1. ASSETS ARE FINAL. Every icon, logo, illustration and image of the design is listed under Assets with a file. Use exactly those files: \
reference the URLs, or call download_assets to write them into the project. Do NOT redraw them as your own inline SVG, do NOT substitute an \
icon library (lucide-react, heroicons, react-icons, Font Awesome, Material Icons…), emoji, CSS shapes or placeholder images, and do not restyle them.
2. EVERYTHING ELSE IS CODE. Layout, text, colours, borders, radii, gradients and shadows are in the reference code with exact values; keep \
them (px, hex, font weight, line height, letter spacing). Adapt the markup to the project's framework and components, not the values.
3. TEXT IS VERBATIM. Copy every string exactly (case, punctuation, line breaks).
4. LAYOUT. flex/grid in the reference comes from auto layout; absolute positions come from free-form frames. You may turn absolute layouts \
into flex/grid only when the rendered result stays identical.
5. VERIFY. Render your implementation at the design's size, screenshot it and call compare_to_design; fix what it reports until the match \
is 95% or better.";

#[derive(Clone)]
pub struct PergelMcp {
    bridge: Arc<Bridge>,
    #[allow(dead_code, reason = "used by the tool_handler macro")]
    tool_router: ToolRouter<Self>,
    #[allow(dead_code, reason = "used by the prompt_handler macro")]
    prompt_router: PromptRouter<Self>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImplementPromptArgs {
    #[schemars(description = "Layer id of the screen/component to build (default: the selection).")]
    pub node_id: Option<String>,
    #[schemars(description = "Folder of the project that serves static files, e.g. C:/app/public/assets.")]
    pub assets_dir: Option<String>,
    #[schemars(description = "react (default) or html.")]
    pub framework: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DesignPromptArgs {
    #[schemars(description = "What to design, e.g. \"a mobile login screen for a banking app\".")]
    pub brief: String,
}

#[prompt_router]
impl PergelMcp {
    #[prompt(name = "implement_design", description = "Implement the selected Pergel design in this codebase exactly: real asset files, tokens, codebase components, then verify against the design.")]
    async fn implement_design(&self, Parameters(a): Parameters<ImplementPromptArgs>) -> Vec<PromptMessage> {
        let target = a.node_id.map_or("the layer selected in Pergel".to_string(), |id| format!("Pergel layer {id}"));
        let assets = a.assets_dir.unwrap_or_else(|| "the folder this project serves static files from (e.g. public/assets)".to_string());
        let fw = a.framework.unwrap_or_else(|| "react".into());
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Implement {target} in this codebase so it looks exactly like the design.\n\n\
                 1. Call get_design_context (framework {fw}, assets_dir {assets}). Read its rules first.\n\
                 2. Use the asset files it lists for every icon, logo, illustration and image. Never draw your own SVGs, never use an icon library or emoji instead.\n\
                 3. If it lists Code Connect components, use those components with the given props. If the design reuses components that are not mapped, build each once and reuse it; map them with add_code_connect_map afterwards.\n\
                 4. Keep every value from the reference code (sizes, spacing, colours, fonts, radii, shadows); turn variables into the project's tokens of the same role.\n\
                 5. Copy all text verbatim.\n\
                 6. Render the result at the design's size, take a screenshot, and call compare_to_design with it. Fix what it reports and repeat until the match is 95% or better.\n\
                 7. If the design has interactions, wire them up as navigation."
            ),
        )]
    }

    #[prompt(name = "design_screen", description = "Design a screen in Pergel with real icons, variables and auto layout, then check it with a screenshot.")]
    async fn design_screen(&self, Parameters(a): Parameters<DesignPromptArgs>) -> Vec<PromptMessage> {
        vec![PromptMessage::new_text(
            Role::User,
            format!(
                "Design this in Pergel: {}\n\n\
                 - Start with get_pages/get_metadata to see what exists, and search_design_system (or get_components / get_variable_defs with page: true) to reuse the file's components, styles and tokens.\n\
                 - For flows, architecture or process diagrams use generate_diagram with Mermaid source instead of placing boxes and arrows by hand.\n\
                 - Define colours, radii and spacing as variables with set_variables (Light and Dark modes when it fits) and bind layers to them.\n\
                 - Build with create_nodes using auto layout frames. For icons call search_icons and use {{ \"type\": \"icon\" }} nodes — never draw icons from rectangles or ellipses. Use real images ({{ \"type\": \"image\", \"src\": … }}) where the design needs photos.\n\
                 - Make repeated elements components (variants for states) and use instances.\n\
                 - Add set_interactions for the main flow if there are several screens.\n\
                 - Check your work with get_screenshot and fix spacing, alignment and contrast until it looks polished.",
                a.brief
            ),
        )]
    }
}

/// Turns a folder inside a web project into the URL prefix its files are served at:
/// `…/public/assets/icons` → `/assets/icons/`; anything else → `./<folder name>/`.
fn url_for_dir(dir: &Path) -> String {
    let parts: Vec<String> = dir.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect();
    if let Some(i) = parts.iter().rposition(|p| p == "public" || p == "static") {
        let rest = parts[i + 1..].join("/");
        return if rest.is_empty() { "/".into() } else { format!("/{rest}/") };
    }
    format!("./{}/", parts.last().cloned().unwrap_or_else(|| "assets".into()))
}

impl PergelMcp {
    pub fn new(bridge: Arc<Bridge>) -> Self {
        Self { bridge, tool_router: Self::tool_router(), prompt_router: Self::prompt_router() }
    }

    async fn json_tool(&self, tool: &str, args: Value) -> Result<CallToolResult, ErrorData> {
        match self.bridge.call(tool, args).await {
            Ok(v) => text(pretty(&v)),
            Err(e) => failure(e),
        }
    }

    /// Writes the assets in `list` (with base64 `data`) into `dir`; returns the written paths.
    fn write_assets(dir: &Path, list: &[Value], overwrite: bool) -> Result<Vec<String>, String> {
        std::fs::create_dir_all(dir).map_err(|e| format!("Could not create {}: {e}", dir.display()))?;
        let mut written = Vec::new();
        for a in list {
            let (Some(file), Some(data)) = (a["file"].as_str(), a["data"].as_str()) else { continue };
            // Names come from layer names: a bare file name only, never a path out of `dir`.
            let mut parts = Path::new(file).components();
            if !matches!((parts.next(), parts.next()), (Some(std::path::Component::Normal(_)), None)) {
                return Err(format!("{file:?} is not a plain file name"));
            }
            let bytes = B64.decode(data).map_err(|e| e.to_string())?;
            let path = dir.join(file);
            if path.exists() && !overwrite && std::fs::read(&path).ok().as_deref() != Some(&bytes[..]) {
                return Err(format!("{} already exists with different content; pass overwrite: true or another folder", path.display()));
            }
            std::fs::write(&path, &bytes).map_err(|e| format!("Could not write {}: {e}", path.display()))?;
            written.push(path.display().to_string());
        }
        Ok(written)
    }

    /// Keeps the assets for `/assets/<file>` and drops their data from `list`.
    fn cache_assets(&self, list: &mut [Value]) {
        if let Ok(mut cache) = self.bridge.assets.lock() {
            for a in list.iter_mut() {
                if let (Some(file), Some(data)) = (a["file"].as_str(), a["data"].as_str())
                    && let Ok(bytes) = B64.decode(data)
                {
                    let mime = a["mime"].as_str().unwrap_or("application/octet-stream").to_string();
                    cache.insert(file.to_string(), (mime, bytes));
                }
                if let Some(o) = a.as_object_mut() {
                    o.remove("data");
                }
            }
        }
    }

    /// A spec's local file (image `src`/`path`, svg `path`); remote URLs and icon ids are
    /// fetched by the editor window.
    fn local_file(spec: &Value) -> Option<String> {
        let local = |v: &Value| v.as_str().filter(|s| !s.starts_with("http://") && !s.starts_with("https://") && !s.starts_with("data:")).map(str::to_string);
        local(&spec["path"]).or_else(|| local(&spec["src"]))
    }

    /// Every local file in node specs, in order.
    fn local_files(spec: &Value, out: &mut Vec<String>) {
        if let Some(children) = spec.get("children").and_then(Value::as_array) {
            for c in children {
                Self::local_files(c, out);
            }
        }
        out.extend(Self::local_file(spec));
    }

    /// Reads the local files in node specs into them before they reach the editor. `allowed`
    /// maps each path to where it resolved once the user allowed its folder.
    fn resolve_files(spec: &mut Value, allowed: &HashMap<String, PathBuf>) -> Result<(), String> {
        if let Some(children) = spec.get_mut("children").and_then(Value::as_array_mut) {
            for c in children {
                Self::resolve_files(c, allowed)?;
            }
        }
        let kind = spec["type"].as_str().unwrap_or("").to_ascii_lowercase();
        let Some(given) = Self::local_file(spec) else { return Ok(()) };
        let path = allowed.get(&given).ok_or_else(|| format!("{given} was not checked"))?;
        let bytes = std::fs::read(path).map_err(|e| format!("Could not read {}: {e}", path.display()))?;
        let is_svg = kind == "svg" || path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
        if is_svg {
            spec["svg"] = json!(String::from_utf8_lossy(&bytes));
            if spec["name"].is_null()
                && let Some(stem) = path.file_stem()
            {
                spec["name"] = json!(stem.to_string_lossy());
            }
        } else {
            spec["image"] = json!(B64.encode(&bytes));
            if spec["name"].is_null()
                && let Some(stem) = path.file_stem()
            {
                spec["name"] = json!(stem.to_string_lossy());
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DesignArgs {
    #[schemars(description = "Layer id (a frame/screen/component). Defaults to the current selection.")]
    pub node_id: Option<String>,
    #[schemars(description = "\"react\" (React + Tailwind, default) or \"html\" (HTML + inline CSS).")]
    pub framework: Option<String>,
    #[schemars(description = "Absolute folder in the user's project to write the design's assets into (e.g. C:/app/public/assets). The code then references them there. Omit to get http://127.0.0.1:3855/assets/… URLs instead.")]
    pub assets_dir: Option<String>,
    #[schemars(description = "URL prefix the code should use for files in assets_dir (default: derived from the folder, e.g. /assets/ for public/assets).")]
    pub assets_url: Option<String>,
    #[schemars(description = "Attach a screenshot of the design (default true).")]
    pub screenshot: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssetsArgs {
    #[schemars(description = "Layer id whose assets to export. Defaults to the current selection.")]
    pub node_id: Option<String>,
    #[schemars(description = "Absolute folder to write the files into (created if missing), e.g. C:/app/public/assets.")]
    pub dir: String,
    #[schemars(description = "Replace files that already exist with different content (default false).")]
    pub overwrite: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompareArgs {
    #[schemars(description = "Layer id of the design to compare against. Defaults to the selection.")]
    pub node_id: Option<String>,
    #[schemars(description = "Absolute path of a PNG/JPEG screenshot of the implementation (same area as the design; any device pixel ratio).")]
    pub image_path: Option<String>,
    #[schemars(description = "The screenshot as base64 instead of a path.")]
    pub image_base64: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetVariablesArgs {
    #[schemars(description = "Collection name (created if missing), e.g. \"Theme\".")]
    pub collection: String,
    #[schemars(description = "Variables to create or update: [{ \"name\": \"color/primary\", \"value\": \"#7C3AED\" }, { \"name\": \"color/bg\", \"modes\": { \"Light\": \"#FFFFFF\", \"Dark\": \"#0B0B0F\" } }, { \"name\": \"radius/md\", \"value\": 8 }, { \"name\": \"color/card\", \"value\": \"{color/bg}\" }]. Colours are #RRGGBB(AA); {name} aliases another variable.")]
    pub variables: Vec<Value>,
    #[schemars(description = "Mode to show afterwards (e.g. \"Dark\").")]
    pub active_mode: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BindArgs {
    #[schemars(description = "Layer ids to bind.")]
    pub node_ids: Vec<String>,
    #[schemars(description = "fill, stroke, strokeWidth, radius, opacity, gap, padding, width, height, fontSize, text or visible.")]
    pub property: String,
    #[schemars(description = "Variable name (e.g. \"color/primary\"); omit to unbind.")]
    pub variable: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VariantArgs {
    #[schemars(description = "Instance ids to switch.")]
    pub node_ids: Vec<String>,
    #[schemars(description = "Variant properties to set, e.g. { \"Size\": \"Large\", \"State\": \"Hover\" } (see get_components → componentSets).")]
    pub properties: serde_json::Map<String, Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InteractionArgs {
    #[schemars(description = "Layer (hotspot) id.")]
    pub node_id: String,
    #[schemars(description = "Replaces the layer's interactions: [{ \"trigger\": \"click\" | \"hover\" | \"after-delay\", \"delay\": 800, \"action\": \"navigate\" | \"back\" | \"url\" | \"change-to\", \"to\": \"<screen name or id; for change-to a variant id or values like State=Hover>\", \"url\": \"https://…\", \"transition\": \"instant\" | \"dissolve\" | \"smart-animate\" | \"slide-left\" | \"slide-right\" | \"slide-up\" | \"slide-down\" | \"push\", \"duration\": 300 }]. An empty list removes them.")]
    pub interactions: Vec<Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeConnectArgs {
    #[schemars(description = "Main component or component set id (get_components).")]
    pub node_id: String,
    #[schemars(description = "The component's name in code, e.g. \"Button\".")]
    pub component: String,
    #[schemars(description = "Import line, e.g. import { Button } from \"@/components/ui/button\".")]
    pub import: Option<String>,
    #[schemars(description = "Source file path in the codebase.")]
    pub source: Option<String>,
    #[schemars(description = "Variant property → value → JSX attributes, e.g. { \"State\": { \"Primary\": \"variant=\\\"default\\\"\", \"Ghost\": \"variant=\\\"ghost\\\"\" } }.")]
    pub props: Option<Value>,
    #[schemars(description = "Text layer name → prop that receives its text (\"children\" puts it between the tags), e.g. { \"Label\": \"children\" }.")]
    pub text: Option<Value>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PageSwitchArgs {
    #[schemars(description = "Page name (case-insensitive).")]
    pub name: Option<String>,
    #[schemars(description = "Page index from get_pages (used when name is omitted).")]
    pub index: Option<u32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FigmaImportArgs {
    #[schemars(description = "The figma.com link (a file, or one frame of it with ?node-id=…).")]
    pub url: String,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LibraryUseArgs {
    #[schemars(description = "Library name or id (from get_libraries). Omit for the first one.")]
    pub library: Option<String>,
    #[schemars(description = "Component name or key.")]
    pub component: String,
    #[schemars(description = "Canvas position. Omit for the middle of the view.")]
    pub x: Option<f32>,
    pub y: Option<f32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommentsArgs {
    #[schemars(description = "Also list resolved threads (default false).")]
    pub include_resolved: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddCommentArgs {
    #[schemars(description = "The comment.")]
    pub text: String,
    #[schemars(description = "Reply in this thread (from get_comments).")]
    pub thread_id: Option<String>,
    #[schemars(description = "Start a thread pinned to this layer.")]
    pub node_id: Option<String>,
    #[schemars(description = "Or start a thread at a canvas point.")]
    pub x: Option<f32>,
    pub y: Option<f32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResolveArgs {
    pub thread_id: String,
    #[schemars(description = "false reopens the thread.")]
    pub resolved: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NewFileArgs {
    #[schemars(description = "Name of the new design file (default \"Untitled\").")]
    pub name: Option<String>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScriptArgs {
    #[schemars(description = "JavaScript (an async function body) using the `pergel` API; `return` a value to get it back.")]
    pub code: String,
    #[schemars(description = "Milliseconds before the script is stopped (default 60000, max 300000).")]
    pub timeout_ms: Option<u32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestArgs {
    #[schemars(description = "Root folder of the codebase to scan for components (.tsx/.jsx/.vue/.svelte), e.g. C:/app.")]
    pub project_dir: String,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagramArgs {
    #[schemars(description = "A Mermaid flowchart: \"graph TD\" / \"flowchart LR\", nodes A[Box], B(Rounded), C{Decision}, D([Pill]), E((Circle)), links -->, ---, -.->, ==>, with labels -->|Yes| or -- No -->, and A & B --> C.")]
    pub mermaid: String,
    #[schemars(description = "Name of the diagram frame (default \"Diagram\").")]
    pub name: Option<String>,
    #[schemars(description = "Canvas position. Omit to place it right of the existing content.")]
    pub x: Option<f32>,
    pub y: Option<f32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DesignSearchArgs {
    #[schemars(description = "Words to look for in component, component set, style and variable names, e.g. \"button primary\", \"color/brand\", \"heading\". Empty lists everything.")]
    pub query: String,
    #[schemars(description = "Limit to \"components\", \"styles\" and/or \"variables\".")]
    pub types: Option<Vec<String>>,
    #[schemars(description = "Results per type (default 20, max 100).")]
    pub limit: Option<u32>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VarDefsArgs {
    #[schemars(description = "Layer id whose variables to list. Defaults to the selection.")]
    pub node_id: Option<String>,
    #[schemars(description = "true: every variable in the file, not just the ones a layer uses.")]
    pub page: Option<bool>,
}

#[derive(Debug, Default, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IconSearchArgs {
    #[schemars(description = "What to look for, e.g. \"arrow right\", \"shopping cart\", \"github\".")]
    pub query: String,
    #[schemars(description = "Limit to icon sets, comma separated (e.g. \"lucide,mdi,tabler,logos\").")]
    pub prefixes: Option<String>,
    #[schemars(description = "How many results (default 24, max 100).")]
    pub limit: Option<u32>,
}

#[tool_router]
impl PergelMcp {
    #[tool(
        description = "Layer tree of the page (or of one layer) as XML: type, id, name, position relative to the parent, size, text characters, auto layout, image fills, gradients and component links. Start here to find layer ids."
    )]
    async fn get_metadata(&self, Parameters(a): Parameters<PageArgs>) -> Result<CallToolResult, ErrorData> {
        match self.bridge.call("get_metadata", json!({ "nodeId": a.node_id })).await {
            Ok(v) => text(v.get("xml").and_then(Value::as_str).unwrap_or_default()),
            Err(e) => failure(e),
        }
    }

    #[tool(description = "PNG screenshot of a layer, the selection, or the whole page, rendered by Pergel's engine.")]
    async fn get_screenshot(&self, Parameters(a): Parameters<ScreenshotArgs>) -> Result<CallToolResult, ErrorData> {
        match self.bridge.call("get_screenshot", json!({ "nodeId": a.node_id, "maxSize": a.max_size })).await {
            Ok(v) => {
                let png = v.get("png").and_then(Value::as_str).unwrap_or_default().to_string();
                let caption = format!(
                    "Screenshot of {} ({}×{} px)",
                    v.get("label").and_then(Value::as_str).unwrap_or("the design"),
                    v.get("width").and_then(Value::as_u64).unwrap_or(0),
                    v.get("height").and_then(Value::as_u64).unwrap_or(0),
                );
                Ok(CallToolResult::success(vec![ContentBlock::image(png, "image/png"), ContentBlock::text(caption)]))
            }
            Err(e) => failure(e),
        }
    }

    #[tool(
        description = "Everything needed to implement a layer exactly, in one call: implementation rules, the design's assets (icons, logos, illustrations and images as real SVG/PNG/JPG files — use them, never redraw them), reference React + Tailwind (or HTML + CSS) code with exact values that already references those files, a screenshot, the layer tree, fonts, reused components, all text strings and design tokens. Pass assets_dir to write the asset files straight into the project."
    )]
    async fn get_design_context(&self, Parameters(a): Parameters<DesignArgs>) -> Result<CallToolResult, ErrorData> {
        let dir = match a.assets_dir.as_deref() {
            Some(d) => match self.bridge.access.check(Path::new(d), Use::Write).await {
                Ok(d) => Some(d),
                Err(e) => return failure(e),
            },
            None => None,
        };
        let base = match (&dir, &a.assets_url) {
            (_, Some(url)) => if url.ends_with('/') { url.clone() } else { format!("{url}/") },
            (Some(d), None) => url_for_dir(d),
            (None, None) => format!("http://127.0.0.1:{PORT}/assets/"),
        };
        let mut v = match self
            .bridge
            .call("get_design_context", json!({ "nodeId": a.node_id, "framework": a.framework, "assetBase": base }))
            .await
        {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let mut assets: Vec<Value> = v["assets"].as_array().cloned().unwrap_or_default();
        let written = match &dir {
            Some(d) => match Self::write_assets(d, &assets, false) {
                Ok(w) => w,
                Err(e) => return failure(e),
            },
            None => Vec::new(),
        };
        self.cache_assets(&mut assets);
        let node = &v["node"];
        let name = node["name"].as_str().unwrap_or("the design").to_string();
        let lang = if v["framework"] == "html" { "html" } else { "tsx" };
        let mut out = format!(
            "# Implement \"{name}\" ({}×{} px) exactly\n\n{GUIDE}\n",
            node["width"].as_f64().unwrap_or(0.0).round(),
            node["height"].as_f64().unwrap_or(0.0).round()
        );
        if assets.is_empty() {
            out.push_str("\n## Assets\nNone: this design has no icons or images; everything is code.\n");
        } else {
            let _ = write!(out, "\n## Assets ({}) — use these files, do not recreate them\n| file | kind | size | layers | src in code |\n|---|---|---|---|---|\n", assets.len());
            for asset in &assets {
                let ids: Vec<&str> = asset["nodeIds"].as_array().map_or(Vec::new(), |a| a.iter().filter_map(Value::as_str).collect());
                let _ = writeln!(
                    out,
                    "| {} | {}{} | {}×{} | {} | {}{} |",
                    asset["file"].as_str().unwrap_or(""),
                    asset["kind"].as_str().unwrap_or(""),
                    if asset["background"] == true { " (background)" } else { "" },
                    asset["width"].as_f64().unwrap_or(0.0).round(),
                    asset["height"].as_f64().unwrap_or(0.0).round(),
                    ids.join(", "),
                    base,
                    asset["file"].as_str().unwrap_or("")
                );
            }
            if written.is_empty() {
                let _ = writeln!(out, "\nThe URLs work while Pergel is open. To copy the files into the project call download_assets with the folder the app serves (e.g. public/assets) and change the prefix {base} accordingly.");
            } else {
                let _ = writeln!(out, "\nWritten to the project:\n{}", written.iter().map(|w| format!("- {w}")).collect::<Vec<_>>().join("\n"));
            }
        }
        if let Some(list) = v["variables"].as_array().filter(|l| !l.is_empty()) {
            out.push_str("\n## Variables (design tokens) — the code uses them as CSS custom properties\nMap each to the project's existing token of the same role if there is one; otherwise define them:\n```css\n:root {\n");
            for t in list {
                let _ = writeln!(out, "  {}: {}; /* {} */", t["css"].as_str().unwrap_or(""), t["value"].as_str().unwrap_or(""), t["name"].as_str().unwrap_or(""));
            }
            out.push_str("}\n```\n");
        }
        let fonts: Vec<&str> = v["fonts"].as_array().map_or(Vec::new(), |a| a.iter().filter_map(Value::as_str).collect());
        if !fonts.is_empty() {
            let _ = writeln!(out, "\n## Fonts\n{} — load them (Google Fonts or @fontsource) before comparing.", fonts.join(", "));
        }
        if let Some(list) = v["components"].as_array().filter(|l| !l.is_empty()) {
            out.push_str("\n## Reused components — build each once\n");
            for c in list {
                let _ = writeln!(out, "- \"{}\" ×{} (main component id {})", c["name"].as_str().unwrap_or(""), c["uses"], c["mainId"].as_str().unwrap_or(""));
            }
        }
        if let Some(list) = v["annotations"].as_array().filter(|l| !l.is_empty()) {
            out.push_str("\n## Designer annotations — requirements to follow\n");
            for a in list {
                let _ = writeln!(out, "- \"{}\" (id {}): {}", a["layer"].as_str().unwrap_or(""), a["layerId"].as_str().unwrap_or(""), a["note"].as_str().unwrap_or(""));
            }
        }
        if let Some(list) = v["codeConnect"].as_array().filter(|l| !l.is_empty()) {
            out.push_str("\n## Codebase components (Code Connect) — the reference code already uses them; do not rebuild them\n");
            for c in list {
                let _ = writeln!(out, "- {} — `{}`{}", c["component"].as_str().unwrap_or(""), c["import"].as_str().unwrap_or(""), c["source"].as_str().filter(|s| !s.is_empty()).map(|s| format!(" ({s})")).unwrap_or_default());
            }
        }
        if let Some(list) = v["interactions"].as_array().filter(|l| !l.is_empty()) {
            out.push_str("\n## Interactions (prototype) — wire these up as navigation/handlers\n");
            for i in list {
                let _ = writeln!(
                    out,
                    "- \"{}\" (id {}): {} → {} ({}, {} ms)",
                    i["layer"].as_str().unwrap_or(""),
                    i["layerId"].as_str().unwrap_or(""),
                    i["trigger"]["type"].as_str().unwrap_or("on-click"),
                    i["action"].as_str().unwrap_or(""),
                    i["transition"].as_str().unwrap_or("instant"),
                    i["duration"]
                );
            }
        }
        let code = v["code"].as_str().unwrap_or_default();
        let _ = write!(out, "\n## Reference code\n```{lang}\n{code}```\n");
        let mut blocks = vec![ContentBlock::text(out)];
        if a.screenshot.unwrap_or(true)
            && let Ok(shot) = self.bridge.call("get_screenshot", json!({ "nodeId": node["id"], "maxSize": 1024 })).await
            && let Some(png) = shot["png"].as_str()
        {
            blocks.push(ContentBlock::image(png.to_string(), "image/png"));
            blocks.push(ContentBlock::text(format!("Screenshot of \"{name}\" as designed — the target to match.")));
        }
        let meta = v["metadata"].as_str().unwrap_or_default();
        let details = json!({ "node": v["node"], "texts": v["texts"], "tokens": v["tokens"] });
        let tail = format!("## Layer tree\n```xml\n{meta}```\n\n## Text strings and tokens\n```json\n{}\n```", pretty(&details));
        blocks.push(ContentBlock::text(tail));
        v["assets"] = Value::Null;
        Ok(CallToolResult::success(blocks))
    }

    #[tool(
        description = "Writes a layer's assets — icons, logos and illustrations as SVG, image fills as their original PNG/JPG/GIF/WebP — into a folder of the user's project, named as get_design_context references them. Returns the paths."
    )]
    async fn download_assets(&self, Parameters(a): Parameters<AssetsArgs>) -> Result<CallToolResult, ErrorData> {
        let dir = match self.bridge.access.check(Path::new(&a.dir), Use::Write).await {
            Ok(d) => d,
            Err(e) => return failure(e),
        };
        let v = match self.bridge.call("get_assets", json!({ "nodeId": a.node_id })).await {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let mut assets: Vec<Value> = v["assets"].as_array().cloned().unwrap_or_default();
        if assets.is_empty() {
            return text("This layer has no icons or images; nothing to download.");
        }
        let written = match Self::write_assets(&dir, &assets, a.overwrite.unwrap_or(false)) {
            Ok(w) => w,
            Err(e) => return failure(e),
        };
        self.cache_assets(&mut assets);
        text(format!(
            "Wrote {} file(s); reference them with the prefix {}:\n{}\n\nAssets:\n```json\n{}\n```",
            written.len(),
            url_for_dir(&dir),
            written.iter().map(|w| format!("- {w}")).collect::<Vec<_>>().join("\n"),
            pretty(&json!(assets))
        ))
    }

    #[tool(
        description = "Compares a screenshot of your implementation with the design and says how close it is: a match score, a diff image (mismatches in red), a side-by-side, and the design layers where the biggest differences are. Use after implementing a design; aim for 95%+."
    )]
    async fn compare_to_design(&self, Parameters(a): Parameters<CompareArgs>) -> Result<CallToolResult, ErrorData> {
        let image = match (&a.image_path, &a.image_base64) {
            (Some(p), _) => match self.bridge.access.check(Path::new(p), Use::Read).await {
                Ok(path) => match std::fs::read(&path) {
                    Ok(b) => B64.encode(b),
                    Err(e) => return failure(format!("Could not read {p}: {e}")),
                },
                Err(e) => return failure(e),
            },
            (None, Some(b)) => b.trim().trim_start_matches("data:image/png;base64,").to_string(),
            (None, None) => return failure("Pass image_path (a screenshot file) or image_base64".into()),
        };
        match self.bridge.call("compare_to_design", json!({ "nodeId": a.node_id, "image": image })).await {
            Ok(v) => {
                let mut blocks = vec![ContentBlock::text(v["report"].as_str().unwrap_or_default().to_string())];
                if let Some(png) = v["sideBySide"].as_str() {
                    blocks.push(ContentBlock::image(png.to_string(), "image/png"));
                    blocks.push(ContentBlock::text("Left: design. Middle: implementation. Right: differences in red."));
                }
                Ok(CallToolResult::success(blocks))
            }
            Err(e) => failure(e),
        }
    }

    #[tool(
        description = "Searches 200,000+ open-source icons (Iconify: lucide, mdi, tabler, heroicons, phosphor, logos for brands…). Returns icon ids to use in create_nodes as { \"type\": \"icon\", \"icon\": \"<id>\" }."
    )]
    async fn search_icons(&self, Parameters(a): Parameters<IconSearchArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("search_icons", json!({ "query": a.query, "prefixes": a.prefixes, "limit": a.limit })).await
    }

    #[tool(
        description = "Variables (design tokens) a layer uses — name → value in the active mode, e.g. {\"color/primary\": \"#7C3AED\"} — plus collections/modes and the raw colours, type styles, radii and spacing it uses with counts. page: true lists every variable."
    )]
    async fn get_variable_defs(&self, Parameters(a): Parameters<VarDefsArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_variable_defs", json!({ "nodeId": a.node_id, "page": a.page })).await
    }

    #[tool(description = "Imports a design shared as a figma.com link (a whole file, or one frame with ?node-id=) into a new Pergel file: layers, vectors, images, text styles, auto layout, components and variants, variables, prototype links. Uses the Figma access token saved in Pergel.")]
    async fn import_figma(&self, Parameters(a): Parameters<FigmaImportArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("import_figma", json!({ "url": a.url })).await
    }

    #[tool(description = "Team libraries available to every file (published components and component sets, with keys and sizes), which library components this file uses, and whether updates are waiting (like Figma's get_libraries).")]
    async fn get_libraries(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_libraries", json!({})).await
    }

    #[tool(description = "Places an instance of a team-library component in the open file (imported on first use; it stays linked to the library for updates). Prefer this over redrawing a component the library already has.")]
    async fn use_library_component(&self, Parameters(a): Parameters<LibraryUseArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("use_library_component", json!({ "library": a.library, "component": a.component, "x": a.x, "y": a.y })).await
    }

    #[tool(description = "Publishes the open file's components as a team library (again: a new version every user of it can update to).")]
    async fn publish_library(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("publish_library", json!({})).await
    }

    #[tool(description = "Comment threads on the shown page (open ones; includeResolved for all): who said what, where, and the layer each is pinned to. Use them as a to-do list of design feedback.")]
    async fn get_comments(&self, Parameters(a): Parameters<CommentsArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_comments", json!({ "includeResolved": a.include_resolved })).await
    }

    #[tool(description = "Adds a comment: a reply in a thread (threadId), or a new thread pinned to a layer (nodeId) or a canvas point (x, y). Signed \"Claude\".")]
    async fn add_comment(&self, Parameters(a): Parameters<AddCommentArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("add_comment", json!({ "text": a.text, "threadId": a.thread_id, "nodeId": a.node_id, "x": a.x, "y": a.y })).await
    }

    #[tool(description = "Marks a comment thread resolved (or reopens it with resolved: false).")]
    async fn resolve_comment(&self, Parameters(a): Parameters<ResolveArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("resolve_comment", json!({ "threadId": a.thread_id, "resolved": a.resolved })).await
    }

    #[tool(
        description = "Creates a new, empty design file and opens it in a new tab (like Figma's create_new_file). Use it to design something new without touching the file the user has open; the following tool calls work on the new file."
    )]
    async fn create_new_file(&self, Parameters(a): Parameters<NewFileArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("create_new_file", json!({ "name": a.name })).await
    }

    #[tool(
        description = "Runs JavaScript against the document (like Figma's use_figma) — for bulk or computed edits that would take many tool calls. Runs sandboxed in a worker; the whole run is one undo step, and a script that throws or times out changes nothing. API (all async): pergel.selection(), pergel.metadata(id?), pergel.node(id), pergel.create(nodeSpecs, parentId?) → created layers, pergel.update(id, props), pergel.remove(ids), pergel.select(ids), pergel.designContext(id?), pergel.searchIcons(query), pergel.variables(), pergel.notify(text), pergel.tool(name, args) for any other tool; console.log output is returned. Example: const s = await pergel.selection(); for (const n of s.selection) await pergel.update(n.id, { radius: 12 }); return s.selection.length;"
    )]
    async fn run_script(&self, Parameters(a): Parameters<ScriptArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("run_script", json!({ "code": a.code, "timeoutMs": a.timeout_ms })).await
    }

    #[tool(
        description = "Suggests Code Connect mappings: scans the codebase for UI components (React/Preact, Vue, Svelte — including class-variance-authority variants) and matches them to the file's main components and component sets by name, with variant → prop mappings. Review, then call add_code_connect_map with each suggestion you accept; get_design_context will then emit those components instead of raw markup."
    )]
    async fn get_code_connect_suggestions(&self, Parameters(a): Parameters<SuggestArgs>) -> Result<CallToolResult, ErrorData> {
        if !Path::new(&a.project_dir).is_dir() {
            return failure(format!("{} is not a folder", a.project_dir));
        }
        let root = match self.bridge.access.check(Path::new(&a.project_dir), Use::Scan).await {
            Ok(r) => r,
            Err(e) => return failure(e),
        };
        let root = root.as_path();
        let design = match self.bridge.call("get_components", json!({})).await {
            Ok(v) => v,
            Err(e) => return failure(e),
        };
        let code = crate::code_scan::scan(root);
        let suggestions = crate::code_scan::suggest(&design, &code);
        let mapped: usize = design["components"].as_array().map_or(0, |c| c.len()) + design["componentSets"].as_array().map_or(0, |c| c.len());
        text(format!(
            "Scanned {} code component(s); {} of {} design component(s)/set(s) have a match.\n\n```json\n{}\n```",
            code.len(),
            suggestions.len(),
            mapped,
            pretty(&json!({ "suggestions": suggestions }))
        ))
    }

    #[tool(
        description = "Draws a flowchart from Mermaid source as real, editable layers: shapes with labels laid out in ranks and connected by arrows (one undo step). Use it for user flows, architecture and process diagrams."
    )]
    async fn generate_diagram(&self, Parameters(a): Parameters<DiagramArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("generate_diagram", json!({ "mermaid": a.mermaid, "name": a.name, "x": a.x, "y": a.y })).await
    }

    #[tool(
        description = "Searches the file's design system — main components (on every page, with variants and Code Connect), styles and variables — by name. Use it before building a screen so you reuse the file's own components and tokens instead of drawing new ones."
    )]
    async fn search_design_system(&self, Parameters(a): Parameters<DesignSearchArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("search_design_system", json!({ "query": a.query, "types": a.types, "limit": a.limit })).await
    }

    #[tool(description = "Creates or updates variables (design tokens) in a collection, with per-mode values (Light/Dark…), and updates every layer bound to them. One call can define a whole token set.")]
    async fn set_variables(&self, Parameters(a): Parameters<SetVariablesArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("set_variables", json!({ "collection": a.collection, "variables": a.variables, "activeMode": a.active_mode })).await
    }

    #[tool(description = "Binds a layer property (fill, stroke, radius, gap, padding, fontSize…) to a variable so it follows the token and its modes; omit variable to unbind.")]
    async fn bind_variable(&self, Parameters(a): Parameters<BindArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("bind_variable", json!({ "nodeIds": a.node_ids, "property": a.property, "variable": a.variable })).await
    }

    #[tool(description = "The file's pages (name, index, which one is shown). Other tools work on the shown page.")]
    async fn get_pages(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_pages", json!({})).await
    }

    #[tool(description = "Shows another page of the file (by name or index); later tool calls work on it.")]
    async fn switch_page(&self, Parameters(a): Parameters<PageSwitchArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("switch_page", json!({ "name": a.name, "index": a.index })).await
    }

    #[tool(description = "The layers currently selected in Pergel (id, name, type, position, size).")]
    async fn get_selection(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_selection", json!({})).await
    }

    #[tool(description = "Prototype: sets a layer's interactions (on click/hover/after delay → navigate to a screen, go back, open a link, or change an instance to another variant — interactive components: put \"hover → change-to State=Hover\" on the Default variant) with transitions. The user can play the prototype in Pergel.")]
    async fn set_interactions(&self, Parameters(a): Parameters<InteractionArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("set_interactions", json!({ "nodeId": a.node_id, "interactions": a.interactions })).await
    }

    #[tool(description = "Code Connect: maps a Pergel main component / component set to a component in the user's codebase. From then on get_design_context writes <Component …props>text</Component> (with its import) for every instance instead of rebuilding it. Map the design system once, before implementing screens.")]
    async fn add_code_connect_map(&self, Parameters(a): Parameters<CodeConnectArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool(
            "add_code_connect_map",
            json!({ "nodeId": a.node_id, "component": a.component, "import": a.import.unwrap_or_default(), "source": a.source.unwrap_or_default(), "props": a.props.unwrap_or(json!({})), "text": a.text.unwrap_or(json!({})) }),
        )
        .await
    }

    #[tool(description = "Code Connect mappings: which Pergel components map to which codebase components (with imports and prop mappings).")]
    async fn get_code_connect_map(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_code_connect_map", json!({})).await
    }

    #[tool(description = "Switches instances to another variant of their component set (keeps the text typed into them).")]
    async fn set_variant(&self, Parameters(a): Parameters<VariantArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("set_variant", json!({ "nodeIds": a.node_ids, "properties": a.properties })).await
    }

    #[tool(description = "Main components in the document (with instance counts and, for variants, their set and property values) and the component sets with their properties.")]
    async fn get_components(&self) -> Result<CallToolResult, ErrorData> {
        self.json_tool("get_components", json!({})).await
    }

    #[tool(description = "Creates layers from node specs (one undo step) and selects them. Returns the new ids. Use real icons (type icon), SVG files and images instead of approximating artwork with shapes.\n\nNODE_SPEC")]
    async fn create_nodes(&self, Parameters(a): Parameters<CreateArgs>) -> Result<CallToolResult, ErrorData> {
        let mut nodes = a.nodes;
        let mut files = Vec::new();
        for n in &nodes {
            Self::local_files(n, &mut files);
        }
        let mut allowed = HashMap::new();
        for f in files {
            match self.bridge.access.check(Path::new(&f), Use::Read).await {
                Ok(p) => allowed.insert(f, p),
                Err(e) => return failure(e),
            };
        }
        for n in &mut nodes {
            if let Err(e) = Self::resolve_files(n, &allowed) {
                return failure(e);
            }
        }
        self.json_tool("create_nodes", json!({ "parentId": a.parent_id, "nodes": nodes })).await
    }

    #[tool(description = "Changes properties of one layer (one undo step). Layers inside instances can only be restyled, not moved or resized.")]
    async fn update_node(&self, Parameters(a): Parameters<UpdateArgs>) -> Result<CallToolResult, ErrorData> {
        let mut args = Value::Object(a.properties);
        args["nodeId"] = json!(a.node_id);
        self.json_tool("update_node", args).await
    }

    #[tool(description = "Deletes layers (one undo step). Layers inside instances can't be deleted.")]
    async fn delete_nodes(&self, Parameters(a): Parameters<IdsArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("delete_nodes", json!({ "nodeIds": a.node_ids })).await
    }

    #[tool(description = "Selects layers in Pergel and (by default) zooms to them, to show the user something.")]
    async fn select_nodes(&self, Parameters(a): Parameters<IdsArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool("select_nodes", json!({ "nodeIds": a.node_ids, "zoom": a.zoom })).await
    }

    #[tool(description = "Places an instance of a main component (see get_components).")]
    async fn create_instance(&self, Parameters(a): Parameters<InstanceArgs>) -> Result<CallToolResult, ErrorData> {
        self.json_tool(
            "create_instance",
            json!({ "componentId": a.component_id, "parentId": a.parent_id, "x": a.x, "y": a.y }),
        )
        .await
    }
}

#[tool_handler(router = self.tool_router)]
#[prompt_handler(router = self.prompt_router)]
impl ServerHandler for PergelMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().enable_prompts().build())
            .with_server_info(Implementation::new("pergel", env!("CARGO_PKG_VERSION")))
            .with_instructions(format!(
                "Pergel is a Figma-like design tool; these tools read and edit the document open in it. \
                 Layer ids are strings (e.g. \"12\"); get_metadata lists them. Tools that take an optional \
                 nodeId use the user's current selection when it is omitted. Positions are relative to the \
                 parent frame.\n\n\
                 Implementing a design as code: call get_design_context (pass assets_dir to drop the icons and \
                 images into the project), follow its rules — the design's icons and images are final files, \
                 never redraw or swap them — then screenshot your result and call compare_to_design.\n\n\
                 Designing in Pergel: use real icons (search_icons, then type \"icon\"), SVGs and images rather than \
                 shapes; check the result with get_screenshot.\n\n{NODE_SPEC}"
            ))
    }
}

/// Whether a request's Host header names this machine. Web pages on other origins can't read
/// the assets (no CORS header), and a page that rebinds its own domain to 127.0.0.1 still sends
/// its domain as the Host, so it is turned away too.
fn local_host(headers: &axum::http::HeaderMap) -> bool {
    let Some(host) = headers.get(axum::http::header::HOST).and_then(|h| h.to_str().ok()) else { return false };
    let name = match host.strip_prefix('[') {
        Some(rest) => rest.split(']').next().unwrap_or(""),
        None => host.rsplit_once(':').map_or(host, |(name, _)| name),
    };
    matches!(name.to_ascii_lowercase().as_str(), "localhost" | "127.0.0.1" | "::1")
}

async fn serve_asset(State(bridge): State<Arc<Bridge>>, AxumPath(file): AxumPath<String>, headers: axum::http::HeaderMap) -> axum::response::Response {
    use axum::http::{StatusCode, header};
    use axum::response::IntoResponse;
    if !local_host(&headers) {
        return (StatusCode::FORBIDDEN, "Assets are only served to this machine").into_response();
    }
    let found = bridge.assets.lock().ok().and_then(|c| c.get(&file).cloned());
    match found {
        Some((mime, bytes)) => ([(header::CONTENT_TYPE, mime), (header::CACHE_CONTROL, "no-cache".to_string())], bytes).into_response(),
        None => (StatusCode::NOT_FOUND, "Unknown asset; call get_design_context or download_assets again").into_response(),
    }
}

/// Starts the server on 127.0.0.1:PORT. Returns the status to show in the UI.
pub async fn serve(bridge: Arc<Bridge>) -> Status {
    let url = format!("http://127.0.0.1:{PORT}/mcp");
    let config = StreamableHttpServerConfig::default();
    let for_mcp = bridge.clone();
    let service: StreamableHttpService<PergelMcp, LocalSessionManager> =
        StreamableHttpService::new(move || Ok(PergelMcp::new(for_mcp.clone())), Default::default(), config);
    let router = axum::Router::new()
        .route("/assets/{*file}", axum::routing::get(serve_asset))
        .with_state(bridge)
        .nest_service("/mcp", service);
    match tokio::net::TcpListener::bind(("127.0.0.1", PORT)).await {
        Ok(listener) => {
            tokio::spawn(async move {
                let _ = axum::serve(listener, router).await;
            });
            Status { url, running: true, error: None }
        }
        Err(e) => Status { url, running: false, error: Some(format!("Port {PORT} is busy: {e}")) },
    }
}

#[cfg(test)]
mod tests {
    use super::local_host;
    use axum::http::{HeaderMap, HeaderValue, header::HOST};

    fn host(h: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(HOST, HeaderValue::from_str(h).unwrap());
        m
    }

    #[test]
    fn assets_are_served_to_this_machine_only() {
        for ok in ["127.0.0.1:3855", "localhost:3855", "LOCALHOST", "[::1]:3855"] {
            assert!(local_host(&host(ok)), "{ok}");
        }
        // A page that rebinds its own domain to 127.0.0.1 still sends its domain.
        for bad in ["evil.example:3855", "127.0.0.1.evil.example", "[::2]:3855"] {
            assert!(!local_host(&host(bad)), "{bad}");
        }
        assert!(!local_host(&HeaderMap::new()));
    }
}
