use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{CStr, CString, c_char};

use pergel_core::{
    Align, AutoLayout, Blend, Color, Component, Document, Effect, ExportSetting, Flow, Framework, ImageFill, ImageMode, NodeSpec, Paint, parse_color, parse_line_height, type_name, Justify, Kind, Sizing, Node, NodeId, Order, Rect, Scene, StrokeAlign, Text, TextAlign, TextResize, Tree, Viewport,
    bounds_of, from_text, import_svg, normalize_degrees, rotate_point, to_text,
};
use base64::Engine as _;
use pergel_render::{Overlay, Renderer};
use serde::{Deserialize, Serialize};
use skia_safe::gpu::gl::FramebufferInfo;
use skia_safe::gpu::{self, DirectContext, SurfaceOrigin};
use skia_safe::{ColorType, Surface};

const CANVAS_SELECTOR: &CStr = c"#pergel-canvas";
const GL_RGBA8: u32 = 0x8058;
const FIT_MARGIN: f32 = 64.0;
const DEFAULT_FILL: Color = Color::rgba(217, 217, 217, 255);
const WHITE: Color = Color::rgba(255, 255, 255, 255);
const BLACK: Color = Color::rgba(0, 0, 0, 255);

unsafe extern "C" {
    fn pergel_gl_create(selector: *const c_char) -> i32;
}

struct Engine {
    context: DirectContext,
    surface: Surface,
    scene: Scene,
    renderer: Renderer,
    view: Viewport,
    dpr: f32,
    selection: Vec<NodeId>,
    hover: Option<NodeId>,
    marquee: Option<Rect>,
    editing: Option<NodeId>,
    /// New text layers start with the last style the user picked, as in Figma.
    text_style: Text,
    drag: Option<TransformDrag>,
    /// The tab (document slot) shown now, and the other open documents.
    slot: u32,
    parked: HashMap<u32, Parked>,
    /// The document's pages; the shown page's scene lives in `scene` (its slot holds an empty one).
    pages: Vec<PageSlot>,
    page: usize,
    /// Bumped by page edits, which don't change any scene (for autosave).
    page_rev: u64,
    /// The document's variables (shared by its pages).
    variables: pergel_core::Variables,
    /// The document's text, colour and effect styles.
    styles: pergel_core::Styles,
    /// Vector edit mode or a pen drawing in progress.
    vedit: Option<VectorEdit>,
    /// Draw prototype connections (Prototype tab).
    show_connections: bool,
    /// Presentation view: instances shown as another variant (instance → variant main).
    present_swaps: Vec<(NodeId, NodeId)>,
    /// Comment threads on empty canvas, with their page.
    comments: Vec<pergel_core::PageComment>,
    /// Counter for live-collaboration uids.
    uid_seed: u64,
    /// Set when the document is a branch of another file.
    branch: Option<pergel_core::Branch>,
    /// Presentation view: variables changed by "Set variable" (id → value).
    present_vars: Vec<(String, pergel_core::VarValue)>,
    /// A drag of the selection in progress (for snapping).
    moving: Option<MoveSession>,
    guides: Vec<((f32, f32), (f32, f32))>,
    /// Layout guides shown (Shift+G).
    show_grids: bool,
    /// The shown page's ruler guides.
    page_guides: Vec<pergel_core::Guide>,
}

/// Layers being dragged: the box at the start and the offset applied so far.
struct MoveSession {
    ids: Vec<NodeId>,
    start: Rect,
    applied: (f32, f32),
    /// Edges and centres of the layers to snap to: (x lines, y lines), each with its layer box.
    xs: Vec<(f32, Rect)>,
    ys: Vec<(f32, Rect)>,
}

/// Vector edit mode: the layer's outline as anchors in world coordinates (unrotated frame).
struct VectorEdit {
    node: NodeId,
    paths: Vec<pergel_core::SubPath>,
    selected: Vec<(usize, usize)>,
    drag: Option<VDrag>,
    /// Drawing with the pen: clicks add anchors to the last sub-path.
    pen: bool,
    /// Drawing with the pencil: the raw points of the stroke so far.
    pencil: Option<Vec<(f32, f32)>>,
    rubber: Option<(f32, f32)>,
}

enum VDrag {
    Anchors { from: (f32, f32), orig: Vec<pergel_core::SubPath> },
    Handle { path: usize, anchor: usize, out: bool, mirror: bool },
    /// The pen's new anchor: dragging pulls out symmetric handles.
    PenHandle { path: usize, anchor: usize },
}

/// One page of the open document.
struct PageSlot {
    name: String,
    scene: Scene,
    view: Viewport,
    selection: Vec<NodeId>,
    guides: Vec<pergel_core::Guide>,
}

impl PageSlot {
    fn new(name: impl Into<String>, view: Viewport) -> Self {
        Self { name: name.into(), scene: Scene::default(), view, selection: Vec::new(), guides: Vec::new() }
    }
}

/// An open document that is not on screen: its layers, undo history, view and selection.
struct Parked {
    scene: Scene,
    view: Viewport,
    selection: Vec<NodeId>,
    pages: Vec<PageSlot>,
    page: usize,
    variables: pergel_core::Variables,
    styles: pergel_core::Styles,
}

/// A resize or rotate drag from a selection handle, computed from the state at its start.
struct TransformDrag {
    handle: u32,
    rect: Rect,
    rotation: f32,
    nodes: Vec<(NodeId, Rect, f32)>,
    /// Offset from the pointer to the dragged handle, in the box's own frame.
    grab: (f32, f32),
    start_angle: f32,
    applied: f32,
    /// Scale tool: the box after the last step (contents scale along with it).
    scaled: Option<Rect>,
}

const HANDLE_HIT_PX: f32 = 6.0;
const EDGE_HIT_PX: f32 = 4.0;
const ROTATE_HIT_PX: f32 = 18.0;

/// Handle numbering: 0 top-left, 1 top, 2 top-right, 3 right, 4 bottom-right, 5 bottom,
/// 6 bottom-left, 7 left; 8–11 rotate zones outside the corners (top-left, top-right,
/// bottom-right, bottom-left).
fn handle_point(r: &Rect, handle: u32) -> (f32, f32) {
    let (cx, cy) = r.center();
    match handle {
        0 => (r.x, r.y),
        1 => (cx, r.y),
        2 => (r.right(), r.y),
        3 => (r.right(), cy),
        4 => (r.right(), r.bottom()),
        5 => (cx, r.bottom()),
        6 => (r.x, r.bottom()),
        _ => (r.x, cy),
    }
}

fn moves_left(h: u32) -> bool {
    matches!(h, 0 | 6 | 7)
}
fn moves_right(h: u32) -> bool {
    matches!(h, 2 | 3 | 4)
}
fn moves_top(h: u32) -> bool {
    matches!(h, 0 | 1 | 2)
}
fn moves_bottom(h: u32) -> bool {
    matches!(h, 4 | 5 | 6)
}

/// Screen-space angle of a point about a centre, in Figma's counter-clockwise degrees.
fn angle(cx: f32, cy: f32, x: f32, y: f32) -> f32 {
    -(y - cy).atan2(x - cx).to_degrees()
}

/// An effect as the UI edits it: hex colour plus opacity in percent.
#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EffectInfo {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    opacity: f32,
    #[serde(default)]
    x: f32,
    #[serde(default)]
    y: f32,
    #[serde(default)]
    blur: f32,
    #[serde(default)]
    spread: f32,
    #[serde(default = "yes")]
    visible: bool,
}

fn yes() -> bool {
    true
}

impl From<&Effect> for EffectInfo {
    fn from(e: &Effect) -> Self {
        let shadow = |kind: &str, color: Color, x, y, blur, spread, visible| EffectInfo {
            kind: kind.into(),
            color: hex(color),
            opacity: alpha(color),
            x,
            y,
            blur,
            spread,
            visible,
        };
        match *e {
            Effect::DropShadow { color, x, y, blur, spread, visible } => shadow("drop-shadow", color, x, y, blur, spread, visible),
            Effect::InnerShadow { color, x, y, blur, spread, visible } => shadow("inner-shadow", color, x, y, blur, spread, visible),
            Effect::LayerBlur { radius, visible } => EffectInfo { kind: "layer-blur".into(), color: String::new(), opacity: 0.0, x: 0.0, y: 0.0, blur: radius, spread: 0.0, visible },
            Effect::BackgroundBlur { radius, visible } => EffectInfo { kind: "background-blur".into(), color: String::new(), opacity: 0.0, x: 0.0, y: 0.0, blur: radius, spread: 0.0, visible },
        }
    }
}

impl EffectInfo {
    fn to_effect(&self) -> Effect {
        let color = Color(parse_hex(&self.color).unwrap_or(0));
        let color = with_alpha_pct(color, self.opacity);
        let blur = self.blur.max(0.0);
        match self.kind.as_str() {
            "inner-shadow" => Effect::InnerShadow { color, x: self.x, y: self.y, blur, spread: self.spread, visible: self.visible },
            "layer-blur" => Effect::LayerBlur { radius: blur, visible: self.visible },
            "background-blur" => Effect::BackgroundBlur { radius: blur, visible: self.visible },
            _ => Effect::DropShadow { color, x: self.x, y: self.y, blur, spread: self.spread, visible: self.visible },
        }
    }
}

#[derive(Deserialize, Default, Clone)]
#[serde(default)]
struct FillPatch {
    color: Option<String>,
    /// 0–100.
    opacity: Option<f32>,
    paint: Option<serde_json::Value>,
    visible: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
struct Patch {
    x: Option<f32>,
    y: Option<f32>,
    w: Option<f32>,
    h: Option<f32>,
    rotation: Option<f32>,
    radius: Option<f32>,
    /// Independent corners; an empty array switches back to the uniform radius.
    corners: Option<Vec<f32>>,
    fill: Option<String>,
    fill_opacity: Option<f32>,
    fill_on: Option<bool>,
    stroke_on: Option<bool>,
    stroke_color: Option<String>,
    stroke_opacity: Option<f32>,
    stroke_width: Option<f32>,
    stroke_align: Option<StrokeAlign>,
    opacity: Option<f32>,
    blend: Option<Blend>,
    lock_ratio: Option<bool>,
    flow: Option<Flow>,
    gap: Option<f32>,
    counter_gap: Option<f32>,
    /// Top, right, bottom, left.
    padding: Option<Vec<f32>>,
    justify: Option<Justify>,
    align: Option<Align>,
    columns: Option<u16>,
    rows: Option<u16>,
    sizing_h: Option<Sizing>,
    sizing_v: Option<Sizing>,
    absolute: Option<bool>,
    effects: Option<Vec<EffectInfo>>,
    exports: Option<Vec<ExportSetting>>,
    /// A gradient or image fill (`null` goes back to the solid colour).
    paint: Option<serde_json::Value>,
    constraint_h: Option<pergel_core::Constraint>,
    constraint_v: Option<pergel_core::Constraint>,
    grids: Option<Vec<pergel_core::LayoutGrid>>,
    /// Dash and gap; an empty list makes the stroke solid.
    dash: Option<Vec<f32>>,
    /// Annotation; an empty string removes it.
    note: Option<String>,
    ready: Option<bool>,
    /// Prototype overflow scrolling.
    scroll: Option<pergel_core::Scroll>,
    /// Fills above the layer's own fill, bottom to top (replaces the list).
    extra_fills: Option<Vec<FillPatch>>,
    /// Main component / set: replaces its property definitions.
    component_props: Option<Vec<pergel_core::ComponentProp>>,
    /// Layer in a component: the properties it shows.
    prop_refs: Option<pergel_core::PropRefs>,
    /// Instance: values by property id (null resets to the default).
    prop_values: Option<serde_json::Map<String, serde_json::Value>>,
    /// Individual stroke weights [top, right, bottom, left]; empty turns them off.
    stroke_sides: Option<Vec<f32>>,
    /// Vector start / end points.
    start_point: Option<pergel_core::EndPoint>,
    end_point: Option<pergel_core::EndPoint>,
}

fn parse_hex(hex: &str) -> Option<u32> {
    let h = hex.trim().trim_start_matches('#');
    let h = if h.len() == 3 { h.chars().flat_map(|c| [c, c]).collect::<String>() } else { h.to_string() };
    (h.len() == 6).then(|| u32::from_str_radix(&h, 16).ok()).flatten()
}

fn with_rgb(c: Color, rgb: u32) -> Color {
    Color((c.argb() & 0xff00_0000) | (rgb & 0x00ff_ffff))
}

fn with_alpha_pct(c: Color, pct: f32) -> Color {
    let a = ((pct.clamp(0.0, 100.0) / 100.0) * 255.0).round() as u32;
    Color((a << 24) | (c.argb() & 0x00ff_ffff))
}

thread_local! {
    static ENGINE: RefCell<Option<Engine>> = const { RefCell::new(None) };
    static OUT: RefCell<CString> = RefCell::new(CString::default());
    static TEXT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn make_surface(context: &mut DirectContext, width: i32, height: i32) -> Option<Surface> {
    let info = FramebufferInfo { fboid: 0, format: GL_RGBA8, ..Default::default() };
    let target = gpu::backend_render_targets::make_gl((width.max(1), height.max(1)), 0, 8, info);
    gpu::surfaces::wrap_backend_render_target(
        context,
        &target,
        SurfaceOrigin::BottomLeft,
        ColorType::RGBA8888,
        None,
        None,
    )
}

fn with_engine<T>(f: impl FnOnce(&mut Engine) -> T) -> Option<T> {
    ENGINE.with_borrow_mut(|engine| engine.as_mut().map(f))
}

fn emit(json: Option<String>) -> *const c_char {
    OUT.with_borrow_mut(|out| {
        *out = CString::new(json.unwrap_or_else(|| "null".into())).unwrap_or_default();
        out.as_ptr()
    })
}

fn text() -> String {
    TEXT.with_borrow(|buf| String::from_utf8_lossy(buf).into_owned())
}

fn text_bytes() -> Vec<u8> {
    TEXT.with_borrow(|buf| buf.clone())
}

fn hex(c: Color) -> String {
    format!("#{:06X}", c.argb() & 0x00ff_ffff)
}

fn alpha(c: Color) -> f32 {
    ((c.argb() >> 24) as f32 / 255.0 * 100.0).round()
}

fn screen_to_world(view: &Viewport, sx: f32, sy: f32, sw: f32, sh: f32) -> Rect {
    let (x0, y0) = view.to_world(sx.min(sx + sw), sy.min(sy + sh));
    let (x1, y1) = view.to_world(sx.max(sx + sw), sy.max(sy + sh));
    Rect::new(x0, y0, x1 - x0, y1 - y0)
}

impl Engine {
    fn reset_scene(&mut self, scene: Scene) {
        self.scene = scene;
        self.pages = vec![PageSlot::new("Page 1", self.view)];
        self.page = 0;
        self.selection.clear();
        self.hover = None;
        self.editing = None;
        self.renderer.text.retain(|_| false);
        self.fit_all_text();
        self.view.fit(self.scene.bounds(), FIT_MARGIN);
    }

    /// Resizes an auto-sized text layer to its laid-out text, keeping the side it is aligned to.
    fn fit_text(&mut self, id: NodeId) {
        let Some(node) = self.scene.node(id) else { return };
        let Some(text) = node.text.as_deref() else { return };
        let b = node.bounds;
        let Some((w, h)) = self.renderer.text.fit(text, b.w) else { return };
        let x = match (text.resize, text.align) {
            (TextResize::AutoWidth, TextAlign::Center) if b.w > 0.0 => b.x + (b.w - w) / 2.0,
            (TextResize::AutoWidth, TextAlign::Right) if b.w > 0.0 => b.x + b.w - w,
            _ => b.x,
        };
        let r = Rect::new(x, b.y, w, h);
        if r != b {
            self.scene.set_bounds(id, r);
        }
    }

    fn fit_all_text(&mut self) {
        let ids: Vec<NodeId> = self.scene.nodes().filter(|n| n.kind == Kind::Text).map(|n| n.id).collect();
        for id in ids {
            self.fit_text(id);
        }
        self.relayout();
    }

    /// Puts the current document aside (it keeps its undo history).
    fn park(&mut self) {
        let parked = Parked {
            scene: std::mem::take(&mut self.scene),
            view: self.view,
            selection: std::mem::take(&mut self.selection),
            pages: std::mem::take(&mut self.pages),
            page: self.page,
            variables: std::mem::take(&mut self.variables),
            styles: std::mem::take(&mut self.styles),
        };
        self.parked.insert(self.slot, parked);
        self.hover = None;
        self.marquee = None;
        self.editing = None;
        self.drag = None;
        // Cached text layouts are keyed by layer id, which documents share.
        self.renderer.text.retain(|_| false);
    }

    /// Shows the document in `slot`; a new empty one when the slot is unknown.
    fn show(&mut self, slot: u32) {
        let (w, h) = (self.view.width, self.view.height);
        match self.parked.remove(&slot) {
            Some(p) => {
                self.scene = p.scene;
                self.view = p.view;
                self.selection = p.selection;
                self.pages = p.pages;
                self.page = p.page;
                self.variables = p.variables;
                self.styles = p.styles;
            }
            None => {
                self.scene = Scene::default();
                self.view = Viewport::new(w, h);
                self.view.x = -w / 2.0;
                self.view.y = -h / 2.0;
                self.pages = vec![PageSlot::new("Page 1", self.view)];
                self.page = 0;
                self.variables = pergel_core::Variables::default();
                self.styles = pergel_core::Styles::default();
            }
        }
        self.view.resize(w, h);
        self.slot = slot;
    }

    fn document_json(&self) -> String {
        let view_json = |v: &Viewport| serde_json::json!({ "x": v.x, "y": v.y, "zoom": v.zoom });
        let pages = self
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut page = if i == self.page { self.scene.to_page(&p.name, view_json(&self.view)) } else { p.scene.to_page(&p.name, view_json(&p.view)) };
                page.guides = if i == self.page { self.page_guides.clone() } else { p.guides.clone() };
                page
            })
            .collect();
        let mut doc = Document::new(pages);
        doc.variables = self.variables.clone();
        doc.styles = self.styles.clone();
        doc.comments = self.comments.clone();
        doc.branch = self.branch.clone();
        // Every page's images live in the shown page's store (see `switch_page`).
        doc.embed_images(&self.scene.images);
        serde_json::to_string(&doc).unwrap_or_default()
    }

    /// Pushes variable values into bound layers on every page (one undo step on the shown one).
    fn apply_variables(&mut self) -> usize {
        let vars = self.variables.clone();
        let mut n = 0;
        for (i, p) in self.pages.iter_mut().enumerate() {
            if i != self.page {
                n += p.scene.apply_variables(&vars).len();
            }
        }
        n + self.step(|e| e.scene.apply_variables(&vars).len())
    }

    /// Restyles linked layers on every page (one undo step on the shown one).
    fn apply_styles(&mut self) -> usize {
        let styles = self.styles.clone();
        let mut n = 0;
        for (i, p) in self.pages.iter_mut().enumerate() {
            if i != self.page {
                n += p.scene.apply_styles(&styles).len();
            }
        }
        n + self.step(|e| {
            let changed = e.scene.apply_styles(&styles);
            for id in &changed {
                e.fit_text(*id);
            }
            changed.len()
        })
    }

    /// Resolves `name:<variable name>` bindings (from agent specs) to variable ids.
    fn link_named_bindings(&mut self, ids: &[NodeId]) {
        let all: Vec<NodeId> = ids.iter().flat_map(|id| self.scene.subtree(*id)).collect();
        for id in all {
            let Some(b) = self.scene.node(id).and_then(|n| n.bindings.as_deref()).cloned() else { continue };
            if !b.values().any(|v| v.starts_with("name:")) {
                continue;
            }
            let fixed: pergel_core::Bindings = b
                .into_iter()
                .filter_map(|(k, v)| match v.strip_prefix("name:") {
                    Some(name) => self.variables.by_name(name).map(|var| (k, var.id.clone())),
                    None => Some((k, v)),
                })
                .collect();
            self.scene.update(id, |n| n.bindings = (!fixed.is_empty()).then(|| Box::new(fixed)));
        }
    }

    /// Shows page `i`. The image store moves along, so it always holds every page's images.
    fn switch_page(&mut self, i: usize) -> bool {
        if i >= self.pages.len() || i == self.page {
            return false;
        }
        let images = std::mem::take(&mut self.scene.images);
        let cur = self.page;
        self.pages[cur].scene = std::mem::take(&mut self.scene);
        self.pages[cur].view = self.view;
        self.pages[cur].selection = std::mem::take(&mut self.selection);
        self.pages[cur].guides = std::mem::take(&mut self.page_guides);
        self.page_guides = std::mem::take(&mut self.pages[i].guides);
        let (w, h) = (self.view.width, self.view.height);
        self.scene = std::mem::take(&mut self.pages[i].scene);
        for (hash, bytes) in images.iter() {
            self.scene.images.insert(hash.clone(), bytes.clone());
        }
        self.view = self.pages[i].view;
        self.view.resize(w, h);
        self.selection = std::mem::take(&mut self.pages[i].selection);
        self.page = i;
        self.hover = None;
        self.marquee = None;
        self.editing = None;
        self.drag = None;
        self.renderer.text.retain(|_| false);
        self.fit_all_text();
        true
    }

    fn page_name(&self, base: &str) -> String {
        let n = (1..).find(|k| !self.pages.iter().any(|p| p.name == format!("{base} {k}"))).unwrap_or(1);
        format!("{base} {n}")
    }

    /// Re-runs auto layout (cheap when the document has none).
    fn relayout(&mut self) {
        self.scene.relayout(&self.renderer.text);
    }

    /// Before an undo step closes: lay out, push main-component edits into instances (recording
    /// overrides), and lay out again for instances that changed.
    fn settle(&mut self) {
        self.relayout();
        let touched = self.scene.touched();
        if !touched.is_empty() {
            self.scene.sync_components(&touched);
            self.relayout();
        }
    }

    /// Selected layers that may change structure (not inside an instance).
    fn editable(&self) -> Vec<NodeId> {
        self.scene.outermost(&self.selection).into_iter().filter(|id| !self.scene.in_instance(*id)).collect()
    }

    /// Figma's resizing per axis (0 horizontal, 1 vertical); for text it maps onto the
    /// auto width / auto height / fixed resize modes.
    fn set_sizing(&mut self, id: NodeId, axis: usize, sizing: Sizing) {
        self.scene.update(id, |n| {
            n.sizing[axis] = sizing;
            if let Some(t) = n.text.as_mut() {
                t.resize = match (axis, sizing, t.resize) {
                    (0, Sizing::Hug, _) => TextResize::AutoWidth,
                    (0, _, TextResize::AutoWidth) => TextResize::AutoHeight,
                    (1, Sizing::Hug, TextResize::Fixed) => TextResize::AutoHeight,
                    (1, Sizing::Fixed | Sizing::Fill, _) => TextResize::Fixed,
                    (_, _, r) => r,
                };
            }
        });
        self.fit_text(id);
    }

    /// Where a layer dropped at (x, y) goes among an auto-layout frame's children: an index
    /// into the full child list as it is before the move.
    fn flow_index(&self, frame: NodeId, x: f32, y: f32, moving: &[NodeId]) -> Option<usize> {
        let n = self.scene.node(frame)?;
        let flow = n.layout.as_ref()?.flow;
        let kids: Vec<NodeId> =
            n.children.iter().copied().filter(|c| !moving.contains(c) && self.scene.in_auto_layout(*c)).collect();
        let before = kids
            .iter()
            .filter(|c| {
                let b = self.scene.node(**c).map(|n| n.aabb()).unwrap_or_default();
                let (cx, cy) = b.center();
                match flow {
                    Flow::Horizontal => cx < x,
                    Flow::Vertical => cy < y,
                    Flow::Wrap | Flow::Grid => b.bottom() < y || (y >= b.y && cx < x),
                }
            })
            .count();
        Some(kids.get(before).and_then(|c| n.children.iter().position(|k| k == c)).unwrap_or(n.children.len()))
    }

    fn apply_patch(&mut self, id: NodeId, p: &Patch) {
        let Some(node) = self.scene.node(id).cloned() else { return };
        if let Some(list) = &p.extra_fills {
            let layers: Vec<pergel_core::FillLayer> = list
                .iter()
                .map(|f| {
                    let rgb = f.color.as_deref().and_then(parse_hex).unwrap_or(0);
                    let a = (f.opacity.unwrap_or(100.0).clamp(0.0, 100.0) / 100.0 * 255.0).round() as u32;
                    pergel_core::FillLayer {
                        color: Color((a << 24) | (rgb & 0x00ff_ffff)),
                        paint: f.paint.clone().filter(|v| !v.is_null()).and_then(|v| serde_json::from_value(v).ok()),
                        visible: f.visible.unwrap_or(true),
                    }
                })
                .collect();
            self.scene.update(id, |n| n.extra_fills = layers);
        }
        if let Some(defs) = &p.component_props {
            let owner = if node.variant_set { id } else { self.scene.props_owner(id) };
            let mut defs = defs.clone();
            for i in 0..defs.len() {
                if defs[i].id.is_empty() {
                    defs[i].id = pergel_core::new_prop_id(&defs);
                }
            }
            self.scene.update(owner, |n| n.props = defs.clone());
            // Instances of every variant follow the new definitions.
            let mains: Vec<NodeId> = if self.scene.node(owner).is_some_and(|n| n.variant_set) {
                self.scene.node(owner).map(|n| n.children.clone()).unwrap_or_default()
            } else {
                vec![owner]
            };
            for m in mains {
                for inst in self.scene.instances_of(m) {
                    self.scene.sync_instance(inst);
                }
            }
        }
        if let Some(r) = &p.prop_refs {
            self.scene.update(id, |n| n.prop_refs = (!r.is_empty()).then(|| Box::new(r.clone())));
            if let Some(c) = self.scene.component_of(id) {
                let mains: Vec<NodeId> = if self.scene.node(c).is_some_and(|n| n.variant_set) { self.scene.node(c).map(|n| n.children.clone()).unwrap_or_default() } else { vec![c] };
                for m in mains {
                    for inst in self.scene.instances_of(m) {
                        self.scene.sync_instance(inst);
                    }
                }
            }
        }
        if let Some(values) = &p.prop_values {
            let (set, reset): (Vec<_>, Vec<_>) = values.iter().partition(|(_, v)| !v.is_null());
            if !reset.is_empty() {
                self.scene.update(id, |n| {
                    for (k, _) in &reset {
                        n.prop_values.remove(*k);
                    }
                });
                self.scene.sync_instance(id);
            }
            let set: std::collections::BTreeMap<String, serde_json::Value> = set.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            if !set.is_empty() {
                self.scene.set_prop_values_by_name(id, &set);
            }
        }
        if let Some(v) = &p.paint {
            let paint: Option<Paint> = if v.is_null() { None } else { serde_json::from_value(v.clone()).ok() };
            if v.is_null() || paint.is_some() {
                self.scene.update(id, |n| {
                    if let Some(Paint::Gradient(g)) = &paint
                        && let Some(first) = g.stops.first()
                    {
                        n.fill = first.color;
                    }
                    n.paint = paint.map(Box::new);
                    n.fill_on = true;
                });
            }
        }
        if p.x.is_some() || p.y.is_some() || p.w.is_some() || p.h.is_some() {
            let old = node.bounds;
            let lock = p.lock_ratio.unwrap_or(node.lock_ratio) && old.w > 0.0 && old.h > 0.0;
            let mut w = p.w.unwrap_or(old.w).max(0.0);
            let mut h = p.h.unwrap_or(old.h).max(0.0);
            if lock && p.w.is_some() && p.h.is_none() {
                h = w * old.h / old.w;
            } else if lock && p.h.is_some() && p.w.is_none() {
                w = h * old.w / old.h;
            }
            let r = Rect::new(p.x.unwrap_or(old.x), p.y.unwrap_or(old.y), w, h);
            if node.kind == Kind::Text && (w, h) != (old.w, old.h) {
                // Typing a width makes auto-width text wrap; a height fixes the box (Figma).
                self.scene.update(id, |n| {
                    if let Some(t) = n.text.as_mut() {
                        if h != old.h {
                            t.resize = TextResize::Fixed;
                        } else if t.resize == TextResize::AutoWidth {
                            t.resize = TextResize::AutoHeight;
                        }
                    }
                });
            }
            self.scene.set_bounds(id, r);
            self.fit_text(id);
            // Typing a size makes that axis fixed, as in Figma.
            if node.kind != Kind::Text {
                self.scene.update(id, |n| {
                    if w != old.w {
                        n.sizing[0] = Sizing::Fixed;
                    }
                    if h != old.h {
                        n.sizing[1] = Sizing::Fixed;
                    }
                });
            }
        }
        if let Some(sizing) = p.sizing_h {
            self.set_sizing(id, 0, sizing);
        }
        if let Some(sizing) = p.sizing_v {
            self.set_sizing(id, 1, sizing);
        }
        if let Some(r) = p.rotation {
            let (cx, cy) = node.bounds.center();
            self.scene.rotate(id, normalize_degrees(r - node.rotation), cx, cy);
        }
        self.scene.update(id, |n| {
            if let Some(r) = p.radius {
                n.radius = r.max(0.0);
                n.corners = None;
            }
            if let Some(c) = &p.corners {
                n.corners = (c.len() == 4).then(|| [c[0], c[1], c[2], c[3]].map(|v| v.max(0.0)));
                if let Some(c) = n.corners {
                    n.radius = c[0];
                }
            }
            if let Some(rgb) = p.fill.as_deref().and_then(parse_hex) {
                n.fill = with_rgb(n.fill, rgb);
                n.fill_on = true;
            }
            if let Some(a) = p.fill_opacity {
                n.fill = with_alpha_pct(n.fill, a);
            }
            if let Some(on) = p.fill_on {
                n.fill_on = on;
            }
            if let Some(on) = p.stroke_on {
                n.stroke = if on { Some(n.stroke.unwrap_or((BLACK, 1.0))) } else { None };
            }
            if let Some((color, width)) = n.stroke.as_mut() {
                if let Some(rgb) = p.stroke_color.as_deref().and_then(parse_hex) {
                    *color = with_rgb(*color, rgb);
                }
                if let Some(a) = p.stroke_opacity {
                    *color = with_alpha_pct(*color, a);
                }
                if let Some(w) = p.stroke_width {
                    *width = w.max(0.0);
                }
            }
            if let Some(a) = p.stroke_align {
                n.stroke_align = a;
            }
            if let Some(o) = p.opacity {
                n.opacity = (o / 100.0).clamp(0.0, 1.0);
            }
            if let Some(b) = p.blend {
                n.blend = b;
            }
            if let Some(l) = p.lock_ratio {
                n.lock_ratio = l;
            }
            if let Some(a) = p.absolute {
                n.absolute = a;
            }
            if let Some(g) = &p.grids {
                n.grids = g.clone();
            }
            if let Some(r) = p.ready {
                n.ready = r;
            }
            if let Some(s) = &p.stroke_sides {
                n.sides = match s.as_slice() {
                    [t, r, b, l, ..] if matches!(n.kind, Kind::Rectangle | Kind::Frame) => Some([*t, *r, *b, *l].map(|w| w.max(0.0))),
                    _ => None,
                };
                if let (Some(sides), Some((_, w))) = (n.sides, n.stroke.as_mut()) {
                    *w = sides.iter().copied().fold(0.0, f32::max);
                }
            }
            if let Some(s) = p.scroll.filter(|_| n.kind == Kind::Frame) {
                n.scroll = s;
                if !s.is_none() {
                    n.clip = true;
                }
            }
            if let Some(v) = n.vector.as_deref_mut() {
                if let Some(s) = p.start_point {
                    v.start = s;
                }
                if let Some(s) = p.end_point {
                    v.end = s;
                }
            }
            if let Some(text) = &p.note {
                let t = text.trim();
                n.note = (!t.is_empty()).then(|| Box::new(t.to_string()));
            }
            if let Some(d) = &p.dash {
                n.dash = match d.as_slice() {
                    [a] => Some([*a, *a]),
                    [a, b, ..] => Some([*a, *b]),
                    _ => None,
                };
            }
            if let Some(c) = p.constraint_h {
                n.constraints[0] = c;
            }
            if let Some(c) = p.constraint_v {
                n.constraints[1] = c;
            }
            if let Some(effects) = &p.effects {
                n.effects = effects.iter().map(EffectInfo::to_effect).collect();
            }
            if let Some(exports) = &p.exports {
                n.exports = exports.clone();
            }
            if let Some(layout) = n.layout.as_deref_mut() {
                if let Some(f) = p.flow {
                    layout.flow = f;
                }
                if let Some(g) = p.gap {
                    layout.gap = g.max(0.0);
                }
                if let Some(g) = p.counter_gap {
                    layout.counter_gap = g.max(0.0);
                }
                if let Some(pad) = p.padding.as_deref().filter(|v| v.len() == 4) {
                    layout.padding = [pad[0], pad[1], pad[2], pad[3]].map(|v| v.max(0.0));
                }
                if let Some(j) = p.justify {
                    layout.justify = j;
                }
                if let Some(a) = p.align {
                    layout.align = a;
                }
                if let Some(c) = p.columns {
                    layout.columns = c.clamp(1, 64);
                }
                if let Some(r) = p.rows {
                    layout.rows = r.clamp(1, 64);
                }
            }
        });
    }

    /// Centre of the box around the movable selection.
    fn pivot(&self, ids: &[NodeId]) -> Option<(f32, f32)> {
        Rect::union_all(ids.iter().filter_map(|id| self.scene.node(*id)).map(|n| n.aabb())).map(|r| r.center())
    }

    /// Which selection handle is under a screen point (see [`handle_point`]); -1 for none.
    fn handle_at(&self, sx: f32, sy: f32) -> i32 {
        if self.movable().is_empty() {
            return -1;
        }
        let Some((rect, rotation)) = self.scene.selection_box(&self.selection) else { return -1 };
        let (rect, rotation) = match self.selected().as_slice() {
            [one] if one.kind == Kind::Group => (one.aabb(), 0.0),
            _ => (rect, rotation),
        };
        let (wx, wy) = self.world(sx, sy);
        let (cx, cy) = rect.center();
        let (x, y) = rotate_point(wx, wy, cx, cy, -rotation);
        let z = self.view.zoom;
        let near = |px: f32| px / z;
        for h in [0, 2, 4, 6] {
            let (hx, hy) = handle_point(&rect, h);
            if (x - hx).abs() <= near(HANDLE_HIT_PX) && (y - hy).abs() <= near(HANDLE_HIT_PX) {
                return h as i32;
            }
        }
        let inside_x = x > rect.x && x < rect.right();
        let inside_y = y > rect.y && y < rect.bottom();
        if inside_x && (y - rect.y).abs() <= near(EDGE_HIT_PX) {
            return 1;
        }
        if inside_x && (y - rect.bottom()).abs() <= near(EDGE_HIT_PX) {
            return 5;
        }
        if inside_y && (x - rect.x).abs() <= near(EDGE_HIT_PX) {
            return 7;
        }
        if inside_y && (x - rect.right()).abs() <= near(EDGE_HIT_PX) {
            return 3;
        }
        if !(rect.contains_point(x, y)) {
            for (i, h) in [0u32, 2, 4, 6].into_iter().enumerate() {
                let (hx, hy) = handle_point(&rect, h);
                if (x - hx).hypot(y - hy) <= near(ROTATE_HIT_PX) {
                    return 8 + i as i32;
                }
            }
        }
        -1
    }

    fn transform_to(&mut self, sx: f32, sy: f32, shift: bool, alt: bool) {
        self.transform_apply(sx, sy, shift, alt);
        self.relayout();
    }

    fn transform_apply(&mut self, sx: f32, sy: f32, shift: bool, alt: bool) {
        let Some(drag) = self.drag.as_ref() else { return };
        let (handle, b, rotation) = (drag.handle, drag.rect, drag.rotation);
        let (cx, cy) = b.center();
        let (wx, wy) = self.world(sx, sy);
        if handle >= 8 {
            let a = angle(cx, cy, wx, wy);
            let mut delta = normalize_degrees(a - drag.start_angle);
            if shift {
                let base = if drag.nodes.len() == 1 { drag.nodes[0].2 } else { 0.0 };
                delta = ((base + delta) / 15.0).round() * 15.0 - base;
            }
            let step = normalize_degrees(delta - drag.applied);
            let ids: Vec<NodeId> = drag.nodes.iter().map(|n| n.0).collect();
            if let Some(d) = self.drag.as_mut() {
                d.applied = delta;
            }
            for id in ids {
                self.scene.rotate(id, step, cx, cy);
            }
            return;
        }
        let (gx, gy) = drag.grab;
        let (lx, ly) = rotate_point(wx, wy, cx, cy, -rotation);
        let (lx, ly) = (lx + gx, ly + gy);
        let (mut x0, mut y0, mut x1, mut y1) = (b.x, b.y, b.right(), b.bottom());
        if moves_left(handle) {
            x0 = lx;
            if alt {
                x1 = 2.0 * cx - lx;
            }
        }
        if moves_right(handle) {
            x1 = lx;
            if alt {
                x0 = 2.0 * cx - lx;
            }
        }
        if moves_top(handle) {
            y0 = ly;
            if alt {
                y1 = 2.0 * cy - ly;
            }
        }
        if moves_bottom(handle) {
            y1 = ly;
            if alt {
                y0 = 2.0 * cy - ly;
            }
        }
        let lock = shift || drag.nodes.len() == 1 && self.scene.node(drag.nodes[0].0).is_some_and(|n| n.lock_ratio);
        if lock && b.w > 0.0 && b.h > 0.0 {
            let corner = matches!(handle, 0 | 2 | 4 | 6);
            let sx_ = (x1 - x0) / b.w;
            let sy_ = (y1 - y0) / b.h;
            let s = if corner { if sx_.abs() > sy_.abs() { sx_ } else { sy_ } } else if handle == 1 || handle == 5 { sy_ } else { sx_ };
            let (w, h) = (b.w * s, b.h * s);
            // Keep the anchored side (or the centre with Alt) where it was.
            let ax = if alt || !corner && (handle == 1 || handle == 5) { (x0 + x1) / 2.0 - w / 2.0 } else if moves_left(handle) { x1 - w } else { x0 };
            let ay = if alt || !corner && (handle == 3 || handle == 7) { (y0 + y1) / 2.0 - h / 2.0 } else if moves_top(handle) { y1 - h } else { y0 };
            (x0, y0, x1, y1) = (ax, ay, ax + w, ay + h);
        }
        let min = 1.0 / self.view.zoom.max(1.0);
        if x1 - x0 < min {
            if moves_left(handle) { x0 = x1 - min } else { x1 = x0 + min }
        }
        if y1 - y0 < min {
            if moves_top(handle) { y0 = y1 - min } else { y1 = y0 + min }
        }
        // Back to world: the new local centre turns about the old centre.
        let (ncx, ncy) = rotate_point((x0 + x1) / 2.0, (y0 + y1) / 2.0, cx, cy, rotation);
        let target = Rect::new(0.0, 0.0, x1 - x0, y1 - y0).centered(ncx, ncy);
        let nodes = drag.nodes.clone();
        let scaled = drag.scaled;
        let axes = [moves_left(handle) || moves_right(handle), moves_top(handle) || moves_bottom(handle)];
        for (id, _, _) in nodes.iter().filter(|_| scaled.is_none()) {
            if self.scene.node(*id).is_some_and(|n| n.kind != Kind::Text) {
                self.scene.update(*id, |n| {
                    for (axis, on) in axes.into_iter().enumerate() {
                        if on {
                            n.sizing[axis] = Sizing::Fixed;
                        }
                    }
                });
            }
        }
        if nodes.len() == 1 && scaled.is_none() {
            let (id, _, _) = nodes[0];
            if self.scene.node(id).is_some_and(|n| n.kind == Kind::Text) {
                let width_only = matches!(handle, 3 | 7);
                self.scene.update(id, |n| {
                    if let Some(t) = n.text.as_mut() {
                        if !width_only {
                            t.resize = TextResize::Fixed;
                        } else if t.resize == TextResize::AutoWidth {
                            t.resize = TextResize::AutoHeight;
                        }
                    }
                });
            }
            self.scene.set_bounds(id, target);
            self.fit_text(id);
            return;
        }
        if let Some(prev) = scaled {
            // Scale tool: grow everything from the last step's box to the new one.
            let k = if prev.w > 0.0 { target.w / prev.w } else { 1.0 };
            for (id, _, _) in &nodes {
                self.scene.scale_about(*id, k, prev.x, prev.y);
                self.scene.translate(*id, target.x - prev.x, target.y - prev.y);
                self.fit_text(*id);
            }
            if let Some(d) = self.drag.as_mut() {
                d.scaled = Some(target);
            }
            return;
        }
        let (sxf, syf) = (if b.w > 0.0 { target.w / b.w } else { 1.0 }, if b.h > 0.0 { target.h / b.h } else { 1.0 });
        for (id, r, _) in nodes {
            let (ncx, ncy) = r.center();
            let nr = Rect::new(0.0, 0.0, r.w * sxf, r.h * syf)
                .centered(target.x + (ncx - b.x) * sxf, target.y + (ncy - b.y) * syf);
            self.scene.update(id, |n| {
                if let Some(t) = n.text.as_mut() {
                    t.resize = TextResize::Fixed;
                }
            });
            self.scene.set_bounds(id, nr);
        }
    }

    fn selected_text(&self, id: i32) -> Vec<NodeId> {
        let ids = if id >= 0 { vec![id as NodeId] } else { self.selection.clone() };
        ids.into_iter().filter(|id| self.scene.node(*id).is_some_and(|n| n.kind == Kind::Text)).collect()
    }

    fn selected(&self) -> Vec<&Node> {
        self.selection.iter().filter_map(|id| self.scene.node(*id)).collect()
    }

    fn selection_bounds(&self) -> Option<Rect> {
        Rect::union_all(self.selected().iter().map(|n| n.aabb()))
    }

    fn world(&self, sx: f32, sy: f32) -> (f32, f32) {
        self.view.to_world(sx, sy)
    }

    /// Adds a layer tree centred on a screen point (NaN: the view's centre), inside the frame
    /// under that point, selects it and returns its id.
    fn place_tree(&mut self, mut tree: Tree, sx: f32, sy: f32) -> i32 {
        let (x, y) = if sx.is_nan() || sy.is_nan() { self.view.world_area().center() } else { self.world(sx, sy) };
        let a = tree.aabb();
        tree.translate((x - a.w / 2.0 - a.x).round(), (y - a.h / 2.0 - a.y).round());
        let placed = tree.aabb();
        self.step(|e| {
            // Into the frame under the point only if it is big enough to hold it (dropping a
            // photo on an icon must not put it inside the icon).
            let mut parent = e.scene.drop_target(x, y, &[]);
            while let Some(p) = parent.and_then(|p| e.scene.node(p)) {
                if p.bounds.contains(&placed) {
                    break;
                }
                parent = p.parent;
            }
            let id = e.scene.insert_tree(&tree, parent, None);
            e.selection = vec![id];
            id as i32
        })
    }

    /// Runs one undoable step, recording the selection before and after it.
    fn step<T>(&mut self, f: impl FnOnce(&mut Engine) -> T) -> T {
        let before = self.selection.clone();
        self.scene.begin(&before);
        let out = f(self);
        self.settle();
        self.prune_selection();
        let after = self.selection.clone();
        self.scene.commit(&after);
        out
    }

    fn prune_selection(&mut self) {
        let scene = &self.scene;
        self.selection.retain(|id| scene.node(*id).is_some());
        if self.hover.is_some_and(|id| scene.node(id).is_none()) {
            self.hover = None;
        }
    }

    /// Layers a drag or handle may move: unlocked and not inside an instance.
    fn movable(&self) -> Vec<NodeId> {
        self.editable().into_iter().filter(|id| !self.scene.is_locked(*id)).collect()
    }

    fn round_positions(&mut self, ids: &[NodeId]) {
        for id in ids {
            if let Some(b) = self.scene.node(*id).map(|n| n.bounds) {
                self.scene.translate(*id, b.x.round() - b.x, b.y.round() - b.y);
            }
        }
    }

    fn restore_selection(&mut self, selection: Option<Vec<NodeId>>) -> i32 {
        let Some(selection) = selection else { return 0 };
        self.selection = selection;
        self.prune_selection();
        1
    }

    /// Pastes into the selected frame, or next to the selected layers, or onto the canvas.
    /// Layers keep their coordinates when those are inside the target, otherwise they are centred.
    fn paste(&mut self, mut trees: Vec<Tree>) -> u32 {
        let Some(area) = bounds_of(&trees) else { return 0 };
        let sel = self.scene.outermost(&self.selection);
        let frame = match sel.as_slice() {
            [one] => self.scene.node(*one).filter(|n| n.kind == Kind::Frame).map(|n| (n.id, n.bounds)),
            _ => None,
        };
        let (parent, index, region) = match (frame, sel.last()) {
            (Some((frame, bounds)), _) => (Some(frame), None, bounds),
            (None, Some(last)) => match self.scene.position(*last) {
                Some((parent, i)) => {
                    let region = parent.and_then(|p| self.scene.node(p)).map_or(self.view.world_area(), |n| n.bounds);
                    (parent, Some(i + 1), region)
                }
                None => (None, None, self.view.world_area()),
            },
            (None, None) => (None, None, self.view.world_area()),
        };
        let (cx, cy) = area.center();
        if !region.contains_point(cx, cy) {
            let (rx, ry) = region.center();
            let (dx, dy) = ((rx - cx).round(), (ry - cy).round());
            trees.iter_mut().for_each(|t| t.translate(dx, dy));
        }
        self.step(|e| {
            let mut ids = Vec::new();
            for (k, t) in trees.iter().enumerate() {
                let at = index.map(|i| i + k);
                let main = t.main.filter(|m| e.scene.is_main(*m));
                let id = match main {
                    // A copied main component pastes as an instance of it.
                    Some(m) if t.is_main => e.scene.create_instance(m, parent, at, t.aabb().center()),
                    Some(m) => {
                        let id = e.scene.insert_tree(t, parent, at);
                        e.scene.relink_instance(id, m);
                        Some(id)
                    }
                    None => Some(e.scene.insert_tree(t, parent, at)),
                };
                ids.extend(id);
            }
            e.selection = ids;
            e.selection.len() as u32
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_init(width: i32, height: i32, dpr: f32) -> i32 {
    if unsafe { pergel_gl_create(CANVAS_SELECTOR.as_ptr()) } <= 0 {
        return -1;
    }
    let Some(interface) = gpu::gl::Interface::new_native() else { return -2 };
    let Some(mut context) = gpu::direct_contexts::make_gl(interface, None) else { return -3 };
    let Some(surface) = make_surface(&mut context, width, height) else { return -4 };
    let scene = Scene::sample();
    let mut view = Viewport::new(width as f32 / dpr, height as f32 / dpr);
    view.fit(scene.bounds(), FIT_MARGIN);
    ENGINE.set(Some(Engine {
        context,
        surface,
        scene,
        renderer: Renderer::new(),
        view,
        dpr,
        selection: Vec::new(),
        hover: None,
        marquee: None,
        editing: None,
        text_style: Text::default(),
        drag: None,
        slot: 0,
        parked: HashMap::new(),
        pages: vec![PageSlot::new("Page 1", Viewport::new(1.0, 1.0))],
        page: 0,
        page_rev: 0,
        variables: pergel_core::Variables::default(),
        styles: pergel_core::Styles::default(),
        vedit: None,
        show_connections: false,
        present_swaps: Vec::new(),
        present_vars: Vec::new(),
        comments: Vec::new(),
        uid_seed: 0,
        branch: None,
        moving: None,
        guides: Vec::new(),
        show_grids: true,
        page_guides: Vec::new(),
    }));
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_load(which: u32, count: u32) {
    with_engine(|e| e.reset_scene(if which == 0 { Scene::sample() } else { Scene::demo(count as usize) }));
}

/// Opens the document JSON in the text buffer as tab `slot` and shows it. 1 on success, 0 when
/// the text is not a Pergel document (nothing changes then).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_open(slot: u32) -> i32 {
    let Ok(doc) = Document::parse(&text()) else { return 0 };
    with_engine(|e| {
        if e.slot != slot {
            e.park();
        }
        e.parked.remove(&slot);
        e.slot = slot;
        e.selection.clear();
        e.renderer.text.retain(|_| false);
        let (w, h) = (e.view.width, e.view.height);
        let saved_view = |scene: &Scene, view: &serde_json::Value| {
            let mut v = Viewport::new(w, h);
            let get = |k: &str| view.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
            match (get("x"), get("y"), get("zoom")) {
                (Some(x), Some(y), Some(zoom)) if zoom > 0.0 => {
                    v.x = x;
                    v.y = y;
                    v.zoom = zoom;
                }
                _ if scene.is_empty() => {
                    // A new page: the canvas origin in the middle, at 100%.
                    v.x = -w / 2.0;
                    v.y = -h / 2.0;
                }
                _ => v.fit(scene.bounds(), FIT_MARGIN),
            }
            v
        };
        e.pages = doc
            .pages
            .iter()
            .map(|p| {
                let scene = Scene::from_trees(&p.layers);
                let view = saved_view(&scene, &p.view);
                PageSlot { name: p.name.clone(), scene, view, selection: Vec::new(), guides: p.guides.clone() }
            })
            .collect();
        if e.pages.is_empty() {
            let scene = Scene::default();
            let view = saved_view(&scene, &serde_json::Value::Null);
            e.pages.push(PageSlot { name: "Page 1".into(), scene, view, selection: Vec::new(), guides: Vec::new() });
        }
        e.page = 0;
        e.page_guides = std::mem::take(&mut e.pages[0].guides);
        e.comments = doc.comments.clone();
        e.branch = doc.branch.clone();
        e.variables = doc.variables.clone();
        e.styles = doc.styles.clone();
        e.scene = std::mem::take(&mut e.pages[0].scene);
        e.scene.images = doc.image_store();
        e.view = e.pages[0].view;
        e.fit_all_text();
        1
    })
    .unwrap_or(0)
}

/// Live collaboration: the shown document becomes another peer's copy (`text()`), keeping the
/// page shown, each page's view and the selection where it still exists. Returns 0 while a
/// drag or an edit is in progress (the caller retries), -1 for text that is not a document.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_replace() -> i32 {
    let Ok(doc) = Document::parse(&text()) else { return -1 };
    with_engine(|e| {
        if e.vedit.is_some() || e.drag.is_some() || e.moving.is_some() || e.editing.is_some() {
            return 0;
        }
        let mut views: Vec<Viewport> = e.pages.iter().map(|p| p.view).collect();
        if let Some(v) = views.get_mut(e.page) {
            *v = e.view;
        }
        let (w, h) = (e.view.width, e.view.height);
        let selection = e.selection.clone();
        e.renderer.text.retain(|_| false);
        e.pages = doc
            .pages
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let scene = Scene::from_trees(&p.layers);
                let view = views.get(i).copied().unwrap_or_else(|| {
                    let mut v = Viewport::new(w, h);
                    v.fit(scene.bounds(), FIT_MARGIN);
                    v
                });
                PageSlot { name: p.name.clone(), scene, view, selection: Vec::new(), guides: p.guides.clone() }
            })
            .collect();
        if e.pages.is_empty() {
            e.pages.push(PageSlot { name: "Page 1".into(), scene: Scene::default(), view: Viewport::new(w, h), selection: Vec::new(), guides: Vec::new() });
        }
        e.page = e.page.min(e.pages.len() - 1);
        let page = e.page;
        e.page_guides = std::mem::take(&mut e.pages[page].guides);
        e.variables = doc.variables.clone();
        e.styles = doc.styles.clone();
        e.comments = doc.comments.clone();
        e.branch = doc.branch.clone();
        e.scene = std::mem::take(&mut e.pages[page].scene);
        e.scene.images = doc.image_store();
        e.view = e.pages[page].view;
        e.selection = selection.into_iter().filter(|id| e.scene.node(*id).is_some()).collect();
        e.fit_all_text();
        1
    })
    .unwrap_or(0)
}

/// Live collaboration: the document with every top-level layer's stable `uid` filled in.
/// `text()` holds this peer's random prefix, so two peers never make the same uid.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_json_live() -> *const c_char {
    let prefix = text();
    emit(with_engine(|e| {
        let mut n = e.uid_seed;
        let mut fresh = || {
            n += 1;
            format!("{prefix}-{n}")
        };
        e.scene.assign_uids(&mut fresh);
        for p in &mut e.pages {
            p.scene.assign_uids(&mut fresh);
        }
        e.uid_seed = n;
        e.document_json()
    }))
}

/// Team libraries: the document's components as a library package. `text()` holds this
/// peer's uid prefix. Returns `{components: [{key, name, set, page, nodeId, w, h, tree}]}`
/// (nodeId only for the shown page, for thumbnails).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_library_package() -> *const c_char {
    let prefix = text();
    emit(with_engine(|e| {
        let mut n = e.uid_seed;
        let mut list = Vec::new();
        let current = e.page;
        let count = e.pages.len();
        for pi in 0..count {
            let scene = if pi == current { &mut e.scene } else { &mut e.pages[pi].scene };
            // Component sets as a whole, and mains that are not variants.
            let ids: Vec<NodeId> = scene
                .nodes()
                .filter(|x| x.variant_set || (x.component == Some(Component::Main) && scene.variant_set_of(x.id).is_none()))
                .filter(|x| x.library.is_none())
                .map(|x| x.id)
                .collect();
            for id in ids {
                let key = scene.ensure_uid(id, || {
                    n += 1;
                    format!("{prefix}-{n}")
                });
                let Some(node) = scene.node(id) else { continue };
                let Some(tree) = scene.extract(id) else { continue };
                list.push(serde_json::json!({
                    "key": key, "name": node.name, "set": node.variant_set, "page": pi,
                    "nodeId": if pi == current { Some(id) } else { None },
                    "w": node.bounds.w, "h": node.bounds.h, "tree": tree,
                }));
            }
        }
        e.uid_seed = n;
        e.page_rev += 1;
        serde_json::json!({ "components": list }).to_string()
    }))
}

/// Team libraries: local mains that came from libraries: `[{lib, key, version, id}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_library_refs() -> *const c_char {
    emit(with_engine(|e| {
        let list: Vec<serde_json::Value> = e
            .scene
            .nodes()
            .filter_map(|n| n.library.as_deref().map(|l| serde_json::json!({ "lib": l.lib, "key": l.key, "version": l.version, "id": n.id, "name": n.name })))
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

impl Engine {
    /// Brings a library component's tree into the document (under the managed container),
    /// marked with where it came from. Nested instances become plain layers: their mains are
    /// in the other document.
    fn import_library(&mut self, lib: &pergel_core::LibraryRef, tree: &Tree) -> Option<NodeId> {
        fn unlink(t: &mut Tree, root: bool) {
            if !root && !t.is_main {
                t.main = None;
            }
            t.uid = None;
            t.comments.clear();
            for c in &mut t.children {
                unlink(c, false);
            }
        }
        let mut tree = tree.clone();
        unlink(&mut tree, true);
        for c in &mut tree.children {
            // Variants of a set stay mains.
            if tree.variant_set {
                c.main = c.saved_id;
            }
        }
        let container = match self.scene.roots().iter().copied().find(|r| self.scene.node(*r).is_some_and(|n| n.uid.as_deref() == Some("pergel-library-components"))) {
            Some(c) => c,
            None => {
                let mut f = Node::new(Kind::Frame, "Library components (managed)", Rect::new(-20_000.0, -20_000.0, 400.0, 400.0), WHITE);
                f.section = true;
                f.clip = false;
                f.uid = Some("pergel-library-components".into());
                f.note = Some(Box::new("Components used from team libraries. Pergel keeps them up to date; edit them in their library file.".into()));
                self.scene.insert(f, None, None)
            }
        };
        let cb = self.scene.node(container)?.bounds;
        let kids = self.scene.node(container)?.children.len() as f32;
        let at = (cb.x + 40.0, cb.y + 40.0 + kids * (tree.bounds.h + 40.0));
        tree.translate(at.0 - tree.bounds.x, at.1 - tree.bounds.y);
        let id = self.scene.insert_tree(&tree, Some(container), None);
        // Mains stay mains (insert_tree copies properties only); layers in preorder line up.
        fn flags(t: &Tree, out: &mut Vec<bool>) {
            out.push(t.is_main);
            for c in &t.children {
                flags(c, out);
            }
        }
        let mut mains = Vec::new();
        flags(&tree, &mut mains);
        for (n, is_main) in self.scene.subtree(id).into_iter().zip(mains) {
            if is_main {
                self.scene.update(n, |x| x.component = Some(Component::Main));
            }
        }
        let lib = lib.clone();
        self.scene.update(id, |n| n.library = Some(Box::new(lib)));
        // Grow the managed section around what it holds.
        if let Some(r) = Rect::union_all(self.scene.node(container)?.children.iter().filter_map(|c| self.scene.node(*c).map(|n| n.aabb()))) {
            self.scene.set_bounds(container, Rect::new(cb.x, cb.y, (r.right() - cb.x + 40.0).max(400.0), (r.bottom() - cb.y + 40.0).max(400.0)));
        }
        Some(id)
    }

    /// The main to make instances of: the component, or a set's first variant.
    fn library_main(&self, id: NodeId) -> Option<NodeId> {
        let n = self.scene.node(id)?;
        if n.variant_set { n.children.iter().copied().find(|c| self.scene.is_main(*c)) } else { Some(id) }
    }
}

/// Team libraries: places an instance of a library component (imported on first use).
/// `text()` holds `{lib, libName, key, version, tree, x, y}`; returns the instance id or -1.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_library_use() -> i32 {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Req {
        lib: String,
        #[serde(default)]
        lib_name: String,
        key: String,
        version: f64,
        tree: Tree,
        x: Option<f32>,
        y: Option<f32>,
    }
    let Ok(r) = serde_json::from_str::<Req>(&text()) else { return -1 };
    with_engine(|e| {
        e.step(|e| {
            let existing = e.scene.nodes().find(|n| n.library.as_deref().is_some_and(|l| l.lib == r.lib && l.key == r.key)).map(|n| n.id);
            let local = match existing {
                Some(id) => id,
                None => {
                    let lib = pergel_core::LibraryRef { lib: r.lib.clone(), key: r.key.clone(), version: r.version, lib_name: r.lib_name.clone() };
                    e.import_library(&lib, &r.tree)?
                }
            };
            let main = e.library_main(local)?;
            let area = e.view.world_area();
            let at = (r.x.unwrap_or(area.center().0), r.y.unwrap_or(area.center().1));
            let parent = e.scene.drop_target(at.0, at.1, &[]);
            let inst = e.scene.create_instance(main, parent, None, at)?;
            e.selection = vec![inst];
            Some(inst as i32)
        })
    })
    .flatten()
    .unwrap_or(-1)
}

/// Team libraries: updates a library component to a newer published version; its instances
/// follow (keeping their text). `text()` holds `{lib, key, version, tree}`. Returns instances moved.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_library_update() -> i32 {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Req {
        lib: String,
        #[serde(default)]
        lib_name: String,
        key: String,
        version: f64,
        tree: Tree,
    }
    let Ok(r) = serde_json::from_str::<Req>(&text()) else { return -1 };
    with_engine(|e| {
        e.step(|e| {
            let old = e.scene.nodes().find(|n| n.library.as_deref().is_some_and(|l| l.lib == r.lib && l.key == r.key)).map(|n| n.id)?;
            let lib = pergel_core::LibraryRef { lib: r.lib.clone(), key: r.key.clone(), version: r.version, lib_name: r.lib_name.clone() };
            let new = e.import_library(&lib, &r.tree)?;
            // Instances move to the new main (the same variant by name in a set).
            let old_mains: Vec<NodeId> = if e.scene.node(old).is_some_and(|n| n.variant_set) { e.scene.node(old)?.children.clone() } else { vec![old] };
            let mut moved = 0;
            for m in old_mains {
                let name = e.scene.node(m).map(|n| n.name.clone()).unwrap_or_default();
                let target = if e.scene.node(new).is_some_and(|n| n.variant_set) {
                    e.scene.node(new)?.children.iter().copied().find(|c| e.scene.node(*c).is_some_and(|n| n.name == name)).or_else(|| e.library_main(new))
                } else {
                    Some(new)
                };
                let Some(target) = target else { continue };
                for inst in e.scene.instances_of(m) {
                    if e.scene.swap_instance(inst, target).is_some() {
                        moved += 1;
                    }
                }
            }
            e.scene.remove(old);
            Some(moved)
        })
    })
    .flatten()
    .unwrap_or(-1)
}

/// Branching: the shown document's branch info (`{of, ofName, base, merged}`) or null.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_branch_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.branch).unwrap_or_default()))
}

/// Branching: sets (or with `null` clears) the branch info of the shown document.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_branch_set() {
    let b: Option<pergel_core::Branch> = serde_json::from_str(&text()).unwrap_or(None);
    with_engine(|e| {
        e.branch = b;
        e.page_rev += 1;
    });
}

/// Branching: what changed from `base` to `other` (`text()` holds `{base, other}`):
/// `[{page, name, change}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_diff() -> *const c_char {
    #[derive(Deserialize)]
    struct Req {
        base: String,
        other: String,
    }
    let out = (|| {
        let r: Req = serde_json::from_str(&text()).ok()?;
        let (base, other) = (Document::parse(&r.base).ok()?, Document::parse(&r.other).ok()?);
        let list: Vec<serde_json::Value> =
            pergel_core::diff_documents(&base, &other).into_iter().map(|(page, name, change)| serde_json::json!({ "page": page, "name": name, "change": change })).collect();
        serde_json::to_string(&list).ok()
    })();
    emit(out)
}

/// Live collaboration: three-way merge. `text()` holds `{base, local, remote}` documents;
/// returns `{doc, ahead}` (ahead: the result has local changes the remote lacks).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_merge() -> *const c_char {
    #[derive(Deserialize)]
    struct Req {
        base: String,
        local: String,
        remote: String,
    }
    let out = (|| {
        let r: Req = serde_json::from_str(&text()).ok()?;
        let (base, local, remote) = (Document::parse(&r.base).ok()?, Document::parse(&r.local).ok()?, Document::parse(&r.remote).ok()?);
        let (doc, ahead) = pergel_core::merge_documents(&base, &local, &remote);
        Some(serde_json::json!({ "doc": serde_json::to_string(&doc).ok()?, "ahead": ahead }).to_string())
    })();
    emit(out)
}

/// Comment threads on the shown page: `[{id, x, y (canvas), node, nodeName, resolved, messages}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_comments_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.comment_list()).unwrap_or_default()))
}

/// Changes comments; `text()` holds `{op: "add"|"reply"|"resolve"|"delete"|"move", id, x, y, text,
/// author, time, resolved}` (x, y on the canvas). Returns the thread id, or "".
#[unsafe(no_mangle)]
pub extern "C" fn pergel_comment_op() -> *const c_char {
    let Ok(req) = serde_json::from_str::<serde_json::Value>(&text()) else { return emit(None) };
    emit(with_engine(|e| e.comment_op(&req)).flatten())
}

impl Engine {
    fn comment_list(&self) -> Vec<serde_json::Value> {
        let mut out = Vec::new();
        for n in self.scene.nodes().filter(|n| !n.comments.is_empty()) {
            for t in &n.comments {
                out.push(serde_json::json!({
                    "id": t.id, "x": n.bounds.x + t.x, "y": n.bounds.y + t.y, "node": n.id, "nodeName": n.name,
                    "resolved": t.resolved, "messages": t.messages,
                }));
            }
        }
        for c in self.comments.iter().filter(|c| c.page == self.page) {
            let t = &c.thread;
            out.push(serde_json::json!({ "id": t.id, "x": t.x, "y": t.y, "node": null, "nodeName": null, "resolved": t.resolved, "messages": t.messages }));
        }
        out
    }

    /// Where a thread lives: (layer, index) or the page list index.
    fn find_thread(&self, id: &str) -> Option<Result<(NodeId, usize), usize>> {
        for n in self.scene.nodes() {
            if let Some(i) = n.comments.iter().position(|t| t.id == id) {
                return Some(Ok((n.id, i)));
            }
        }
        self.comments.iter().position(|c| c.thread.id == id && c.page == self.page).map(Err)
    }

    fn edit_thread(&mut self, id: &str, f: impl FnOnce(&mut pergel_core::CommentThread)) -> bool {
        match self.find_thread(id) {
            Some(Ok((node, i))) => {
                self.scene.update(node, |n| f(&mut n.comments[i]));
                true
            }
            Some(Err(i)) => {
                f(&mut self.comments[i].thread);
                self.page_rev += 1;
                true
            }
            None => false,
        }
    }

    fn comment_op(&mut self, req: &serde_json::Value) -> Option<String> {
        let s = |k: &str| req.get(k).and_then(|v| v.as_str()).map(str::to_string);
        let f = |k: &str| req.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
        let message = || pergel_core::Comment {
            author: s("author").unwrap_or_else(|| "You".into()),
            text: s("text").unwrap_or_default(),
            time: req.get("time").and_then(|v| v.as_f64()).unwrap_or(0.0),
        };
        match req.get("op").and_then(|v| v.as_str())? {
            "add" => {
                let (x, y) = (f("x")?, f("y")?);
                let id = s("id").unwrap_or_else(|| format!("c{}", self.scene.len() + self.comments.len() + 1));
                // On the topmost frame (or layer) under the point, else on the page.
                let target = s("nodeId").and_then(|v| v.parse::<NodeId>().ok()).filter(|n| self.scene.node(*n).is_some()).or_else(|| {
                    let point = Rect::new(x, y, 0.0, 0.0);
                    let hits: Vec<&Node> = self.scene.visible(&point).filter(|n| n.contains(x, y) && !self.scene.in_instance(n.id)).collect();
                    hits.iter().rev().find(|n| n.kind == Kind::Frame).or(hits.last()).map(|n| n.id)
                });
                let thread = |ox: f32, oy: f32| pergel_core::CommentThread { id: id.clone(), x: ox, y: oy, resolved: false, messages: vec![message()] };
                match target {
                    Some(node) => {
                        let b = self.scene.node(node)?.bounds;
                        let t = thread(x - b.x, y - b.y);
                        self.step(|e| e.scene.update(node, |n| n.comments.push(t)));
                    }
                    None => {
                        self.comments.push(pergel_core::PageComment { page: self.page, thread: thread(x, y) });
                        self.page_rev += 1;
                    }
                }
                Some(id)
            }
            "reply" => {
                let id = s("id")?;
                let m = message();
                self.edit_thread(&id, |t| t.messages.push(m)).then_some(id)
            }
            "resolve" => {
                let id = s("id")?;
                let r = req.get("resolved").and_then(|v| v.as_bool()).unwrap_or(true);
                self.edit_thread(&id, |t| t.resolved = r).then_some(id)
            }
            "delete" => {
                let id = s("id")?;
                match self.find_thread(&id)? {
                    Ok((node, i)) => {
                        self.step(|e| e.scene.update(node, |n| {
                            n.comments.remove(i);
                        }));
                    }
                    Err(i) => {
                        self.comments.remove(i);
                        self.page_rev += 1;
                    }
                }
                Some(id)
            }
            _ => None,
        }
    }
}

/// Every visible layer under a screen point, topmost first, for the "Select layer" menu.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_layers_at(sx: f32, sy: f32) -> *const c_char {
    emit(with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        let point = Rect::new(x, y, 0.0, 0.0);
        let mut hits: Vec<&Node> = e
            .scene
            .visible(&point)
            .filter(|n| {
                let inside = if n.kind == Kind::Group { n.aabb().contains_point(x, y) } else { n.contains(x, y) };
                inside && e.scene.reach(n.id).is_some_and(|clip| clip.iter().all(|c| c.contains_point(x, y)))
            })
            .collect();
        hits.reverse();
        let list: Vec<serde_json::Value> = hits
            .into_iter()
            .take(24)
            .map(|n| serde_json::json!({ "id": n.id, "name": n.name, "kind": n.kind.label(), "type": type_name(n) }))
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

/// SVG markup of a layer (`id`) or the first selected layer (-1).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_svg(id: i32) -> *const c_char {
    emit(with_engine(|e| {
        let id = if id >= 0 { Some(id as NodeId) } else { e.selection.first().copied() };
        id.and_then(|id| e.scene.to_svg(id)).unwrap_or_default()
    }))
}

/// The characters of every text layer in the selection, in layer order ("Copy as text").
#[unsafe(no_mangle)]
pub extern "C" fn pergel_text_content() -> *const c_char {
    emit(with_engine(|e| {
        let mut parts = Vec::new();
        for id in e.scene.outermost(&e.selection) {
            for i in e.scene.subtree(id) {
                if let Some(t) = e.scene.node(i).and_then(|n| n.text.as_deref()) {
                    parts.push(t.content.clone());
                }
            }
        }
        parts.join("\n")
    }))
}

/// "Paste to replace" (Ctrl+Shift+R): the clipboard layers take the place of each selected layer.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_paste_replace() -> u32 {
    let clip = text();
    with_engine(|e| {
        let Some(trees) = from_text(&clip, &mut e.scene.images) else { return 0 };
        let Some(area) = bounds_of(&trees) else { return 0 };
        let targets = e.editable();
        if targets.is_empty() {
            return e.paste(trees);
        }
        e.step(|e| {
            let mut placed = Vec::new();
            for target in targets {
                let Some((parent, index)) = e.scene.position(target) else { continue };
                let Some((cx, cy)) = e.scene.node(target).map(|n| n.aabb().center()) else { continue };
                let (ax, ay) = area.center();
                let (dx, dy) = ((cx - ax).round(), (cy - ay).round());
                for (k, t) in trees.iter().enumerate() {
                    let mut t = t.clone();
                    t.translate(dx, dy);
                    placed.push(e.scene.insert_tree(&t, parent, Some(index + 1 + k)));
                }
                e.scene.remove(target);
            }
            e.selection = placed;
            e.selection.len() as u32
        })
    })
    .unwrap_or(0)
}

/// "Resize to fit": selected frames shrink or grow around their children.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_resize_to_fit() -> u32 {
    with_engine(|e| {
        let ids = e.editable();
        e.step(|e| ids.iter().filter(|id| e.scene.resize_to_fit(**id)).count() as u32)
    })
    .unwrap_or(0)
}

/// Shows tab `slot` (an empty document if it was never opened).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_switch(slot: u32) {
    with_engine(|e| {
        if e.slot != slot {
            e.park();
            e.show(slot);
        }
    });
}

/// Forgets a tab's document. Switch away from it first.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_close(slot: u32) {
    with_engine(|e| {
        e.parked.remove(&slot);
    });
}

/// The shown document as `.pergel` JSON.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_json() -> *const c_char {
    emit(with_engine(|e| e.document_json()))
}

/// Changes whenever the shown document changes (for autosave).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_doc_revision() -> f64 {
    with_engine(|e| e.scene.generation() as f64 + e.page_rev as f64 * 1e9).unwrap_or(0.0)
}

/// Converts a Figma REST file to a Pergel document. The text buffer holds
/// `{"file": <GET /v1/files/:key?geometry=paths>, "images": {"<imageRef>": "<base64>"}}`;
/// returns `{"document": …, "report": …}` or `{"error": "…"}`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_figma_document() -> *const c_char {
    let input: serde_json::Value = match serde_json::from_slice(&text_bytes()) {
        Ok(v) => v,
        Err(e) => return emit(Some(serde_json::json!({ "error": format!("Bad input: {e}") }).to_string())),
    };
    let mut images = pergel_core::Images::default();
    if let Some(map) = input.get("images").and_then(|v| v.as_object()) {
        for (key, data) in map {
            if let Some(bytes) = data.as_str().and_then(|d| base64::engine::general_purpose::STANDARD.decode(d).ok()) {
                images.insert(key.clone(), std::sync::Arc::new(bytes));
            }
        }
    }
    let out = match pergel_core::figma_to_document(&input["file"], &images) {
        Ok((doc, report)) => serde_json::json!({ "document": doc, "report": report }),
        Err(e) => serde_json::json!({ "error": e }),
    };
    emit(Some(out.to_string()))
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_variables_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.variables).unwrap_or_default()))
}

/// Replaces the document's variables with the JSON in the text buffer and updates bound layers.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_variables_set() -> i32 {
    let Ok(vars) = serde_json::from_str::<pergel_core::Variables>(&text()) else { return -1 };
    with_engine(|e| {
        e.variables = vars;
        e.page_rev += 1;
        e.apply_variables() as i32
    })
    .unwrap_or(-1)
}

/// Binds (or with `"variable": null` unbinds) a property of the selected layers:
/// `{"prop": "fill", "variable": "<id>"}` in the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_bind() -> i32 {
    #[derive(Deserialize)]
    struct Req {
        prop: pergel_core::BindProp,
        variable: Option<String>,
    }
    let Ok(req) = serde_json::from_str::<Req>(&text()) else { return 0 };
    with_engine(|e| {
        let ids = e.editable();
        if ids.is_empty() || req.variable.as_ref().is_some_and(|v| e.variables.get(v).is_none()) {
            return 0;
        }
        let vars = e.variables.clone();
        e.step(|e| {
            for id in &ids {
                let var = req.variable.clone();
                e.scene.update(*id, |n| {
                    let mut b = n.bindings.take().map(|b| *b).unwrap_or_default();
                    match var {
                        Some(v) => {
                            b.insert(req.prop, v);
                        }
                        None => {
                            b.remove(&req.prop);
                        }
                    }
                    n.bindings = (!b.is_empty()).then(|| Box::new(b));
                });
            }
            e.scene.apply_variables(&vars);
            ids.len() as i32
        })
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_styles_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.styles).unwrap_or_default()))
}

/// Replaces the document's styles (JSON in the text buffer) and restyles linked layers.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_styles_set() -> i32 {
    let Ok(styles) = serde_json::from_str::<pergel_core::Styles>(&text()) else { return -1 };
    with_engine(|e| {
        e.styles = styles;
        e.page_rev += 1;
        e.apply_styles() as i32
    })
    .unwrap_or(-1)
}

/// Links (or with `"style": null` unlinks) the selection: `{"kind": "text|fill|stroke|effect", "style": "<id>"}`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_style_link() -> i32 {
    #[derive(Deserialize)]
    struct Req {
        kind: String,
        style: Option<String>,
    }
    let Ok(req) = serde_json::from_str::<Req>(&text()) else { return 0 };
    with_engine(|e| {
        let ids = e.editable();
        if ids.is_empty() || req.style.as_ref().is_some_and(|s| e.styles.get(s).is_none()) {
            return 0;
        }
        let styles = e.styles.clone();
        e.step(|e| {
            for id in &ids {
                let style = req.style.clone();
                let kind = req.kind.clone();
                e.scene.update(*id, |n| {
                    let mut links = n.styles.take().map(|b| *b).unwrap_or_default();
                    match kind.as_str() {
                        "text" => links.text = style,
                        "fill" => links.fill = style,
                        "stroke" => links.stroke = style,
                        _ => links.effect = style,
                    }
                    n.styles = (!links.is_empty()).then(|| Box::new(links));
                });
            }
            for id in e.scene.apply_styles(&styles) {
                e.fit_text(id);
            }
            ids.len() as i32
        })
    })
    .unwrap_or(0)
}

/// Creates a style named in the text buffer (`{"kind": …, "name": …}`) from the first selected
/// layer and links the selection to it. Returns the style id as JSON, or null.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_style_create() -> *const c_char {
    #[derive(Deserialize)]
    struct Req {
        kind: String,
        name: String,
    }
    let req = serde_json::from_str::<Req>(&text()).ok();
    emit(with_engine(|e| {
        let req = req?;
        let n = e.selection.first().and_then(|id| e.scene.node(*id)).cloned()?;
        let value = match req.kind.as_str() {
            "text" => pergel_core::StyleValue::Text(pergel_core::TypeStyle::of(n.text.as_deref()?)),
            "stroke" => pergel_core::StyleValue::Color { color: n.stroke?.0, paint: None },
            "effect" => pergel_core::StyleValue::Effect { effects: n.effects.clone() },
            _ => pergel_core::StyleValue::Color { color: n.fill, paint: n.paint.as_deref().cloned() },
        };
        let id = e.styles.set(req.name.trim(), value);
        e.page_rev += 1;
        let ids = e.editable();
        let styles = e.styles.clone();
        let kind = req.kind.clone();
        e.step(|e| {
            for i in &ids {
                let id = id.clone();
                let kind = kind.clone();
                e.scene.update(*i, |n| {
                    let mut links = n.styles.take().map(|b| *b).unwrap_or_default();
                    match kind.as_str() {
                        "text" => links.text = Some(id),
                        "stroke" => links.stroke = Some(id),
                        "effect" => links.effect = Some(id),
                        _ => links.fill = Some(id),
                    }
                    n.styles = Some(Box::new(links));
                });
            }
            e.scene.apply_styles(&styles);
        });
        Some(serde_json::Value::String(id).to_string())
    }).flatten())
}

/// Sets the interactions of a layer (`id`) or the selection (-1): JSON reactions in the buffer.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_reactions(id: i32) -> i32 {
    let Ok(list) = serde_json::from_str::<Vec<pergel_core::Reaction>>(&text()) else { return 0 };
    with_engine(|e| {
        let ids = if id >= 0 { vec![id as NodeId] } else { e.editable() };
        e.step(|e| {
            for i in &ids {
                let list = list.clone();
                e.scene.update(*i, |n| n.reactions = list);
            }
            ids.len() as i32
        })
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_guides_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.page_guides).unwrap_or_default()))
}

/// Replaces the shown page's ruler guides (JSON in the text buffer).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_guides_set() -> i32 {
    let Ok(list) = serde_json::from_str::<Vec<pergel_core::Guide>>(&text()) else { return 0 };
    with_engine(|e| {
        e.page_guides = list;
        e.page_rev += 1;
        1
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_show_grids(on: i32) {
    with_engine(|e| e.show_grids = on != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_show_connections(on: i32) {
    with_engine(|e| e.show_connections = on != 0);
}

/// The page's screens (top-level frames): `[{id, name, w, h}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_screens_json() -> *const c_char {
    emit(with_engine(|e| {
        let list: Vec<serde_json::Value> = e
            .scene
            .screens()
            .into_iter()
            .filter_map(|id| {
                e.scene.node(id).map(|n| {
                    serde_json::json!({ "id": id, "name": n.name, "w": n.bounds.w, "h": n.bounds.h, "scroll": n.scroll, "fill": hex(n.fill) })
                })
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

/// Presentation view: clears the variant changes made by "Change to" interactions and the
/// variables set by "Set variable".
#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_reset() {
    with_engine(|e| {
        e.present_swaps.clear();
        e.present_vars.clear();
    });
}

/// Presentation view: the variables' values now (`{id: value}`), with the prototype's changes.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_vars() -> *const c_char {
    emit(with_engine(|e| {
        let mut map = serde_json::Map::new();
        for v in &e.variables.variables {
            if let Some(value) = e.variables.resolve(&v.id) {
                map.insert(v.id.clone(), serde_json::json!(value));
            }
        }
        for (id, value) in &e.present_vars {
            map.insert(id.clone(), serde_json::json!(value));
        }
        serde_json::Value::Object(map).to_string()
    }))
}

/// Presentation view: sets a variable (`text()` holds `{id, value}`) for the rest of the run.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_set_var() {
    #[derive(Deserialize)]
    struct Req {
        id: String,
        value: pergel_core::VarValue,
    }
    let Ok(r) = serde_json::from_str::<Req>(&text()) else { return };
    with_engine(|e| {
        e.present_vars.retain(|(id, _)| *id != r.id);
        e.present_vars.push((r.id, r.value));
    });
}

/// Presentation view: shows `instance` as variant `variant` (u32::MAX shows its own again).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_swap(instance: u32, variant: u32) {
    with_engine(|e| {
        e.present_swaps.retain(|(i, _)| *i != instance);
        if variant != u32::MAX && e.scene.is_main(variant) && e.scene.is_instance(instance) {
            e.present_swaps.push((instance, variant));
        }
    });
}

thread_local! {
    /// Hotspots and content area of the last `pergel_present_capture`.
    static PRESENT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Presentation view: renders a screen with the current variant changes (the document itself is
/// left untouched) — its whole content when it scrolls — at `scale` pixels per unit. `width`,
/// `height`, then pixels; the
/// hotspots and area come from `pergel_present_info`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_capture(screen: u32, scale: f32) -> *const u8 {
    with_engine(|e| {
        let swaps = e.present_swaps.clone();
        let before = e.selection.clone();
        e.scene.begin(&before);
        // Variables the prototype set, for every mode.
        if !e.present_vars.is_empty() {
            let mut vars = e.variables.clone();
            for (id, value) in &e.present_vars {
                if let Some(v) = vars.variables.iter_mut().find(|v| v.id == *id) {
                    for x in v.values.values_mut() {
                        *x = value.clone();
                    }
                }
            }
            e.scene.apply_variables(&vars);
        }
        // Swapped instances get new ids; hotspots report the original instance.
        let mut origin: HashMap<NodeId, NodeId> = HashMap::new();
        for (inst, variant) in &swaps {
            if let Some(new) = e.scene.swap_instance(*inst, *variant) {
                origin.insert(new, *inst);
            }
        }
        e.settle();
        let result = (|| {
            let s = e.scene.node(screen)?;
            let scrolls = !s.scroll.is_none();
            let (sx, sy) = (s.bounds.x, s.bounds.y);
            let area = if scrolls { e.scene.scroll_content(screen)? } else { Rect::new(0.0, 0.0, s.bounds.w, s.bounds.h) };
            let max = (area.w.max(area.h) * scale.clamp(0.1, 4.0)).min(8192.0);
            let pixels = if scrolls {
                let mut tree = e.scene.extract(screen)?;
                tree.translate(-sx, -sy);
                tree.clip = false;
                let mut tmp = Scene::default();
                tmp.images = e.scene.images.clone();
                tmp.insert_tree(&tree, None, None);
                e.renderer.thumbnail(&tmp, area, max)?
            } else {
                e.renderer.thumbnail(&e.scene, Rect::new(sx, sy, area.w, area.h), max)?
            };
            let spots: Vec<serde_json::Value> = e
                .scene
                .hotspots(screen)
                .into_iter()
                .map(|h| {
                    // The instance a hotspot belongs to (for "Change to"), as the document knows it.
                    let mut cur = Some(h.id);
                    let mut instance = None;
                    while let Some(c) = cur {
                        if e.scene.is_instance(c) {
                            instance = Some(*origin.get(&c).unwrap_or(&c));
                            break;
                        }
                        cur = e.scene.node(c).and_then(|n| n.parent);
                    }
                    let mut v = serde_json::to_value(&h).unwrap_or_default();
                    v["instance"] = serde_json::json!(instance);
                    v
                })
                .collect();
            let info = serde_json::json!({ "area": area, "spots": spots }).to_string();
            Some((pixels, info))
        })();
        e.scene.rollback();
        e.relayout();
        let (pixels, info) = result?;
        PRESENT.with_borrow_mut(|p| *p = info);
        PIXELS.with_borrow_mut(|out| {
            *out = pixels;
            Some(out.as_ptr())
        })
    })
    .flatten()
    .unwrap_or(std::ptr::null())
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_present_info() -> *const c_char {
    emit(PRESENT.with_borrow(|p| (!p.is_empty()).then(|| p.clone())))
}

/// The variants of a layer's component set (for "Change to"): `[{id, name}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_variants_of(id: u32) -> *const c_char {
    emit(with_engine(|e| {
        let main = e.scene.main_of(id).or(Some(id).filter(|i| e.scene.is_main(*i)))?;
        let set = e.scene.variant_set_of(main)?;
        let list: Vec<serde_json::Value> = e
            .scene
            .node(set)?
            .children
            .iter()
            .filter_map(|c| e.scene.node(*c).filter(|n| e.scene.is_main(n.id)).map(|n| serde_json::json!({ "id": n.id, "name": n.name })))
            .collect();
        serde_json::to_string(&list).ok()
    }).flatten())
}

/// A scrolling screen's whole content, unclipped and without the layers around it (presentation
/// view): `width`, `height` (u32 each) then the pixels of the area from `pergel_scroll_area`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_scroll_capture(id: u32, max: u32) -> *const u8 {
    with_engine(|e| {
        let content = e.scene.scroll_content(id)?;
        let mut tree = e.scene.extract(id)?;
        let origin = (tree.bounds.x, tree.bounds.y);
        tree.translate(-origin.0, -origin.1);
        tree.clip = false;
        let mut tmp = Scene::default();
        tmp.images = e.scene.images.clone();
        tmp.insert_tree(&tree, None, None);
        let pixels = e.renderer.thumbnail(&tmp, content, max as f32)?;
        PIXELS.with_borrow_mut(|out| {
            *out = pixels;
            Some(out.as_ptr())
        })
    })
    .flatten()
    .unwrap_or(std::ptr::null())
}

/// The content area of a scrolling screen, relative to it: `{x, y, w, h}`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_scroll_area(id: u32) -> *const c_char {
    emit(with_engine(|e| e.scene.scroll_content(id).and_then(|r| serde_json::to_string(&r).ok())).flatten())
}

/// Interactive layers of a screen, relative to it (presentation view).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_hotspots_json(screen: u32) -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(&e.scene.hotspots(screen)).unwrap_or_default()))
}

/// The screen (top-level frame) a layer is in, or -1.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_screen_of(id: u32) -> i32 {
    with_engine(|e| e.scene.screen_of(id).filter(|s| e.scene.node(*s).is_some_and(|n| n.kind == Kind::Frame)).map_or(-1, |s| s as i32)).unwrap_or(-1)
}

#[derive(Serialize)]
struct PageInfo<'a> {
    name: &'a str,
    current: bool,
}

/// The document's pages in order.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pages_json() -> *const c_char {
    emit(with_engine(|e| {
        let list: Vec<PageInfo> = e.pages.iter().enumerate().map(|(i, p)| PageInfo { name: &p.name, current: i == e.page }).collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_switch(i: u32) -> i32 {
    with_engine(|e| e.switch_page(i as usize) as i32).unwrap_or(0)
}

/// Adds a page after the current one and shows it. Returns its index.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_add() -> i32 {
    with_engine(|e| {
        let mut view = Viewport::new(e.view.width, e.view.height);
        view.x = -e.view.width / 2.0;
        view.y = -e.view.height / 2.0;
        let at = e.page + 1;
        let name = e.page_name("Page");
        e.pages.insert(at, PageSlot::new(name, view));
        // The shown page's slot index moved only if the new page went before it (it didn't).
        e.page_rev += 1;
        e.switch_page(at);
        at as i32
    })
    .unwrap_or(-1)
}

/// Renames page `i` to the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_rename(i: u32) -> i32 {
    let name = text().trim().to_string();
    with_engine(|e| {
        let Some(p) = e.pages.get_mut(i as usize).filter(|_| !name.is_empty()) else { return 0 };
        p.name = name;
        e.page_rev += 1;
        1
    })
    .unwrap_or(0)
}

/// Deletes page `i` (never the last one).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_delete(i: u32) -> i32 {
    with_engine(|e| {
        let i = i as usize;
        if e.pages.len() <= 1 || i >= e.pages.len() {
            return 0;
        }
        if i == e.page {
            let next = if i + 1 < e.pages.len() { i + 1 } else { i - 1 };
            e.switch_page(next);
        }
        e.pages.remove(i);
        if e.page > i {
            e.page -= 1;
        }
        e.page_rev += 1;
        1
    })
    .unwrap_or(0)
}

/// Copies page `i` (layers, view) right after it and shows the copy.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_duplicate(i: u32) -> i32 {
    with_engine(|e| {
        let i = i as usize;
        let Some(src) = e.pages.get(i) else { return -1 };
        let (scene, view) = if i == e.page { (&e.scene, e.view) } else { (&src.scene, src.view) };
        let page = scene.to_page(&src.name, serde_json::Value::Null);
        let copy = Scene::from_trees(&page.layers);
        let name = format!("{} copy", src.name);
        let guides = if i == e.page { e.page_guides.clone() } else { src.guides.clone() };
        e.pages.insert(i + 1, PageSlot { name, scene: copy, view, selection: Vec::new(), guides });
        if e.page > i {
            e.page += 1;
        }
        e.page_rev += 1;
        e.switch_page(i + 1);
        (i + 1) as i32
    })
    .unwrap_or(-1)
}

/// Moves page `from` to position `to`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_page_move(from: u32, to: u32) -> i32 {
    with_engine(|e| {
        let (from, to) = (from as usize, to as usize);
        if from >= e.pages.len() || to >= e.pages.len() || from == to {
            return 0;
        }
        let current = e.page;
        let p = e.pages.remove(from);
        e.pages.insert(to, p);
        e.page = if current == from {
            to
        } else if from < current && to >= current {
            current - 1
        } else if from > current && to <= current {
            current + 1
        } else {
            current
        };
        e.page_rev += 1;
        1
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_fit() {
    with_engine(|e| e.view.fit(e.scene.bounds(), FIT_MARGIN));
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_fit_selection() {
    with_engine(|e| {
        if let Some(area) = e.selection_bounds() {
            e.view.fit(area, FIT_MARGIN);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_resize(width: i32, height: i32, dpr: f32) -> i32 {
    with_engine(|e| {
        let Some(surface) = make_surface(&mut e.context, width, height) else { return -4 };
        e.surface = surface;
        e.dpr = dpr;
        e.view.resize(width as f32 / dpr, height as f32 / dpr);
        1
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_pan(dx: f32, dy: f32) {
    with_engine(|e| e.view.pan(dx, dy));
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_zoom(factor: f32, x: f32, y: f32) {
    with_engine(|e| e.view.zoom_at(factor, x, y));
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_zoom_level() -> f32 {
    with_engine(|e| e.view.zoom).unwrap_or(1.0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_render() -> u32 {
    with_engine(|e| {
        let connections: Vec<((f32, f32), (f32, f32))> = if e.show_connections {
            e.scene
                .reactions()
                .into_iter()
                .filter_map(|(id, r)| {
                    let pergel_core::Action::Navigate { to } = r.action else { return None };
                    let (from, to) = (e.scene.node(id)?.aabb(), e.scene.node(to)?.aabb());
                    Some(((from.right(), from.y + from.h / 2.0), (to.x, to.y + to.h / 2.0)))
                })
                .collect()
        } else {
            Vec::new()
        };
        let page_guides: Vec<(bool, f32)> = e.page_guides.iter().map(|g| (g.vertical, g.at)).collect();
        let notes: Vec<(f32, f32)> = e
            .scene
            .visible(&e.view.world_area())
            .filter(|n| n.note.is_some() && e.scene.reach(n.id).is_some())
            .map(|n| {
                let b = n.aabb();
                (b.right(), b.y)
            })
            .collect();
        let slices: Vec<Rect> = e.scene.visible(&e.view.world_area()).filter(|n| n.slice && n.visible).map(|n| n.aabb()).collect();
        let overlay = Overlay {
            slices: &slices,
            notes: &notes,
            page_guides: &page_guides,
            layout_grids: e.show_grids,
            guides: &e.guides,
            connections: &connections,
            selection: &e.selection,
            hover: e.hover,
            marquee: e.marquee,
            editing: e.editing,
            hide_selection: false,
            vector: e.vedit.as_ref().filter(|v| v.pencil.is_none()).map(|v| pergel_render::VectorOverlay {
                paths: &v.paths,
                selected: &v.selected,
                rubber: v.rubber.filter(|_| v.pen).and_then(|r| {
                    let last = v.paths.last()?.anchors.last()?;
                    Some((last.p, r))
                }),
            }),
        };
        let stats = e.renderer.draw(e.surface.canvas(), &e.scene, &e.view, e.dpr, &overlay);
        e.context.flush_and_submit();
        stats.drawn
    })
    .unwrap_or(0)
}

/// What a click at this point selects (`deep` = Ctrl-click); -1 means start a marquee.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pick(sx: f32, sy: f32, deep: i32) -> i32 {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        e.scene.pick(x, y, deep != 0, &e.selection).map_or(-1, |id| id as i32)
    })
    .unwrap_or(-1)
}

/// Double-click: the layer one level below the selection under the point.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_dive(sx: f32, sy: f32) -> i32 {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        e.scene.dive(x, y, &e.selection).map_or(-1, |id| id as i32)
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_hover(id: i32) {
    with_engine(|e| e.hover = (id >= 0).then_some(id as NodeId));
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_select(id: i32, additive: i32) {
    with_engine(|e| {
        if id < 0 || e.scene.node(id as NodeId).is_none() {
            if additive == 0 {
                e.selection.clear();
            }
            return;
        }
        let id = id as NodeId;
        if additive == 0 {
            e.selection = vec![id];
        } else if let Some(pos) = e.selection.iter().position(|s| *s == id) {
            e.selection.remove(pos);
        } else {
            // A layer and its own ancestors or descendants are never selected together.
            let scene = &e.scene;
            e.selection.retain(|s| !scene.is_ancestor(*s, id) && !scene.is_ancestor(id, *s));
            e.selection.push(id);
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_is_selected(id: i32) -> i32 {
    with_engine(|e| (id >= 0 && e.selection.contains(&(id as NodeId))) as i32).unwrap_or(0)
}

/// 0 parent (Shift+Enter), 1 children (Enter), 2 next sibling (Tab), 3 previous sibling
/// (Shift+Tab), 4 every sibling of the selection, or every top-level layer (Ctrl+A).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_select_relative(which: u32) {
    with_engine(|e| {
        let sel = e.scene.outermost(&e.selection);
        let scene = &e.scene;
        let pickable = |ids: &[NodeId]| -> Vec<NodeId> {
            ids.iter().copied().filter(|c| scene.reach(*c).is_some() && !scene.is_locked(*c)).collect()
        };
        let next: Vec<NodeId> = match which {
            0 => {
                let mut parents: Vec<NodeId> = sel.iter().filter_map(|id| scene.node(*id)?.parent).collect();
                parents.sort_unstable();
                parents.dedup();
                parents
            }
            1 => sel.iter().flat_map(|id| pickable(scene.children(Some(*id)))).collect(),
            2 | 3 => sel
                .iter()
                .filter_map(|id| {
                    let (parent, i) = scene.position(*id)?;
                    let list = scene.children(parent);
                    let j = if which == 2 { (i + 1) % list.len() } else { (i + list.len() - 1) % list.len() };
                    Some(list[j])
                })
                .collect(),
            _ => {
                let parent = sel.first().and_then(|id| scene.node(*id)?.parent);
                pickable(scene.children(parent))
            }
        };
        if !next.is_empty() {
            e.selection = next;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_marquee(sx: f32, sy: f32, sw: f32, sh: f32) {
    with_engine(|e| {
        e.marquee = (sw != 0.0 || sh != 0.0).then(|| Rect::new(sx.min(sx + sw), sy.min(sy + sh), sw.abs(), sh.abs()));
    });
}

/// Marquee from (sx, sy): selects inside a top-level frame when it starts on that frame's background.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_select_area(sx: f32, sy: f32, sw: f32, sh: f32, additive: i32) {
    with_engine(|e| {
        let area = screen_to_world(&e.view, sx, sy, sw, sh);
        let (x, y) = e.world(sx, sy);
        let hits = e.scene.select_area(&area, e.scene.marquee_scope(x, y));
        if additive == 0 {
            e.selection.clear();
        }
        for id in hits {
            if !e.selection.contains(&id) {
                e.selection.push(id);
            }
        }
        e.marquee = None;
    });
}

/// Opens an undo step spanning several calls (a drag); close it with `pergel_commit`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_begin() {
    with_engine(|e| {
        let selection = e.selection.clone();
        e.scene.begin(&selection);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_commit() -> i32 {
    with_engine(|e| {
        e.settle();
        e.prune_selection();
        let selection = e.selection.clone();
        e.scene.commit(&selection) as i32
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_undo() -> i32 {
    with_engine(|e| {
        let selection = e.scene.undo();
        e.restore_selection(selection)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_redo() -> i32 {
    with_engine(|e| {
        let selection = e.scene.redo();
        e.restore_selection(selection)
    })
    .unwrap_or(0)
}

/// Drags the selection by a screen-space delta.
/// Starts dragging the selection: remembers where it was and what it can snap to.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_move_start() {
    with_engine(|e| {
        let ids = e.movable();
        let Some(start) = Rect::union_all(ids.iter().filter_map(|id| e.scene.node(*id)).map(|n| n.aabb())) else { return };
        let moving: std::collections::HashSet<NodeId> = ids.iter().flat_map(|id| e.scene.subtree(*id)).collect();
        let area = e.view.world_area();
        let mut xs = Vec::new();
        let mut ys = Vec::new();
        for n in e.scene.visible(&area).take(4000) {
            if moving.contains(&n.id) || e.scene.reach(n.id).is_none() {
                continue;
            }
            let b = n.aabb();
            for x in [b.x, b.x + b.w / 2.0, b.right()] {
                xs.push((x, b));
            }
            for y in [b.y, b.y + b.h / 2.0, b.bottom()] {
                ys.push((y, b));
            }
            // Layout guide columns and rows are snap lines too (ruler guides are added below).
            if e.show_grids && n.kind == Kind::Frame && n.rotation == 0.0 {
                for g in n.grids.iter().filter(|g| g.visible) {
                    match g.kind {
                        pergel_core::GridKind::Columns => {
                            for (s, w) in g.bands(b.w) {
                                xs.push((b.x + s, b));
                                xs.push((b.x + s + w, b));
                            }
                        }
                        pergel_core::GridKind::Rows => {
                            for (s, h) in g.bands(b.h) {
                                ys.push((b.y + s, b));
                                ys.push((b.y + s + h, b));
                            }
                        }
                        pergel_core::GridKind::Grid => {}
                    }
                }
            }
        }
        for g in &e.page_guides {
            let line = if g.vertical { Rect::new(g.at, start.y, 0.0, start.h) } else { Rect::new(start.x, g.at, start.w, 0.0) };
            if g.vertical {
                xs.push((g.at, line));
            } else {
                ys.push((g.at, line));
            }
        }
        e.moving = Some(MoveSession { ids, start, applied: (0.0, 0.0), xs, ys });
    });
}

/// Drags the selection by (dx, dy) screen pixels from where the drag started, snapping its
/// edges and centre to other layers (unless `snap` is 0). `axis` 1 locks to the larger axis.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_move_total(dx: f32, dy: f32, snap: i32, axis: i32) {
    with_engine(|e| {
        let zoom = e.view.zoom;
        let Some(m) = e.moving.as_mut() else { return };
        let (mut ox, mut oy) = (dx / zoom, dy / zoom);
        if axis != 0 {
            if ox.abs() > oy.abs() {
                oy = 0.0;
            } else {
                ox = 0.0;
            }
        }
        let mut guides = Vec::new();
        if snap != 0 {
            let tol = 5.0 / zoom;
            let b = m.start.translated(ox, oy);
            let best = |lines: &[(f32, Rect)], mine: [f32; 3]| -> Option<(f32, f32, Rect)> {
                let mut best: Option<(f32, f32, Rect)> = None;
                for &(v, r) in lines {
                    for &own in &mine {
                        let d = v - own;
                        if d.abs() <= tol && best.is_none_or(|(bd, _, _)| d.abs() < bd.abs()) {
                            best = Some((d, v, r));
                        }
                    }
                }
                best
            };
            if let Some((d, x, r)) = best(&m.xs, [b.x, b.x + b.w / 2.0, b.right()]) {
                ox += d;
                let nb = m.start.translated(ox, oy);
                guides.push(((x, nb.y.min(r.y)), (x, nb.bottom().max(r.bottom()))));
            }
            if let Some((d, y, r)) = best(&m.ys, [b.y, b.y + b.h / 2.0, b.bottom()]) {
                oy += d;
                let nb = m.start.translated(ox, oy);
                guides.push(((nb.x.min(r.x), y), (nb.right().max(r.right()), y)));
            }
        }
        let (sx, sy) = (ox - m.applied.0, oy - m.applied.1);
        m.applied = (ox, oy);
        let ids = m.ids.clone();
        for id in ids {
            e.scene.translate(id, sx, sy);
        }
        e.guides = guides;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_move_end() {
    with_engine(|e| {
        e.moving = None;
        e.guides.clear();
    });
}

/// Find and replace in text layers on the shown page: `{"find", "replace", "matchCase", "dryRun"}`.
/// Returns `{"layers": n, "matches": m}`; with `dryRun` nothing changes (and the matches are selected).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_find_replace() -> *const c_char {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Req {
        find: String,
        #[serde(default)]
        replace: String,
        #[serde(default)]
        match_case: bool,
        #[serde(default)]
        dry_run: bool,
    }
    let req = serde_json::from_str::<Req>(&text()).ok();
    emit(with_engine(|e| {
        let req = req.filter(|r| !r.find.is_empty())?;
        let find_lower = req.find.to_lowercase();
        let count = |s: &str| if req.match_case { s.matches(&req.find).count() } else { s.to_lowercase().matches(&find_lower).count() };
        let hits: Vec<(NodeId, usize)> = e
            .scene
            .nodes()
            .filter(|n| !e.scene.in_instance(n.id))
            .filter_map(|n| n.text.as_deref().map(|t| (n.id, count(&t.content))))
            .filter(|(_, c)| *c > 0)
            .collect();
        let matches: usize = hits.iter().map(|h| h.1).sum();
        if req.dry_run {
            e.selection = hits.iter().map(|h| h.0).collect();
        } else if !hits.is_empty() {
            e.step(|e| {
                for (id, _) in &hits {
                    e.scene.update(*id, |n| {
                        if let Some(t) = n.text.as_mut() {
                            let new = if req.match_case {
                                t.content.replace(&req.find, &req.replace)
                            } else {
                                // Case-insensitive: walk the lowercase copy to find the spans.
                                let lower = t.content.to_lowercase();
                                let mut out = String::new();
                                let mut last = 0;
                                for (i, _) in lower.match_indices(&find_lower) {
                                    if i < last || !t.content.is_char_boundary(i) || !t.content.is_char_boundary(i + req.find.len()) {
                                        continue;
                                    }
                                    out.push_str(&t.content[last..i]);
                                    out.push_str(&req.replace);
                                    last = i + req.find.len();
                                }
                                out.push_str(&t.content[last..]);
                                out
                            };
                            t.set_content(new);
                            if t.auto_name {
                                n.name = t.layer_name();
                            }
                        }
                    });
                    e.fit_text(*id);
                }
                e.selection = hits.iter().map(|h| h.0).collect();
            });
        }
        Some(serde_json::json!({ "layers": hits.len(), "matches": matches }).to_string())
    }).flatten())
}

/// "Select inverse": the other layers next to the selected ones.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_select_inverse() -> u32 {
    with_engine(|e| {
        let parent = e.selection.first().and_then(|id| e.scene.node(*id)).map(|n| n.parent).unwrap_or(None);
        let siblings: Vec<NodeId> = e.scene.children(parent).to_vec();
        let chosen: Vec<NodeId> = siblings.into_iter().filter(|id| !e.selection.contains(id) && !e.scene.is_locked(*id)).collect();
        e.selection = chosen;
        e.selection.len() as u32
    })
    .unwrap_or(0)
}

/// "Round to pixel": positions and sizes of the selection to whole pixels.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_round_to_pixel() -> u32 {
    with_engine(|e| {
        let ids = e.editable();
        e.step(|e| {
            for id in &ids {
                if let Some(b) = e.scene.node(*id).map(|n| n.bounds) {
                    e.scene.set_bounds(*id, Rect::new(b.x.round(), b.y.round(), b.w.round().max(1.0), b.h.round().max(1.0)));
                }
            }
            ids.len() as u32
        })
    })
    .unwrap_or(0)
}

/// N / Shift+N: selects the next (`dir` 1) or previous (-1) top-level frame and zooms to it.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_next_frame(dir: i32) -> i32 {
    with_engine(|e| {
        let screens = e.scene.screens();
        if screens.is_empty() {
            return -1;
        }
        let cur = e.selection.first().and_then(|id| e.scene.screen_of(*id)).and_then(|s| screens.iter().position(|x| *x == s));
        let n = screens.len() as i32;
        let next = match cur {
            Some(i) => ((i as i32 + dir).rem_euclid(n)) as usize,
            None if dir >= 0 => 0,
            None => screens.len() - 1,
        };
        let id = screens[next];
        e.selection = vec![id];
        if let Some(b) = e.scene.node(id).map(|n| n.aabb()) {
            e.view.fit(b, FIT_MARGIN);
        }
        id as i32
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_move_selection(dx: f32, dy: f32) {
    with_engine(|e| {
        let (wx, wy) = (dx / e.view.zoom, dy / e.view.zoom);
        for id in e.movable() {
            e.scene.translate(id, wx, wy);
        }
    });
}

/// Ends a drag at (sx, sy): layers dropped over a frame move into it, layers dragged out of
/// one land on the canvas; layers inside groups stay put. Positions snap to whole pixels.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_drop(sx: f32, sy: f32) {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        let moving = e.movable();
        let target = e.scene.drop_target(x, y, &moving);
        let mut at = target.and_then(|t| e.flow_index(t, x, y, &moving));
        for id in &moving {
            let Some(node) = e.scene.node(*id) else { continue };
            let (parent, absolute) = (node.parent, node.absolute);
            let in_group = parent.and_then(|p| e.scene.node(p)).is_some_and(|p| p.kind == Kind::Group);
            if in_group {
                continue;
            }
            match at {
                // Dropped into an auto-layout frame: take the slot under the pointer.
                Some(index) if !absolute => {
                    e.scene.reparent(*id, target, Some(index));
                    at = e.scene.position(*id).map(|(_, i)| i + 1);
                }
                _ if parent != target => {
                    e.scene.reparent(*id, target, None);
                }
                _ => {}
            }
        }
        e.round_positions(&moving);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_nudge(dx: f32, dy: f32) {
    with_engine(|e| {
        e.step(|e| {
            let moving = e.movable();
            for id in &moving {
                e.scene.translate(*id, dx, dy);
            }
            e.round_positions(&moving);
        })
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_create(kind: u32, sx: f32, sy: f32, sw: f32, sh: f32) -> i32 {
    if (3..=6).contains(&kind) {
        return create_vector(kind, sx, sy, sw, sh);
    }
    let section = kind == 7;
    let slice = kind == 8;
    with_engine(|e| {
        let kind = match kind {
            0 | 7 => Kind::Frame,
            2 => Kind::Ellipse,
            _ => Kind::Rectangle,
        };
        let mut r = screen_to_world(&e.view, sx, sy, sw, sh);
        if r.w < 2.0 && r.h < 2.0 {
            r.w = 100.0;
            r.h = 100.0;
        }
        let r = Rect::new(r.x.round(), r.y.round(), r.w.round().max(1.0), r.h.round().max(1.0));
        let (x, y) = e.world(sx, sy);
        e.step(|e| {
            let parent = e.scene.drop_target(x, y, &[]);
            let fill = if kind == Kind::Frame { WHITE } else { DEFAULT_FILL };
            let name = if section {
                e.scene.next_name_for("Section")
            } else if slice {
                e.scene.next_name_for("Slice")
            } else {
                e.scene.next_name(kind)
            };
            let mut node = Node::new(kind, name, r, fill);
            if slice {
                node.slice = true;
                node.fill_on = false;
                node.exports = vec![pergel_core::ExportSetting::default()];
            }
            if section {
                node.section = true;
                node.clip = false;
                node.radius = 2.0;
                node.stroke = Some((Color::rgba(0, 0, 0, 26), 1.0));
            }
            let id = e.scene.insert(node, parent, None);
            if kind == Kind::Frame {
                // A frame drawn around existing layers takes them in, as in Figma.
                let inside: Vec<NodeId> = e
                    .scene
                    .children(parent)
                    .iter()
                    .copied()
                    .filter(|c| *c != id && e.scene.node(*c).is_some_and(|n| r.contains(&n.bounds)))
                    .collect();
                for c in inside {
                    e.scene.reparent(c, Some(id), None);
                }
            }
            e.selection = vec![id];
            id as i32
        })
    })
    .unwrap_or(-1)
}

/// A Skia path in world coordinates as a vector layer's outline: (box, path data local to it).
fn path_to_vector(path: &skia_safe::Path) -> Option<(Rect, pergel_core::VectorPath)> {
    let b = path.compute_tight_bounds();
    if b.is_empty() && b.width() == 0.0 && b.height() == 0.0 {
        return None;
    }
    let local = path.make_transform(&skia_safe::Matrix::translate((-b.left, -b.top)));
    let d = pergel_core::map_path(&local.to_svg(), |x, y| (x, y));
    let even_odd = matches!(local.fill_type(), skia_safe::PathFillType::EvenOdd | skia_safe::PathFillType::InverseEvenOdd);
    Some((
        Rect::new(b.left, b.top, b.width(), b.height()),
        pergel_core::VectorPath { d, size: (b.width(), b.height()), even_odd, cap: pergel_core::Cap::Butt, join: pergel_core::Join::Miter, start: Default::default(), end: Default::default() },
    ))
}

impl Engine {
    /// Layers in painting order (bottom first).
    fn paint_order(&self, ids: &[NodeId]) -> Vec<NodeId> {
        let mut order = HashMap::new();
        let mut stack: Vec<NodeId> = self.scene.roots().iter().rev().copied().collect();
        let mut k = 0usize;
        while let Some(id) = stack.pop() {
            order.insert(id, k);
            k += 1;
            if let Some(n) = self.scene.node(id) {
                stack.extend(n.children.iter().rev());
            }
        }
        let mut out = ids.to_vec();
        out.sort_by_key(|id| order.get(id).copied().unwrap_or(usize::MAX));
        out
    }

    /// A layer's filled outline in world coordinates; containers merge their visible shapes.
    fn outline_of(&self, id: NodeId) -> Option<skia_safe::Path> {
        let n = self.scene.node(id)?;
        if !n.visible {
            return None;
        }
        if n.kind.is_container() {
            let mut acc: Option<skia_safe::Path> = if n.kind == Kind::Frame && n.fill_on { Some(pergel_render::world_path(n)) } else { None };
            for c in self.paint_order(&n.children) {
                if let Some(p) = self.outline_of(c) {
                    acc = Some(match acc {
                        Some(a) => a.op(&p, skia_safe::PathOp::Union).unwrap_or(a),
                        None => p,
                    });
                }
            }
            return acc;
        }
        if n.kind == Kind::Text {
            return None;
        }
        let fill = n.fill_on.then(|| pergel_render::world_path(n));
        let stroke = pergel_render::stroke_outline(n);
        match (fill, stroke) {
            (Some(f), Some(s)) => f.op(&s, skia_safe::PathOp::Union).or(Some(f)),
            (Some(f), None) => Some(f),
            (None, s) => s,
        }
    }

    /// Union (0), subtract (1), intersect (2), exclude (3) or flatten (4) the selection into one
    /// vector layer, placed where the topmost selected layer was. Returns its id.
    fn boolean(&mut self, op: u32) -> Option<NodeId> {
        let ids = self.paint_order(&self.scene.outermost(&self.editable()));
        if ids.is_empty() || (op < 4 && ids.len() < 2) {
            return None;
        }
        let paths: Vec<skia_safe::Path> = ids.iter().filter_map(|id| self.outline_of(*id)).collect();
        let (first, rest) = paths.split_first()?;
        let path = match op {
            1 => {
                let mut cut = rest.first()?.clone();
                for p in &rest[1..] {
                    cut = cut.op(&p, skia_safe::PathOp::Union).unwrap_or(cut);
                }
                first.op(&cut, skia_safe::PathOp::Difference)?
            }
            _ => {
                let pop = match op {
                    2 => skia_safe::PathOp::Intersect,
                    3 => skia_safe::PathOp::XOR,
                    _ => skia_safe::PathOp::Union,
                };
                let mut acc = first.clone();
                for p in rest {
                    acc = acc.op(&p, pop).unwrap_or(acc);
                }
                acc
            }
        };
        let (bounds, vector) = path_to_vector(&path)?;
        // Style: the base layer for Subtract, the top layer otherwise (its first painted shape).
        let style_from = if op == 1 { ids[0] } else { *ids.last()? };
        let leaf = self.scene.subtree(style_from).into_iter().find(|i| self.scene.node(*i).is_some_and(|n| !n.kind.is_container() && n.kind != Kind::Text)).unwrap_or(style_from);
        let style = self.scene.node(leaf)?.clone();
        let top = *ids.last()?;
        let (parent, index) = self.scene.position(top)?;
        let name = match op {
            0 => "Union".to_string(),
            1 => "Subtract".to_string(),
            2 => "Intersect".to_string(),
            3 => "Exclude".to_string(),
            _ => style.name.clone(),
        };
        // A stroke-only shape (a line) keeps its stroke colour as the outline's fill.
        let stroke_only = !style.fill_on && style.stroke.is_some();
        let fill = if stroke_only { style.stroke.map_or(style.fill, |s| s.0) } else { style.fill };
        let mut node = Node::new(Kind::Vector, name, bounds, fill);
        node.vector = Some(Box::new(vector));
        node.fill_on = true;
        node.paint = if stroke_only { None } else { style.paint.clone() };
        node.effects = style.effects.clone();
        node.opacity = style.opacity;
        node.blend = style.blend;
        let id = self.step(|e| {
            let id = e.scene.insert(node, parent, Some(index + 1));
            for old in &ids {
                e.scene.remove(*old);
            }
            e.selection = vec![id];
            id
        });
        Some(id)
    }

    /// "Outline stroke": each selected stroke becomes a filled vector of its area.
    fn outline_strokes(&mut self) -> u32 {
        let ids: Vec<NodeId> = self.scene.outermost(&self.editable());
        let jobs: Vec<(NodeId, skia_safe::Path, Color, bool)> = ids
            .iter()
            .filter_map(|id| {
                let n = self.scene.node(*id)?;
                if n.kind.is_container() || n.kind == Kind::Text {
                    return None;
                }
                let path = pergel_render::stroke_outline(n)?;
                Some((*id, path, n.stroke?.0, n.fill_on))
            })
            .collect();
        if jobs.is_empty() {
            return 0;
        }
        self.step(|e| {
            let mut selection = Vec::new();
            for (id, path, color, keep_fill) in &jobs {
                let Some((bounds, vector)) = path_to_vector(path) else { continue };
                let Some((parent, index)) = e.scene.position(*id) else { continue };
                let name = e.scene.node(*id).map_or("Vector".to_string(), |n| n.name.clone());
                let mut node = Node::new(Kind::Vector, name, bounds, *color);
                node.vector = Some(Box::new(vector));
                let outline = e.scene.insert(node, parent, Some(index + 1));
                if *keep_fill {
                    // The fill stays as its own layer under the outlined stroke.
                    e.scene.update(*id, |n| n.stroke = None);
                    selection.push(*id);
                } else {
                    e.scene.remove(*id);
                }
                selection.push(outline);
            }
            e.selection = selection;
            jobs.len() as u32
        })
    }
}

/// Offset path: a new vector layer `d` outside (or, negative, inside) each selected shape, with
/// `join` 0 miter, 1 round, 2 bevel. Returns how many layers were made.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_offset_path(d: f32, join: u32) -> u32 {
    with_engine(|e| {
        if d.abs() < 1e-3 {
            return 0;
        }
        let ids: Vec<NodeId> = e.scene.outermost(&e.editable());
        let jobs: Vec<(NodeId, skia_safe::Path, Color)> = ids
            .iter()
            .filter_map(|id| {
                let n = e.scene.node(*id)?;
                if n.kind == Kind::Text || n.kind == Kind::Group {
                    return None;
                }
                let shape = pergel_render::world_path(n);
                let mut paint = skia_safe::Paint::default();
                paint.set_style(skia_safe::PaintStyle::Stroke);
                paint.set_stroke_width(d.abs() * 2.0);
                paint.set_stroke_join(match join {
                    1 => skia_safe::PaintJoin::Round,
                    2 => skia_safe::PaintJoin::Bevel,
                    _ => skia_safe::PaintJoin::Miter,
                });
                paint.set_stroke_miter(8.0);
                let mut b = skia_safe::PathBuilder::new();
                if !skia_safe::path_utils::fill_path_with_paint(&shape, &paint, &mut b, None, None) {
                    return None;
                }
                let ring = b.detach();
                let out = if d > 0.0 { shape.op(&ring, skia_safe::PathOp::Union)? } else { shape.op(&ring, skia_safe::PathOp::Difference)? };
                let color = if n.fill_on { n.fill } else { n.stroke.map_or(n.fill, |s| s.0) };
                Some((*id, out, color))
            })
            .collect();
        if jobs.is_empty() {
            return 0;
        }
        e.step(|e| {
            let mut selection = Vec::new();
            for (id, path, color) in &jobs {
                let Some((bounds, vector)) = path_to_vector(path) else { continue };
                let Some((parent, index)) = e.scene.position(*id) else { continue };
                let name = e.scene.node(*id).map_or("Offset".to_string(), |n| format!("{} offset", n.name));
                let mut node = Node::new(Kind::Vector, name, bounds, *color);
                node.vector = Some(Box::new(vector));
                // An outer offset goes behind the shape, an inner one in front.
                let at = if d > 0.0 { index } else { index + 1 };
                selection.push(e.scene.insert(node, parent, Some(at)));
            }
            e.selection = selection;
            jobs.len() as u32
        })
    })
    .unwrap_or(0)
}

/// Simplify vertices: refits each selected vector with fewer, smooth anchors (`tolerance` in
/// pixels). Returns how many layers changed.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_simplify(tolerance: f32) -> u32 {
    with_engine(|e| {
        let ids: Vec<NodeId> = e
            .editable()
            .into_iter()
            .filter(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Vector && n.vector.is_some()))
            .collect();
        let tol = tolerance.max(0.1);
        let jobs: Vec<(NodeId, String)> = ids
            .iter()
            .filter_map(|id| {
                let n = e.scene.node(*id)?;
                let v = n.vector.as_deref()?;
                let path = skia_safe::Path::from_svg(v.placed(n.bounds.x, n.bounds.y, n.bounds.w, n.bounds.h))?;
                let mut subs = Vec::new();
                let mut it = skia_safe::ContourMeasureIter::new(&path, false, 1.0);
                while let Some(c) = it.next() {
                    let len = c.length();
                    let steps = ((len / (tol * 0.75)).ceil() as usize).clamp(2, 4000);
                    let pts: Vec<(f32, f32)> = (0..=steps)
                        .filter_map(|i| c.pos_tan(len * i as f32 / steps as f32).map(|(p, _)| (p.x, p.y)))
                        .collect();
                    let mut sp = pergel_core::smooth_stroke(&pts, tol);
                    if c.is_closed() {
                        if sp.anchors.len() > 2 {
                            sp.anchors.pop();
                        }
                        sp.closed = true;
                    }
                    subs.push(sp);
                }
                (!subs.is_empty()).then(|| (*id, pergel_core::to_path(&subs)))
            })
            .collect();
        if jobs.is_empty() {
            return 0;
        }
        e.step(|e| {
            for (id, d) in &jobs {
                let Some(path) = skia_safe::Path::from_svg(d) else { continue };
                let b = path.compute_tight_bounds();
                let bounds = Rect::new(b.left, b.top, b.width(), b.height());
                let local = pergel_core::map_path(d, |x, y| (x - b.left, y - b.top));
                e.scene.update(*id, |n| {
                    if let Some(v) = n.vector.as_deref_mut() {
                        v.d = local.clone();
                        v.size = (bounds.w, bounds.h);
                    }
                });
                e.scene.set_bounds(*id, bounds);
            }
            jobs.len() as u32
        })
    })
    .unwrap_or(0)
}

/// "Use as mask" (Ctrl+Alt+M): one layer becomes a mask; several are grouped with the
/// bottom one as the mask. Toggles off when every selected layer already is a mask.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_use_as_mask() -> i32 {
    with_engine(|e| {
        let ids = e.paint_order(&e.scene.outermost(&e.editable()));
        if ids.is_empty() {
            return 0;
        }
        let all_masks = ids.iter().all(|id| e.scene.node(*id).is_some_and(|n| n.mask));
        e.step(|e| {
            if all_masks {
                for id in &ids {
                    e.scene.update(*id, |n| n.mask = false);
                }
                return 1;
            }
            let mask = ids[0];
            if ids.len() > 1
                && let Some(g) = e.scene.group(&ids, Kind::Group)
            {
                e.scene.update(g, |n| n.name = "Mask group".into());
                e.selection = vec![g];
            }
            e.scene.update(mask, |n| n.mask = true);
            1
        })
    })
    .unwrap_or(0)
}

/// "Select matching layers": every layer on the page that looks like the selected one (same
/// type, size, fill, stroke and radius; instances of the same main).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_select_matching() -> u32 {
    with_engine(|e| {
        let Some(model) = e.selection.first().and_then(|id| e.scene.node(*id)).cloned() else { return 0 };
        let same = |n: &Node| -> bool {
            if let (Some(Component::Instance(a)), Some(Component::Instance(b))) = (n.component, model.component) {
                return a == b;
            }
            n.kind == model.kind
                && (n.bounds.w - model.bounds.w).abs() < 0.5
                && (n.bounds.h - model.bounds.h).abs() < 0.5
                && n.fill == model.fill
                && n.fill_on == model.fill_on
                && n.paint == model.paint
                && n.stroke == model.stroke
                && n.radii() == model.radii()
                && n.text.as_ref().map(|t| (t.family.clone(), t.weight, t.size.to_bits())) == model.text.as_ref().map(|t| (t.family.clone(), t.weight, t.size.to_bits()))
        };
        let ids: Vec<NodeId> = e.scene.nodes().filter(|n| n.visible && same(n)).map(|n| n.id).collect();
        e.selection = ids;
        e.selection.len() as u32
    })
    .unwrap_or(0)
}

/// "Combine as variants": the selected main components become a component set.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_combine_variants() -> i32 {
    with_engine(|e| {
        let ids = e.editable();
        e.step(|e| {
            let set = e.scene.combine_as_variants(&ids);
            if let Some(s) = set {
                e.selection = vec![s];
            }
            set.map_or(-1, |s| s as i32)
        })
    })
    .unwrap_or(-1)
}

/// Switches the selected instances' variant property (`{"property": …, "value": …}` in the
/// text buffer). Returns how many instances changed.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_variant() -> u32 {
    #[derive(Deserialize)]
    struct Req {
        property: String,
        value: String,
    }
    let Ok(req) = serde_json::from_str::<Req>(&text()) else { return 0 };
    with_engine(|e| e.set_variant(&e.selection.clone(), &req.property, &req.value)).unwrap_or(0)
}

impl Engine {
    fn set_variant(&mut self, ids: &[NodeId], property: &str, value: &str) -> u32 {
        let jobs: Vec<(NodeId, NodeId)> = ids
            .iter()
            .filter_map(|id| {
                let main = self.scene.main_of(*id)?;
                let set = self.scene.variant_set_of(main)?;
                let mut props = pergel_core::parse_variant(&self.scene.node(main)?.name);
                match props.iter_mut().find(|(k, _)| k == property) {
                    Some(kv) => kv.1 = value.to_string(),
                    None => props.push((property.to_string(), value.to_string())),
                }
                let target = self.scene.find_variant(set, &props)?;
                Some((*id, target))
            })
            .collect();
        if jobs.is_empty() {
            return 0;
        }
        self.step(|e| {
            let mut sel = Vec::new();
            for (inst, target) in &jobs {
                if let Some(new) = e.scene.swap_instance(*inst, *target) {
                    sel.push(new);
                }
            }
            e.selection = sel;
            jobs.len() as u32
        })
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_boolean(op: u32) -> i32 {
    with_engine(|e| e.boolean(op).map_or(-1, |id| id as i32)).unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_outline_stroke() -> u32 {
    with_engine(|e| e.outline_strokes()).unwrap_or(0)
}

impl Engine {
    /// Enters vector edit mode on a vector (shapes are turned into vectors first). One undo
    /// step lasts until `vector_end`.
    fn vector_begin(&mut self, id: NodeId) -> bool {
        if self.vedit.is_some() {
            self.vector_end();
        }
        let Some(n) = self.scene.node(id).cloned() else { return false };
        if self.scene.in_instance(id) || n.kind.is_container() || n.kind == Kind::Text {
            return false;
        }
        let before = self.selection.clone();
        self.scene.begin(&before);
        if n.kind != Kind::Vector {
            // "Edit object" on a rectangle, ellipse…: it becomes a vector of the same outline.
            let Some((bounds, vector)) = path_to_vector(&pergel_render::world_path(&n)) else {
                self.scene.commit(&before);
                return false;
            };
            self.scene.update(id, |m| {
                m.kind = Kind::Vector;
                m.vector = Some(Box::new(vector));
                m.rotation = 0.0;
                m.radius = 0.0;
                m.corners = None;
            });
            self.scene.set_bounds(id, bounds);
        }
        let Some(n) = self.scene.node(id) else { return false };
        let Some(v) = n.vector.as_deref() else { return false };
        let (sx, sy) = v.scale_to(n.bounds.w, n.bounds.h);
        let (bx, by) = (n.bounds.x, n.bounds.y);
        let mut paths = pergel_core::parse_path(&v.d);
        pergel_core::map_points(&mut paths, |(x, y)| (bx + x * sx, by + y * sy));
        self.selection = vec![id];
        self.vedit = Some(VectorEdit { node: id, paths, selected: Vec::new(), drag: None, pen: false, pencil: None, rubber: None });
        true
    }

    /// Writes the edited outline back into the layer (its box hugs the outline).
    fn vector_write(&mut self) {
        let Some(v) = &self.vedit else { return };
        let id = v.node;
        let d = pergel_core::to_path(&v.paths);
        let Some(path) = skia_safe::Path::from_svg(&d) else { return };
        let b = path.compute_tight_bounds();
        let bounds = Rect::new(b.left, b.top, b.width(), b.height());
        let local = pergel_core::map_path(&d, |x, y| (x - b.left, y - b.top));
        self.scene.update(id, |n| {
            let old = n.vector.as_deref().cloned();
            n.vector = Some(Box::new(pergel_core::VectorPath {
                d: local,
                size: (bounds.w, bounds.h),
                even_odd: old.as_ref().is_some_and(|o| o.even_odd),
                cap: old.as_ref().map_or(pergel_core::Cap::Round, |o| o.cap),
                join: old.as_ref().map_or(pergel_core::Join::Round, |o| o.join),
                start: old.as_ref().map_or(pergel_core::EndPoint::None, |o| o.start),
                end: old.as_ref().map_or(pergel_core::EndPoint::None, |o| o.end),
            }));
        });
        if self.scene.node(id).is_some_and(|n| n.bounds != bounds) {
            self.scene.set_bounds(id, bounds);
        }
    }

    /// Leaves vector edit mode (or finishes a pen drawing) and closes its undo step.
    fn vector_end(&mut self) {
        if self.vedit.is_none() {
            return;
        }
        self.vector_write();
        let Some(v) = self.vedit.take() else { return };
        let anchors: usize = v.paths.iter().map(|p| p.anchors.len()).sum();
        if anchors < 2 {
            // A pen click without a second point draws nothing.
            self.scene.remove(v.node);
            self.selection.clear();
        } else {
            self.selection = vec![v.node];
        }
        self.settle();
        self.prune_selection();
        let after = self.selection.clone();
        self.scene.commit(&after);
    }

    fn vector_tolerance(&self) -> f32 {
        7.0 / self.view.zoom
    }

    fn vector_down(&mut self, x: f32, y: f32, shift: bool, alt: bool) -> i32 {
        let tol = self.vector_tolerance();
        let Some(v) = self.vedit.as_mut() else { return 0 };
        let dist = |a: (f32, f32)| ((a.0 - x).powi(2) + (a.1 - y).powi(2)).sqrt();
        // Handles of selected anchors first, then anchors, then segments.
        for &(pi, ai) in &v.selected {
            let Some(a) = v.paths.get(pi).and_then(|p| p.anchors.get(ai)) else { continue };
            for (out, h) in [(false, a.cin), (true, a.cout)] {
                if h.is_some_and(|h| dist(h) <= tol) {
                    v.drag = Some(VDrag::Handle { path: pi, anchor: ai, out, mirror: !alt });
                    return 2;
                }
            }
        }
        let hit = v.paths.iter().enumerate().find_map(|(pi, p)| p.anchors.iter().position(|a| dist(a.p) <= tol).map(|ai| (pi, ai)));
        if let Some(h) = hit {
            if shift {
                if let Some(k) = v.selected.iter().position(|s| *s == h) {
                    v.selected.remove(k);
                } else {
                    v.selected.push(h);
                }
            } else if !v.selected.contains(&h) {
                v.selected = vec![h];
            }
            if alt && let Some(a) = v.paths.get_mut(h.0).and_then(|p| p.anchors.get_mut(h.1)) {
                // Alt-click an anchor: toggle between a sharp corner and a smooth point.
                if a.cin.is_some() || a.cout.is_some() {
                    a.cin = None;
                    a.cout = None;
                    self.vector_write();
                    return 1;
                }
            }
            v.drag = Some(VDrag::Anchors { from: (x, y), orig: v.paths.clone() });
            return 1;
        }
        if let Some((pi, i, t, d)) = pergel_core::nearest_segment(&v.paths, (x, y))
            && d <= tol
        {
            // Clicking a segment adds an anchor there.
            let path = &mut v.paths[pi];
            let k = pergel_core::split_segment(path, i, t);
            v.selected = vec![(pi, k)];
            v.drag = Some(VDrag::Anchors { from: (x, y), orig: v.paths.clone() });
            self.vector_write();
            return 3;
        }
        if !shift {
            v.selected.clear();
        }
        4
    }

    fn vector_move(&mut self, x: f32, y: f32) {
        let Some(v) = self.vedit.as_mut() else { return };
        match &v.drag {
            Some(VDrag::Anchors { from, orig }) => {
                let (dx, dy) = (x - from.0, y - from.1);
                let mut paths = orig.clone();
                for &(pi, ai) in &v.selected {
                    if let Some(a) = paths.get_mut(pi).and_then(|p| p.anchors.get_mut(ai)) {
                        let mv = |p: (f32, f32)| (p.0 + dx, p.1 + dy);
                        a.p = mv(a.p);
                        a.cin = a.cin.map(mv);
                        a.cout = a.cout.map(mv);
                    }
                }
                v.paths = paths;
            }
            Some(VDrag::Handle { path, anchor, out, mirror }) => {
                let (pi, ai, out, mirror) = (*path, *anchor, *out, *mirror);
                if let Some(a) = v.paths.get_mut(pi).and_then(|p| p.anchors.get_mut(ai)) {
                    let opposite = (2.0 * a.p.0 - x, 2.0 * a.p.1 - y);
                    if out {
                        a.cout = Some((x, y));
                        if mirror && a.cin.is_some() {
                            a.cin = Some(opposite);
                        }
                    } else {
                        a.cin = Some((x, y));
                        if mirror && a.cout.is_some() {
                            a.cout = Some(opposite);
                        }
                    }
                }
            }
            Some(VDrag::PenHandle { path, anchor }) => {
                let (pi, ai) = (*path, *anchor);
                if let Some(a) = v.paths.get_mut(pi).and_then(|p| p.anchors.get_mut(ai)) {
                    a.cout = Some((x, y));
                    a.cin = Some((2.0 * a.p.0 - x, 2.0 * a.p.1 - y));
                }
            }
            None => {
                if v.pen {
                    v.rubber = Some((x, y));
                }
                return;
            }
        }
        self.vector_write();
    }

    /// Deletes the selected anchors (sub-paths left with fewer than two anchors go away).
    fn vector_delete(&mut self) -> bool {
        let Some(v) = self.vedit.as_mut() else { return false };
        if v.selected.is_empty() {
            return false;
        }
        let mut sel = v.selected.clone();
        sel.sort_unstable_by(|a, b| b.cmp(a));
        for (pi, ai) in sel {
            if let Some(p) = v.paths.get_mut(pi)
                && ai < p.anchors.len()
            {
                p.anchors.remove(ai);
                if p.anchors.len() < 3 {
                    p.closed = false;
                }
            }
        }
        v.paths.retain(|p| p.anchors.len() > 1);
        v.selected.clear();
        self.vector_write();
        true
    }

    /// Pen tool click: starts a vector (black 1 px stroke, like Figma) or adds an anchor;
    /// clicking the first anchor closes the path and finishes.
    fn pen_down(&mut self, x: f32, y: f32) -> i32 {
        if self.vedit.as_ref().is_some_and(|v| !v.pen) {
            // Pen inside edit mode: start a new sub-path in the same vector.
            if let Some(v) = self.vedit.as_mut() {
                v.pen = true;
                v.paths.push(pergel_core::SubPath { anchors: vec![pergel_core::Anchor::corner(x, y)], closed: false });
                let pi = v.paths.len() - 1;
                v.selected = vec![(pi, 0)];
                v.drag = Some(VDrag::PenHandle { path: pi, anchor: 0 });
            }
            return 1;
        }
        if self.vedit.is_none() {
            let before = self.selection.clone();
            self.scene.begin(&before);
            let parent = self.scene.drop_target(x, y, &[]);
            let name = self.scene.next_name_for("Vector");
            let mut node = Node::new(Kind::Vector, name, Rect::new(x, y, 0.0, 0.0), DEFAULT_FILL);
            node.fill_on = false;
            node.stroke = Some((BLACK, 1.0));
            node.stroke_align = StrokeAlign::Center;
            node.vector = Some(Box::new(pergel_core::VectorPath { d: format!("M{x} {y}"), size: (0.0, 0.0), even_odd: false, cap: pergel_core::Cap::Round, join: pergel_core::Join::Round, start: Default::default(), end: Default::default() }));
            let id = self.scene.insert(node, parent, None);
            self.selection = vec![id];
            self.vedit = Some(VectorEdit {
                node: id,
                paths: vec![pergel_core::SubPath { anchors: vec![pergel_core::Anchor::corner(x, y)], closed: false }],
                selected: vec![(0, 0)],
                drag: Some(VDrag::PenHandle { path: 0, anchor: 0 }),
                pen: true,
                pencil: None,
                rubber: None,
            });
            return 1;
        }
        let tol = self.vector_tolerance();
        let Some(v) = self.vedit.as_mut() else { return 0 };
        let pi = v.paths.len() - 1;
        let path = &mut v.paths[pi];
        let first = path.anchors[0].p;
        if path.anchors.len() >= 2 && ((first.0 - x).powi(2) + (first.1 - y).powi(2)).sqrt() <= tol {
            path.closed = true;
            v.pen = false;
            v.drag = None;
            self.vector_write();
            return 2;
        }
        path.anchors.push(pergel_core::Anchor::corner(x, y));
        let ai = path.anchors.len() - 1;
        v.selected = vec![(pi, ai)];
        v.drag = Some(VDrag::PenHandle { path: pi, anchor: ai });
        self.vector_write();
        1
    }
}

/// Pencil (Shift+P): starts a freehand stroke at a screen point.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pencil_begin(sx: f32, sy: f32) {
    with_engine(|e| {
        if e.vedit.is_some() {
            e.vector_end();
        }
        let (x, y) = e.world(sx, sy);
        let before = e.selection.clone();
        e.scene.begin(&before);
        let parent = e.scene.drop_target(x, y, &[]);
        let name = e.scene.next_name_for("Vector");
        let mut node = Node::new(Kind::Vector, name, Rect::new(x, y, 0.0, 0.0), DEFAULT_FILL);
        node.fill_on = false;
        node.stroke = Some((BLACK, 2.0));
        node.stroke_align = StrokeAlign::Center;
        node.vector = Some(Box::new(pergel_core::VectorPath { d: format!("M{x} {y}"), size: (0.0, 0.0), even_odd: false, cap: pergel_core::Cap::Round, join: pergel_core::Join::Round, start: Default::default(), end: Default::default() }));
        let id = e.scene.insert(node, parent, None);
        e.selection = vec![id];
        e.vedit = Some(VectorEdit {
            node: id,
            paths: vec![pergel_core::SubPath { anchors: vec![pergel_core::Anchor::corner(x, y)], closed: false }],
            selected: Vec::new(),
            drag: None,
            pen: false,
            pencil: Some(vec![(x, y)]),
            rubber: None,
        });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_pencil_point(sx: f32, sy: f32) {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        let min = 1.5 / e.view.zoom;
        let Some(v) = e.vedit.as_mut() else { return };
        let Some(points) = v.pencil.as_mut() else { return };
        if points.last().is_some_and(|p| ((p.0 - x).powi(2) + (p.1 - y).powi(2)).sqrt() < min) {
            return;
        }
        points.push((x, y));
        v.paths = vec![pergel_core::SubPath { anchors: points.iter().map(|p| pergel_core::Anchor::corner(p.0, p.1)).collect(), closed: false }];
        e.vector_write();
    });
}

/// Ends the stroke: simplified and smoothed into curves, one undo step.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pencil_end() {
    with_engine(|e| {
        let tol = 1.2 / e.view.zoom;
        if let Some(v) = e.vedit.as_mut()
            && let Some(points) = v.pencil.take()
        {
            v.paths = vec![pergel_core::smooth_stroke(&points, tol)];
        }
        e.vector_end();
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_begin(id: i32) -> i32 {
    with_engine(|e| {
        let id = if id >= 0 { Some(id as NodeId) } else { e.selection.first().copied() };
        id.is_some_and(|id| e.vector_begin(id)) as i32
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_end() {
    with_engine(|e| e.vector_end());
}

/// 0 = not editing, 1 = editing, 2 = drawing with the pen.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_state() -> i32 {
    with_engine(|e| e.vedit.as_ref().map_or(0, |v| if v.pen { 2 } else { 1 })).unwrap_or(0)
}

/// Pointer down in vector edit mode: 1 anchor, 2 handle, 3 new anchor on a segment, 4 empty.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_down(sx: f32, sy: f32, shift: i32, alt: i32) -> i32 {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        e.vector_down(x, y, shift != 0, alt != 0)
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_move(sx: f32, sy: f32) {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        e.vector_move(x, y);
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_up() {
    with_engine(|e| {
        if let Some(v) = e.vedit.as_mut() {
            v.drag = None;
        }
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_vector_delete() -> i32 {
    with_engine(|e| e.vector_delete() as i32).unwrap_or(0)
}

/// Pen tool click: 1 anchor added, 2 path closed (drawing finished).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pen_down(sx: f32, sy: f32) -> i32 {
    with_engine(|e| {
        let (x, y) = e.world(sx, sy);
        e.pen_down(x, y)
    })
    .unwrap_or(0)
}

/// Line (3), arrow (4), polygon (5) and star (6) tools: a vector layer in the dragged box.
fn create_vector(kind: u32, sx: f32, sy: f32, sw: f32, sh: f32) -> i32 {
    with_engine(|e| {
        let down = (sw >= 0.0) == (sh >= 0.0);
        let tool = match kind {
            3 => pergel_core::ShapeTool::Line { down },
            4 => pergel_core::ShapeTool::Arrow { down },
            5 => pergel_core::ShapeTool::Polygon(3),
            _ => pergel_core::ShapeTool::Star { points: 5, ratio: 0.382 },
        };
        let mut r = screen_to_world(&e.view, sx, sy, sw, sh);
        if r.w < 2.0 && r.h < 2.0 {
            // A click: Figma's default sizes.
            if tool.is_line() {
                r.w = 100.0;
                r.h = 0.0;
            } else {
                r.w = 100.0;
                r.h = 100.0;
            }
        }
        let r = Rect::new(r.x.round(), r.y.round(), r.w.round(), r.h.round());
        let (x, y) = e.world(sx, sy);
        e.step(|e| {
            let parent = e.scene.drop_target(x, y, &[]);
            let name = e.scene.next_name_for(tool.label());
            let mut node = Node::new(Kind::Vector, name, r, DEFAULT_FILL);
            node.vector = Some(Box::new(tool.path(r.w, r.h)));
            if tool.is_line() {
                node.fill_on = false;
                node.stroke = Some((BLACK, 1.0));
                node.stroke_align = StrokeAlign::Center;
            }
            let id = e.scene.insert(node, parent, None);
            e.selection = vec![id];
            id as i32
        })
    })
    .unwrap_or(-1)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_delete_selection() -> u32 {
    with_engine(|e| {
        e.step(|e| {
            let removed = e.editable().into_iter().filter(|id| e.scene.remove(*id)).count();
            e.selection.clear();
            removed as u32
        })
    })
    .unwrap_or(0)
}

/// 0 group selection (Ctrl+G), 1 frame selection (Ctrl+Alt+G).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_group(kind: u32) -> i32 {
    with_engine(|e| {
        if e.selection.is_empty() {
            return -1;
        }
        e.step(|e| {
            let ids = e.editable();
            let group = e.scene.group(&ids, if kind == 1 { Kind::Frame } else { Kind::Group });
            if let Some(g) = group {
                e.selection = vec![g];
            }
            group.map_or(-1, |g| g as i32)
        })
    })
    .unwrap_or(-1)
}

/// Ungroup / remove frame (Ctrl+Backspace); the former children become the selection.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_ungroup() -> u32 {
    with_engine(|e| {
        e.step(|e| {
            let mut next = Vec::new();
            for id in e.editable() {
                let children = if e.scene.node(id).is_some_and(|n| n.component.is_some()) {
                    Vec::new()
                } else {
                    e.scene.ungroup(id)
                };
                if children.is_empty() {
                    next.push(id);
                }
                next.extend(children);
            }
            e.selection = next;
            e.selection.len() as u32
        })
    })
    .unwrap_or(0)
}

/// Ctrl+D: copies sit right above their originals; top-level frames go beside them.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_duplicate() -> u32 {
    with_engine(|e| {
        e.step(|e| {
            let mut ids = Vec::new();
            for id in e.editable() {
                let (Some(mut tree), Some((parent, i))) = (e.scene.extract(id), e.scene.position(id)) else {
                    continue;
                };
                if parent.is_none() && tree.kind == Kind::Frame {
                    tree.translate(tree.bounds.w + 40.0, 0.0);
                }
                let (cx, cy) = tree.aabb().center();
                let copy = if e.scene.is_main(id) {
                    // Duplicating a main component places an instance of it, as in Figma.
                    e.scene.create_instance(id, parent, Some(i + 1), (cx, cy))
                } else {
                    let copy = e.scene.insert_tree(&tree, parent, Some(i + 1));
                    if let Some(main) = e.scene.main_of(id) {
                        e.scene.relink_instance(copy, main);
                    }
                    Some(copy)
                };
                ids.extend(copy);
            }
            e.selection = ids;
            e.selection.len() as u32
        })
    })
    .unwrap_or(0)
}

/// The selection as clipboard text; empty when nothing is selected.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_copy() -> *const c_char {
    emit(with_engine(|e| {
        let trees: Vec<Tree> = e.scene.outermost(&e.selection).iter().filter_map(|id| e.scene.extract(*id)).collect();
        if trees.is_empty() { String::new() } else { to_text(trees, &e.scene.images) }
    }))
}

/// Pastes the clipboard text held in the text buffer; returns how many layers were added.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_paste() -> u32 {
    let clip = text();
    with_engine(|e| {
        let Some(trees) = from_text(&clip, &mut e.scene.images) else { return 0 };
        e.paste(trees)
    })
    .unwrap_or(0)
}

/// Imports SVG markup as vector layers centred on a screen point (NaN: the view's centre).
/// The text buffer holds the layer name (`name_len` bytes) then the markup. Returns the new
/// frame's id, or -1 when the markup is not usable SVG.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_import_svg(sx: f32, sy: f32, name_len: usize) -> i32 {
    let bytes = text_bytes();
    if name_len > bytes.len() {
        return -1;
    }
    let (name, svg) = bytes.split_at(name_len);
    let name = String::from_utf8_lossy(name).trim().to_string();
    let svg = String::from_utf8_lossy(svg).into_owned();
    with_engine(|e| {
        let Ok(imported) = import_svg(&svg, if name.is_empty() { "SVG" } else { &name }, &mut e.scene.images) else { return -1 };
        e.place_tree(imported.tree, sx, sy)
    })
    .unwrap_or(-1)
}

/// Places an image file as a rectangle with an image fill, centred on a screen point (NaN: the
/// view's centre). The text buffer holds the layer name (`name_len` bytes) then the file.
/// Returns the new layer's id, or -1 when the bytes are not an image.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_place_image(sx: f32, sy: f32, name_len: usize) -> i32 {
    let bytes = text_bytes();
    if name_len > bytes.len() {
        return -1;
    }
    let (name, data) = bytes.split_at(name_len);
    let name = String::from_utf8_lossy(name).trim().to_string();
    let data = data.to_vec();
    with_engine(|e| {
        let Some((hash, w, h)) = e.scene.images.add(data) else { return -1 };
        // Natural size, scaled down to fit two thirds of the view (as Figma does).
        let area = e.view.world_area();
        let k = (area.w * 0.66 / w as f32).min(area.h * 0.66 / h as f32).min(1.0);
        let (bw, bh) = ((w as f32 * k).round().max(1.0), (h as f32 * k).round().max(1.0));
        let mut node = Node::new(Kind::Rectangle, if name.is_empty() { "Image".to_string() } else { name }, Rect::new(0.0, 0.0, bw, bh), DEFAULT_FILL);
        node.paint = Some(Box::new(Paint::Image(ImageFill { hash, mode: ImageMode::Fill, width: w, height: h, scale: 1.0, opacity: 1.0, adjust: Default::default() })));
        let mut scratch = Scene::default();
        let id = scratch.insert(node, None, None);
        let Some(tree) = scratch.extract(id) else { return -1 };
        e.place_tree(tree, sx, sy)
    })
    .unwrap_or(-1)
}

/// Sets the image of every selected layer that can hold a fill (Figma's image swap). The text
/// buffer holds the file. Returns how many layers changed.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_image() -> u32 {
    let data = text_bytes();
    with_engine(|e| {
        let Some((hash, w, h)) = e.scene.images.add(data) else { return 0 };
        let ids: Vec<NodeId> = e.editable().into_iter().filter(|id| e.scene.node(*id).is_some_and(|n| !matches!(n.kind, Kind::Group | Kind::Text))).collect();
        e.step(|e| {
            for id in &ids {
                let hash = hash.clone();
                e.scene.update(*id, |n| {
                    let mode = match n.paint.as_deref() {
                        Some(Paint::Image(old)) => old.mode,
                        _ => ImageMode::Fill,
                    };
                    n.paint = Some(Box::new(Paint::Image(ImageFill { hash, mode, width: w, height: h, scale: 1.0, opacity: 1.0, adjust: Default::default() })));
                    n.fill_on = true;
                });
            }
            ids.len() as u32
        })
    })
    .unwrap_or(0)
}

/// The original file of an image fill (`hash`, from the selection JSON), for "Export image".
#[unsafe(no_mangle)]
pub extern "C" fn pergel_image_bytes() -> *const u8 {
    let hash = text();
    with_engine(|e| {
        let bytes = e.scene.images.get(&hash).map(|b| b.to_vec()).unwrap_or_default();
        PIXELS.with_borrow_mut(|out| {
            out.clear();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
            out.as_ptr()
        })
    })
    .unwrap_or(std::ptr::null())
}

/// Layers-panel drag and drop: moves the selection 0 above `target`, 1 below it, or 2 into it
/// (on top of its children), keeping the layers' relative order.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_move_to(target: u32, place: u32) -> i32 {
    with_engine(|e| {
        let moving = e.movable();
        let into_instance = e.scene.in_instance(target) || place == 2 && e.scene.is_instance(target);
        if into_instance || moving.iter().any(|m| *m == target || e.scene.is_ancestor(*m, target)) {
            return 0;
        }
        let (parent, index) = match (place, e.scene.position(target)) {
            (2, _) => (Some(target), None),
            (_, Some((parent, i))) => (parent, Some(if place == 0 { i + 1 } else { i })),
            _ => return 0,
        };
        e.step(|e| {
            let mut at = index;
            for id in &moving {
                if !e.scene.reparent(*id, parent, at) {
                    return 0;
                }
                at = at.and_then(|_| e.scene.position(*id)).map(|(_, i)| i + 1);
            }
            1
        })
    })
    .unwrap_or(0)
}

/// 0 bring forward (Ctrl+]), 1 send backward (Ctrl+[), 2 bring to front (]), 3 send to back ([).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_reorder(op: u32) {
    let order = match op {
        0 => Order::Forward,
        1 => Order::Backward,
        2 => Order::Front,
        _ => Order::Back,
    };
    with_engine(|e| {
        e.step(|e| {
            let ids = e.editable();
            e.scene.reorder(&ids, order);
        })
    });
}

/// Flag 0 visible, 1 locked, 2 clip content. `id` -1 means the selection; `value` -1 toggles
/// (based on the first layer, like Figma's Ctrl+Shift+H / Ctrl+Shift+L).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_flag(id: i32, flag: u32, value: i32) {
    with_engine(|e| {
        let ids = if id < 0 { e.scene.outermost(&e.selection) } else { vec![id as NodeId] };
        let Some(first) = ids.first().and_then(|id| e.scene.node(*id)) else { return };
        let current = match flag {
            0 => first.visible,
            1 => first.locked,
            _ => first.clip,
        };
        let on = if value < 0 { !current } else { value != 0 };
        e.step(|e| {
            for id in &ids {
                e.scene.update(*id, |n| match flag {
                    0 => n.visible = on,
                    1 => n.locked = on,
                    _ => n.clip = on,
                });
            }
        });
        if flag == 0 && !on {
            e.hover = None;
        }
    });
}

/// Registers a font. The text buffer holds the family name (`name_len` bytes, may be 0 to use the
/// font's own name) followed by the font file. Returns 1 when the font was usable.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_add_font(name_len: usize) -> i32 {
    let bytes = text_bytes();
    if name_len > bytes.len() {
        return 0;
    }
    let (name, font) = bytes.split_at(name_len);
    let name = String::from_utf8_lossy(name).trim().to_string();
    with_engine(|e| {
        let added = e.renderer.text.add_font(font, (!name.is_empty()).then_some(name.as_str())).is_some();
        if added {
            e.fit_all_text();
        }
        added as i32
    })
    .unwrap_or(0)
}

/// Font families the document's text uses on every page: `[{family, weights, italic, available}]`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_used_fonts() -> *const c_char {
    emit(with_engine(|e| {
        let mut used: std::collections::BTreeMap<String, (std::collections::BTreeSet<u16>, bool)> = Default::default();
        let scenes = std::iter::once(&e.scene).chain(e.pages.iter().enumerate().filter(|(i, _)| *i != e.page).map(|(_, p)| &p.scene));
        for scene in scenes {
            for n in scene.nodes() {
                if let Some(t) = n.text.as_deref() {
                    let entry = used.entry(t.family.clone()).or_default();
                    entry.0.insert(t.weight);
                    entry.1 |= t.italic;
                }
            }
        }
        let list: Vec<serde_json::Value> = used
            .into_iter()
            .map(|(family, (weights, italic))| {
                let available = e.renderer.text.has_family(&family);
                serde_json::json!({ "family": family, "weights": weights, "italic": italic, "available": available })
            })
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

/// Styles characters `start..end` of a text layer: the text buffer holds a JSON text run
/// (`{"weight": 700}`, `{"italic": true}`, `{"color": 4294901760}`…). Toggles when `toggle` is 1
/// and the whole range already has that style.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_text_style_range(id: u32, start: u32, end: u32, toggle: i32) -> i32 {
    let Ok(mut style) = serde_json::from_str::<pergel_core::TextRun>(&text()) else { return 0 };
    with_engine(|e| {
        let Some(t) = e.scene.node(id).and_then(|n| n.text.as_deref()).cloned() else { return 0 };
        if toggle != 0 {
            let (s, en) = (start as usize, end as usize);
            let covered = |f: &dyn Fn(&pergel_core::TextRun) -> bool, base: bool| -> bool {
                (s..en).all(|i| t.runs.iter().find(|r| r.start <= i && i < r.end).map_or(base, f))
            };
            if let Some(w) = style.weight {
                let bold = covered(&|r: &pergel_core::TextRun| r.weight.unwrap_or(t.weight) >= 600, t.weight >= 600);
                style.weight = Some(if bold { if w >= 600 { 400 } else { w } } else { w });
            }
            if style.italic.is_some() {
                style.italic = Some(!covered(&|r: &pergel_core::TextRun| r.italic.unwrap_or(t.italic), t.italic));
            }
            if let Some(d) = style.decoration {
                let on = covered(&|r: &pergel_core::TextRun| r.decoration.unwrap_or(t.decoration) == d, t.decoration == d);
                style.decoration = Some(if on { pergel_core::Decoration::None } else { d });
            }
        }
        let editing = e.editing == Some(id);
        let apply = |e: &mut Engine| {
            e.scene.update(id, |n| {
                if let Some(t) = n.text.as_mut() {
                    t.style_range(start as usize, end as usize, &style);
                }
            });
            e.fit_text(id);
        };
        // While typing, the edit session's undo step takes the change.
        if editing { apply(e) } else { e.step(apply) }
        1
    })
    .unwrap_or(0)
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_fonts_json() -> *const c_char {
    emit(with_engine(|e| serde_json::to_string(e.renderer.text.families()).unwrap_or_default()))
}

/// Text tool: a click makes an auto-width layer, a drag a fixed-size box. Call inside
/// `pergel_begin`/`pergel_commit` so typing and creating undo as one step.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_create_text(sx: f32, sy: f32, sw: f32, sh: f32) -> i32 {
    with_engine(|e| {
        let r = screen_to_world(&e.view, sx, sy, sw, sh);
        let (x, y) = e.world(sx, sy);
        let mut style = Text { content: String::new(), auto_name: true, ..e.text_style.clone() };
        let dragged = r.w >= 4.0 || r.h >= 4.0;
        style.resize = if dragged { TextResize::Fixed } else { TextResize::AutoWidth };
        let parent = e.scene.drop_target(x, y, &[]);
        let mut node = Node::text(style, r.x.round(), r.y.round(), BLACK);
        if dragged {
            node.bounds = Rect::new(r.x.round(), r.y.round(), r.w.round().max(1.0), r.h.round().max(1.0));
        }
        let id = e.scene.insert(node, parent, None);
        e.fit_text(id);
        e.selection = vec![id];
        id as i32
    })
    .unwrap_or(-1)
}

/// Replaces a text layer's characters with the text buffer.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_text(id: u32) -> i32 {
    let content = text();
    with_engine(|e| {
        e.step(|e| {
            let ok = e.scene.update(id, |n| {
                if let Some(t) = n.text.as_mut() {
                    // Styled ranges move with the edit.
                    t.set_content(content);
                    if t.auto_name {
                        n.name = t.layer_name();
                    }
                }
            });
            e.fit_text(id);
            ok as i32
        })
    })
    .unwrap_or(0)
}

/// Applies the typography JSON in the text buffer (Figma's Text section) to `id`, or to every
/// selected text layer when `id` is -1. Content and naming are left as they are.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_text_style(id: i32) -> i32 {
    let Ok(style) = serde_json::from_str::<Text>(&text()) else { return 0 };
    with_engine(|e| {
        let ids = e.selected_text(id);
        e.text_style = Text { content: String::new(), auto_name: true, ..style.clone() };
        e.step(|e| {
            for id in &ids {
                e.scene.update(*id, |n| {
                    if let Some(t) = n.text.as_mut() {
                        // A property set for the whole layer replaces the ranges' own values.
                        let mut runs = std::mem::take(&mut t.runs);
                        for r in &mut runs {
                            if style.family != t.family {
                                r.family = None;
                            }
                            if style.weight != t.weight {
                                r.weight = None;
                            }
                            if style.italic != t.italic {
                                r.italic = None;
                            }
                            if style.size != t.size {
                                r.size = None;
                            }
                            if style.decoration != t.decoration {
                                r.decoration = None;
                            }
                            if style.letter_spacing != t.letter_spacing {
                                r.letter_spacing = None;
                            }
                        }
                        runs.retain(|r| !r.is_plain());
                        **t = Text { content: std::mem::take(&mut t.content), auto_name: t.auto_name, runs, ..style.clone() };
                    }
                });
                e.fit_text(*id);
            }
            ids.len() as i32
        })
    })
    .unwrap_or(0)
}

/// The text layer the UI is editing in its overlay (-1 when none); the canvas skips its text.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_set_editing(id: i32) {
    with_engine(|e| e.editing = (id >= 0).then_some(id as NodeId));
}

/// Applies the property patch (JSON in the text buffer) to `id`, or to every selected layer
/// when `id` is -1. One undo step.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_patch(id: i32) -> i32 {
    let Ok(patch) = serde_json::from_str::<Patch>(&text()) else { return 0 };
    with_engine(|e| {
        let ids = if id >= 0 { vec![id as NodeId] } else { e.scene.outermost(&e.selection) };
        e.step(|e| {
            for id in &ids {
                e.apply_patch(*id, &patch);
            }
            ids.len() as i32
        })
    })
    .unwrap_or(0)
}

/// Shift+A (`add` 1): gives the selected frame an auto layout guessed from its children, or
/// wraps the selection in a new auto-layout frame that hugs it. Alt+Shift+A (`add` 0) removes it.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_auto_layout(add: i32) -> i32 {
    with_engine(|e| {
        let sel = e.scene.outermost(&e.selection);
        if sel.is_empty() {
            return 0;
        }
        e.step(|e| {
            if add == 0 {
                for id in &sel {
                    e.scene.update(*id, |n| n.layout = None);
                }
                return 1;
            }
            let single = match sel.as_slice() {
                [one] => e.scene.node(*one).filter(|n| n.kind == Kind::Frame && n.layout.is_none()).map(|n| (n.id, n.bounds)),
                _ => None,
            };
            if let Some((frame, bounds)) = single {
                let kids: Vec<NodeId> = e.scene.children(Some(frame)).to_vec();
                let layout = e.scene.infer_layout(Some(bounds), &kids);
                e.scene.update(frame, |n| n.layout = Some(Box::new(layout)));
                return 1;
            }
            let Some(frame) = e.scene.group(&sel, Kind::Frame) else { return 0 };
            let kids: Vec<NodeId> = e.scene.children(Some(frame)).to_vec();
            let layout = e.scene.infer_layout(None, &kids);
            e.scene.update(frame, |n| {
                n.layout = Some(Box::new(layout));
                n.sizing = [Sizing::Hug, Sizing::Hug];
                n.fill_on = false;
                n.clip = false;
            });
            e.selection = vec![frame];
            1
        })
    })
    .unwrap_or(0)
}

/// Create component (Ctrl+Alt+K): a selected frame becomes a main component; anything else is
/// wrapped in a new component frame.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_create_component() -> i32 {
    with_engine(|e| {
        let sel = e.editable();
        if sel.is_empty() {
            return -1;
        }
        e.step(|e| {
            let single = match sel.as_slice() {
                [one] => e.scene.node(*one).filter(|n| n.kind == Kind::Frame && n.component.is_none()).map(|n| n.id),
                _ => None,
            };
            let id = match single {
                Some(id) => id,
                None => {
                    let name = match sel.as_slice() {
                        [one] => e.scene.node(*one).map(|n| n.name.clone()),
                        _ => None,
                    };
                    let Some(frame) = e.scene.group(&sel, Kind::Frame) else { return -1 };
                    let name = name.unwrap_or_else(|| e.scene.next_name_for("Component"));
                    e.scene.update(frame, |n| {
                        n.name = name;
                        n.fill_on = false;
                        n.clip = false;
                    });
                    frame
                }
            };
            e.scene.update(id, |n| n.component = Some(Component::Main));
            e.selection = vec![id];
            id as i32
        })
    })
    .unwrap_or(-1)
}

/// Places an instance of `main` centred at a screen point (the view centre when `sx` < 0),
/// inside the frame under that point.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_create_instance(main: u32, sx: f32, sy: f32) -> i32 {
    with_engine(|e| {
        if !e.scene.is_main(main) {
            return -1;
        }
        let (x, y) = if sx < 0.0 { e.view.world_area().center() } else { e.world(sx, sy) };
        e.step(|e| {
            let parent = e.scene.drop_target(x, y, &[main]);
            let id = e.scene.create_instance(main, parent, None, (x.round(), y.round()));
            if let Some(id) = id {
                e.round_positions(&[id]);
                e.selection = vec![id];
            }
            id.map_or(-1, |id| id as i32)
        })
    })
    .unwrap_or(-1)
}

/// Detach instance (Ctrl+Alt+B).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_detach_instance() -> u32 {
    with_engine(|e| {
        let ids = e.editable();
        e.step(|e| ids.iter().filter(|id| e.scene.detach(**id)).count() as u32)
    })
    .unwrap_or(0)
}

/// Reset all changes of the selected instances.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_reset_instance() {
    with_engine(|e| {
        let ids: Vec<NodeId> = e.editable().into_iter().filter(|id| e.scene.is_instance(*id)).collect();
        e.step(|e| {
            for id in ids {
                e.scene.reset_instance(id);
            }
        });
    });
}

/// Selects the main component of the selected instance; returns its id or -1.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_go_to_main() -> i32 {
    with_engine(|e| {
        let main = e.selection.first().and_then(|id| e.scene.main_of(*id));
        if let Some(m) = main {
            e.selection = vec![m];
            if let Some(b) = e.scene.node(m).map(|n| n.aabb()) {
                e.view.fit(b, FIT_MARGIN * 2.0);
            }
        }
        main.map_or(-1, |m| m as i32)
    })
    .unwrap_or(-1)
}

#[derive(Serialize)]
struct ComponentInfo<'a> {
    id: NodeId,
    name: &'a str,
    w: f32,
    h: f32,
    instances: usize,
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_components_json() -> *const c_char {
    emit(with_engine(|e| {
        let list: Vec<ComponentInfo> = e
            .scene
            .mains()
            .into_iter()
            .filter_map(|id| e.scene.node(id))
            .map(|n| ComponentInfo { id: n.id, name: &n.name, w: n.bounds.w, h: n.bounds.h, instances: e.scene.instances_of(n.id).len() })
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

thread_local! {
    static PIXELS: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Renders a layer into an RGBA thumbnail at most `max` pixels on its long side. Returns a
/// pointer to `width`, `height` (u32 each) followed by the pixels, or null.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_thumbnail(id: u32, max: u32) -> *const u8 {
    with_engine(|e| {
        // u32::MAX: the whole page.
        let area = if id == u32::MAX { e.scene.bounds() } else { e.scene.node(id)?.paint_bounds() };
        let pixels = e.renderer.thumbnail(&e.scene, area, max as f32)?;
        PIXELS.with_borrow_mut(|out| {
            *out = pixels;
            Some(out.as_ptr())
        })
    })
    .flatten()
    .unwrap_or(std::ptr::null())
}

/// A one-page PDF of a layer. Returns a pointer to the byte length (u32) followed by the bytes.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pdf(id: u32) -> *const u8 {
    with_engine(|e| {
        let area = e.scene.node(id)?.paint_bounds();
        let bytes = e.renderer.pdf(&e.scene, area)?;
        PIXELS.with_borrow_mut(|out| {
            out.clear();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
            Some(out.as_ptr())
        })
    })
    .flatten()
    .unwrap_or(std::ptr::null())
}

/// A multi-page PDF, one page per layer (`text()` holds the ids, e.g. a deck's slides in order).
/// Returns a pointer to the byte length (u32) followed by the bytes.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_pdf_pages() -> *const u8 {
    let ids: Vec<NodeId> = serde_json::from_str(&text()).unwrap_or_default();
    with_engine(|e| {
        let areas: Vec<Rect> = ids.iter().filter_map(|id| e.scene.node(*id).map(|n| n.paint_bounds())).collect();
        if areas.is_empty() {
            return None;
        }
        let bytes = e.renderer.pdf_pages(&e.scene, &areas)?;
        PIXELS.with_borrow_mut(|out| {
            out.clear();
            out.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend_from_slice(&bytes);
            Some(out.as_ptr())
        })
    })
    .flatten()
    .unwrap_or(std::ptr::null())
}

/// The fill and stroke colours used inside the selection, most used first ("Selection colors").
#[unsafe(no_mangle)]
pub extern "C" fn pergel_selection_colors() -> *const c_char {
    emit(with_engine(|e| {
        let mut counts: Vec<(u32, usize)> = Vec::new();
        let mut add = |c: Color| match counts.iter_mut().find(|(v, _)| *v == c.argb()) {
            Some((_, n)) => *n += 1,
            None => counts.push((c.argb(), 1)),
        };
        for root in e.scene.outermost(&e.selection) {
            for id in e.scene.subtree(root) {
                let Some(n) = e.scene.node(id) else { continue };
                if n.fill_on && n.kind != Kind::Group && n.fill.argb() >> 24 != 0 {
                    add(n.fill);
                }
                if let Some((c, _)) = n.stroke {
                    add(c);
                }
            }
        }
        counts.sort_by(|a, b| b.1.cmp(&a.1));
        let list: Vec<serde_json::Value> = counts
            .into_iter()
            .map(|(argb, count)| serde_json::json!({ "color": hex(Color(argb)), "opacity": alpha(Color(argb)), "count": count }))
            .collect();
        serde_json::to_string(&list).unwrap_or_default()
    }))
}

/// Replaces one colour (fills and strokes) everywhere in the selection. Text buffer:
/// `{"from": "#RRGGBB", "fromOpacity": 100, "to": "#RRGGBB", "toOpacity": 100}`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_replace_color() -> u32 {
    let Ok(args) = serde_json::from_str::<serde_json::Value>(&text()) else { return 0 };
    let get = |k: &str| args.get(k).and_then(|v| v.as_str()).and_then(parse_hex);
    let pct = |k: &str| args.get(k).and_then(|v| v.as_f64()).unwrap_or(100.0) as f32;
    let (Some(from), Some(to)) = (get("from"), get("to")) else { return 0 };
    let from = with_alpha_pct(Color(from), pct("fromOpacity"));
    let to = with_alpha_pct(Color(to), pct("toOpacity"));
    with_engine(|e| {
        let roots = e.scene.outermost(&e.selection);
        e.step(|e| {
            let mut changed = 0;
            for root in roots {
                for id in e.scene.subtree(root) {
                    e.scene.update(id, |n| {
                        if n.fill == from {
                            n.fill = to;
                            changed += 1;
                        }
                        if let Some((c, _)) = n.stroke.as_mut()
                            && *c == from
                        {
                            *c = to;
                            changed += 1;
                        }
                    });
                }
            }
            changed
        })
    })
    .unwrap_or(0)
}

/// The selection handle under a screen point (-1 for none). See `handle_point` for numbering.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_handle_at(sx: f32, sy: f32) -> i32 {
    with_engine(|e| e.handle_at(sx, sy)).unwrap_or(-1)
}

/// Starts dragging a handle at (sx, sy); call inside `pergel_begin`/`pergel_commit`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_transform_begin(handle: u32, sx: f32, sy: f32) -> i32 {
    with_engine(|e| {
        let Some((rect, rotation)) = e.scene.selection_box(&e.selection) else { return 0 };
        let (rect, rotation) = match e.selected().as_slice() {
            [one] if one.kind == Kind::Group => (one.aabb(), 0.0),
            _ => (rect, rotation),
        };
        let nodes = e.movable().into_iter().filter_map(|id| e.scene.node(id).map(|n| (id, n.bounds, n.rotation))).collect();
        let (wx, wy) = e.world(sx, sy);
        let (cx, cy) = rect.center();
        let (lx, ly) = rotate_point(wx, wy, cx, cy, -rotation);
        let (hx, hy) = handle_point(&rect, handle.min(7));
        e.drag = Some(TransformDrag {
            handle,
            rect,
            rotation,
            nodes,
            grab: (hx - lx, hy - ly),
            start_angle: angle(cx, cy, wx, wy),
            applied: 0.0,
            scaled: None,
        });
        1
    })
    .unwrap_or(0)
}

/// Continues a handle drag. `flags`: 1 Shift (keep proportions / snap to 15°), 2 Alt (from centre),
/// 4 scale tool (K: proportional, and strokes, text and effects scale too).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_transform_to(sx: f32, sy: f32, flags: u32) {
    with_engine(|e| {
        if flags & 4 != 0
            && let Some(d) = e.drag.as_mut()
            && d.handle < 8
            && d.scaled.is_none()
        {
            d.scaled = Some(d.rect);
        }
        e.transform_to(sx, sy, flags & 1 != 0 || flags & 4 != 0, flags & 2 != 0)
    });
}

/// Scales the selection by `k` (Scale tool panel) about an anchor of its box: 0–8 row by row
/// from the top-left (4 is the centre).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_scale(k: f32, anchor: u32) {
    with_engine(|e| {
        if !(k > 0.0) || (k - 1.0).abs() < 1e-6 {
            return;
        }
        let ids = e.movable();
        let Some(b) = Rect::union_all(ids.iter().filter_map(|id| e.scene.node(*id).map(|n| n.aabb()))) else { return };
        let (ax, ay) = ((anchor % 3) as f32 / 2.0, (anchor.min(8) / 3) as f32 / 2.0);
        let (px, py) = (b.x + b.w * ax, b.y + b.h * ay);
        e.step(|e| {
            for id in &ids {
                e.scene.scale_about(*id, k, px, py);
                e.fit_text(*id);
            }
        });
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_transform_end() {
    with_engine(|e| {
        let Some(drag) = e.drag.take() else { return };
        for (id, _, _) in drag.nodes {
            let Some(n) = e.scene.node(id) else { continue };
            if n.rotation == 0.0 && n.kind != Kind::Text {
                let b = n.bounds;
                let r = Rect::new(b.x.round(), b.y.round(), b.w.round().max(1.0), b.h.round().max(1.0));
                e.scene.set_bounds(id, r);
            }
        }
    });
}

/// Align the selection: 0 left, 1 horizontal centres, 2 right, 3 top, 4 vertical centres,
/// 5 bottom; 6 distribute horizontal spacing, 7 distribute vertical spacing, 8 tidy up (rows
/// read from the current layout, laid out on an even grid). A single layer aligns to its parent frame.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_align(op: u32) {
    with_engine(|e| {
        let ids = e.movable();
        let boxes: Vec<(NodeId, Rect)> = ids.iter().filter_map(|id| e.scene.node(*id).map(|n| (*id, n.aabb()))).collect();
        let target = match boxes.as_slice() {
            [] => return,
            [(id, _)] => match e.scene.node(*id).and_then(|n| n.parent).and_then(|p| e.scene.node(p)) {
                Some(p) if p.kind == Kind::Frame => p.aabb(),
                _ => return,
            },
            many => match Rect::union_all(many.iter().map(|b| b.1)) {
                Some(r) => r,
                None => return,
            },
        };
        e.step(|e| {
            if op == 8 {
                tidy_up(e, &boxes, target);
                return;
            }
            if op == 9 {
                pack(e, &boxes, target);
                return;
            }
            if op >= 6 {
                let horizontal = op == 6;
                let mut sorted = boxes.clone();
                sorted.sort_by(|a, b| {
                    let (ka, kb) = if horizontal { (a.1.x, b.1.x) } else { (a.1.y, b.1.y) };
                    ka.total_cmp(&kb)
                });
                if sorted.len() < 3 {
                    return;
                }
                let total: f32 = sorted.iter().map(|b| if horizontal { b.1.w } else { b.1.h }).sum();
                let span = if horizontal { target.w } else { target.h };
                let gap = (span - total) / (sorted.len() - 1) as f32;
                let mut at = if horizontal { target.x } else { target.y };
                for (id, r) in sorted {
                    let (d, size) = if horizontal { (at - r.x, r.w) } else { (at - r.y, r.h) };
                    if horizontal { e.scene.translate(id, d, 0.0) } else { e.scene.translate(id, 0.0, d) }
                    at += size + gap;
                }
                return;
            }
            for (id, r) in &boxes {
                let (dx, dy) = match op {
                    0 => (target.x - r.x, 0.0),
                    1 => (target.center().0 - r.center().0, 0.0),
                    2 => (target.right() - r.right(), 0.0),
                    3 => (0.0, target.y - r.y),
                    4 => (0.0, target.center().1 - r.center().1),
                    _ => (0.0, target.bottom() - r.bottom()),
                };
                e.scene.translate(*id, dx, dy);
            }
        });
    });
}

/// Tidy up: groups the boxes into rows by their vertical centres, keeps the reading order and
/// re-lays them on a grid with one gap (the median of the current gaps, 16 when there is none).
fn tidy_up(e: &mut Engine, boxes: &[(NodeId, Rect)], target: Rect) {
    if boxes.len() < 2 {
        return;
    }
    let mut sorted = boxes.to_vec();
    sorted.sort_by(|a, b| a.1.center().1.total_cmp(&b.1.center().1));
    let mut rows: Vec<Vec<(NodeId, Rect)>> = Vec::new();
    for b in sorted {
        match rows.last_mut() {
            Some(row) if (b.1.center().1 - row[0].1.center().1).abs() < row.iter().map(|r| r.1.h).fold(b.1.h, f32::max) / 2.0 => row.push(b),
            _ => rows.push(vec![b]),
        }
    }
    for row in &mut rows {
        row.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
    }
    let mut gaps: Vec<f32> = rows.iter().flat_map(|r| r.windows(2).map(|w| w[1].1.x - w[0].1.right())).collect();
    if gaps.is_empty() {
        gaps = rows.windows(2).map(|w| w[1].iter().map(|b| b.1.y).fold(f32::MAX, f32::min) - w[0].iter().map(|b| b.1.bottom()).fold(f32::MIN, f32::max)).collect();
    }
    gaps.sort_by(f32::total_cmp);
    let gap = gaps.get(gaps.len() / 2).copied().filter(|g| *g >= 0.0).map(f32::round).unwrap_or(16.0);
    let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
    let widths: Vec<f32> = (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|b| b.1.w).fold(0.0, f32::max)).collect();
    let mut y = target.y;
    for row in &rows {
        let mut x = target.x;
        for (c, (id, r)) in row.iter().enumerate() {
            e.scene.translate(*id, x - r.x, y - r.y);
            x += widths[c] + gap;
        }
        y += row.iter().map(|b| b.1.h).fold(0.0, f32::max) + gap;
    }
}

/// Pack: the boxes in rows (shelves), tallest first, wrapped at about a square's width, so they
/// take as little room as possible; the gap is the selection's median spacing, at most 24.
fn pack(e: &mut Engine, boxes: &[(NodeId, Rect)], target: Rect) {
    if boxes.len() < 2 {
        return;
    }
    let mut gaps: Vec<f32> = boxes
        .iter()
        .filter_map(|(id, a)| {
            boxes
                .iter()
                .filter(|(o, _)| o != id)
                .map(|(_, b)| {
                    let dx = (b.x - a.right()).max(a.x - b.right());
                    let dy = (b.y - a.bottom()).max(a.y - b.bottom());
                    dx.max(dy)
                })
                .filter(|g| *g >= 0.0)
                .min_by(f32::total_cmp)
        })
        .collect();
    gaps.sort_by(f32::total_cmp);
    // Packing is tight: the current spacing, but never more than 24.
    let gap = gaps.get(gaps.len() / 2).copied().map_or(16.0, f32::round).clamp(0.0, 24.0);
    let mut sorted = boxes.to_vec();
    sorted.sort_by(|a, b| b.1.h.total_cmp(&a.1.h).then(b.1.w.total_cmp(&a.1.w)));
    let area: f32 = sorted.iter().map(|b| (b.1.w + gap) * (b.1.h + gap)).sum();
    let widest = sorted.iter().map(|b| b.1.w).fold(0.0, f32::max);
    let limit = area.sqrt().max(widest);
    let (mut x, mut y, mut shelf) = (target.x, target.y, 0.0f32);
    for (id, r) in sorted {
        if x > target.x && x + r.w > target.x + limit + 0.5 {
            x = target.x;
            y += shelf + gap;
            shelf = 0.0;
        }
        e.scene.translate(id, x - r.x, y - r.y);
        x += r.w + gap;
        shelf = shelf.max(r.h);
    }
}

/// Type on path: with a text layer and a vector selected, the text follows the vector's outline
/// (the vector becomes part of the text). With only text on a path selected, detaches it.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_text_on_path() -> i32 {
    with_engine(|e| {
        let sel = e.editable();
        let text = sel.iter().copied().find(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Text))?;
        let vector = sel.iter().copied().find(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Vector && n.vector.is_some()));
        match vector {
            Some(v) => {
                let vn = e.scene.node(v)?.clone();
                let path = vn.vector.as_deref()?.clone();
                e.step(|e| {
                    e.scene.update(text, |n| {
                        if let Some(t) = n.text.as_mut() {
                            t.path = Some(path.clone());
                            t.resize = TextResize::Fixed;
                        }
                        n.rotation = vn.rotation;
                    });
                    e.scene.set_bounds(text, vn.bounds);
                    e.scene.remove(v);
                    e.selection = vec![text];
                });
                Some(1)
            }
            None if e.scene.node(text)?.text.as_deref()?.path.is_some() => {
                e.step(|e| {
                    e.scene.update(text, |n| {
                        if let Some(t) = n.text.as_mut() {
                            t.path = None;
                            t.resize = TextResize::AutoWidth;
                        }
                    });
                    e.fit_text(text);
                });
                Some(2)
            }
            None => None,
        }
    })
    .flatten()
    .unwrap_or(0)
}

/// Join (Ctrl+J): the open paths of the selected vectors become one path, chained end to end
/// by their nearest endpoints (touching ends merge); one open path is closed instead.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_join() -> u32 {
    with_engine(|e| {
        let ids: Vec<NodeId> = e
            .paint_order(&e.editable())
            .into_iter()
            .filter(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Vector && n.vector.is_some()))
            .collect();
        let mut open: Vec<pergel_core::SubPath> = Vec::new();
        let mut closed: Vec<pergel_core::SubPath> = Vec::new();
        for id in &ids {
            let Some(n) = e.scene.node(*id) else { continue };
            let Some(v) = n.vector.as_deref() else { continue };
            for p in pergel_core::parse_path(&v.placed(n.bounds.x, n.bounds.y, n.bounds.w, n.bounds.h)) {
                if p.closed || p.anchors.len() < 2 {
                    closed.push(p);
                } else {
                    open.push(p);
                }
            }
        }
        if open.is_empty() {
            return 0;
        }
        let near = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
        let reverse = |p: &mut pergel_core::SubPath| {
            p.anchors.reverse();
            for a in &mut p.anchors {
                std::mem::swap(&mut a.cin, &mut a.cout);
            }
        };
        let mut chain = open.remove(0);
        while !open.is_empty() {
            let end = chain.anchors.last().map(|a| a.p).unwrap_or_default();
            let start = chain.anchors[0].p;
            // The closest remaining endpoint to either end of the chain.
            let mut best = (f32::MAX, 0usize, false, false);
            for (i, p) in open.iter().enumerate() {
                let (ps, pe) = (p.anchors[0].p, p.anchors.last().map(|a| a.p).unwrap_or_default());
                for (d, flip, at_start) in [(near(end, ps), false, false), (near(end, pe), true, false), (near(start, pe), false, true), (near(start, ps), true, true)] {
                    if d < best.0 {
                        best = (d, i, flip, at_start);
                    }
                }
            }
            let (d, i, flip, at_start) = best;
            let mut next = open.remove(i);
            if flip {
                reverse(&mut next);
            }
            if at_start {
                std::mem::swap(&mut chain, &mut next);
            }
            // `chain` now ends where `next` starts (or a straight segment joins them).
            if d < 0.5 {
                let first = next.anchors.remove(0);
                if let Some(last) = chain.anchors.last_mut() {
                    last.cout = first.cout;
                }
            }
            chain.anchors.extend(next.anchors);
        }
        let (s, t) = (chain.anchors[0].p, chain.anchors.last().map(|a| a.p).unwrap_or_default());
        if ids.len() == 1 || near(s, t) < 0.5 {
            if near(s, t) < 0.5 && chain.anchors.len() > 2 {
                let last = chain.anchors.pop().expect("anchor");
                chain.anchors[0].cin = last.cin;
            }
            chain.closed = true;
        }
        let mut subs = vec![chain];
        subs.extend(closed);
        let d = pergel_core::to_path(&subs);
        let Some(path) = skia_safe::Path::from_svg(&d) else { return 0 };
        let b = path.compute_tight_bounds();
        let bounds = Rect::new(b.left, b.top, b.width().max(0.01), b.height().max(0.01));
        let local = pergel_core::map_path(&d, |x, y| (x - b.left, y - b.top));
        let keep = ids[0];
        e.step(|e| {
            e.scene.update(keep, |n| {
                if let Some(v) = n.vector.as_deref_mut() {
                    v.d = local.clone();
                    v.size = (bounds.w, bounds.h);
                }
                n.rotation = 0.0;
            });
            e.scene.set_bounds(keep, bounds);
            for id in ids.iter().skip(1) {
                e.scene.remove(*id);
            }
            e.selection = vec![keep];
            ids.len() as u32
        })
    })
    .unwrap_or(0)
}

/// Flip horizontal (Shift+H) or vertical (Shift+V) about the selection's centre.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_flip(horizontal: i32) {
    with_engine(|e| {
        let ids = e.movable();
        let Some((px, py)) = e.pivot(&ids) else { return };
        e.step(|e| {
            for id in &ids {
                e.scene.flip(*id, horizontal != 0, if horizontal != 0 { px } else { py });
            }
        });
    });
}

/// Rotates the selection about its centre (90° buttons, Rotate 180°).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_rotate_by(degrees: f32) {
    with_engine(|e| {
        let ids = e.movable();
        let Some((px, py)) = e.pivot(&ids) else { return };
        e.step(|e| {
            for id in &ids {
                e.scene.rotate(*id, degrees, px, py);
            }
        });
    });
}

/// Swap fill and stroke colours (Shift+X).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_swap_fill_stroke() {
    with_engine(|e| {
        let ids = e.scene.outermost(&e.selection);
        e.step(|e| {
            for id in &ids {
                e.scene.update(*id, |n| {
                    let fill = n.fill_on.then_some(n.fill);
                    let width = n.stroke.map_or(1.0, |s| s.1);
                    let stroke = n.stroke.map(|s| s.0);
                    n.stroke = fill.map(|c| (c, width));
                    n.fill_on = stroke.is_some();
                    if let Some(c) = stroke {
                        n.fill = c;
                    }
                });
            }
        });
    });
}

/// Node ids arrive as strings ("12", or Figma-style "0:12").
fn parse_id(v: &serde_json::Value) -> Option<NodeId> {
    match v {
        serde_json::Value::Number(n) => n.as_u64().map(|n| n as NodeId),
        serde_json::Value::String(s) => s.rsplit([':', '-']).next()?.trim().parse().ok(),
        _ => None,
    }
}

fn arg_id(args: &serde_json::Value, key: &str) -> Option<NodeId> {
    args.get(key).and_then(parse_id)
}

fn arg_ids(args: &serde_json::Value, key: &str) -> Vec<NodeId> {
    args.get(key).and_then(|v| v.as_array()).map_or(Vec::new(), |a| a.iter().filter_map(parse_id).collect())
}

impl Engine {
    /// A node id from the arguments, else the first selected layer.
    fn target(&self, args: &serde_json::Value) -> Result<NodeId, String> {
        match arg_id(args, "nodeId") {
            Some(id) if self.scene.node(id).is_some() => Ok(id),
            Some(id) => Err(format!("No layer with id {id}. Call get_metadata to see the layer ids.")),
            None => self.selection.first().copied().ok_or_else(|| {
                "Nothing is selected in Pergel. Pass a nodeId (see get_metadata) or select a layer.".to_string()
            }),
        }
    }

    /// An asset for MCP: what it is, where it is used and (with `data`) its file as base64.
    fn asset_json(&self, a: &pergel_core::Asset, data: bool) -> serde_json::Value {
        let mut v = serde_json::json!({
            "file": a.file,
            "kind": a.kind,
            "name": a.name,
            "nodeIds": a.nodes.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
            "width": a.width,
            "height": a.height,
            "format": a.format,
            "mime": a.mime(),
            "background": a.background,
        });
        if data && let Some(bytes) = a.bytes(&self.scene) {
            v["data"] = serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(bytes));
        }
        v
    }

    fn summary(&self, id: NodeId) -> serde_json::Value {
        let Some(n) = self.scene.node(id) else { return serde_json::Value::Null };
        let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
        let parent = n.parent.and_then(|p| self.scene.node(p));
        let (ox, oy) = parent.filter(|p| p.kind != Kind::Group).map_or((0.0, 0.0), |p| (p.bounds.x, p.bounds.y));
        serde_json::json!({
            "id": id.to_string(),
            "name": n.name,
            "type": type_name(n),
            "x": b.x - ox, "y": b.y - oy, "width": b.w, "height": b.h,
            "parentId": n.parent.map(|p| p.to_string()),
        })
    }

    /// One prototype action from an agent's spec (`set_interactions`), nested ones included.
    fn parse_action(&self, id: NodeId, it: &serde_json::Value, screens: &[NodeId]) -> Result<pergel_core::Action, String> {
        let target = |v: Option<&serde_json::Value>, what: &str| -> Result<NodeId, String> {
            v.and_then(|v| match v {
                serde_json::Value::String(s) => parse_id(v).filter(|id| self.scene.node(*id).is_some()).or_else(|| {
                    screens.iter().copied().find(|sid| self.scene.node(*sid).is_some_and(|n| n.name.eq_ignore_ascii_case(s)))
                }),
                _ => parse_id(v),
            })
            .ok_or_else(|| format!("{what} {:?} not found; use a layer id or a top-level frame name", v))
        };
        let var = |v: Option<&serde_json::Value>| -> Result<String, String> {
            let name = v.and_then(|v| v.as_str()).ok_or("Pass `variable` (a variable name or id)")?;
            self.variables
                .variables
                .iter()
                .find(|x| x.id == name || x.name.eq_ignore_ascii_case(name))
                .map(|x| x.id.clone())
                .ok_or_else(|| format!("No variable {name:?} (see get_variable_defs)"))
        };
        let value = |var_id: &str, v: Option<&serde_json::Value>| -> Result<pergel_core::VarValue, String> {
            let v = v.ok_or("Pass `value`")?;
            if let Ok(x) = serde_json::from_value::<pergel_core::VarValue>(v.clone()) {
                return Ok(x);
            }
            let kind = self.variables.get(var_id).map(|x| x.kind);
            Ok(match (kind, v) {
                (_, serde_json::Value::Bool(b)) => pergel_core::VarValue::Boolean(*b),
                (_, serde_json::Value::Number(n)) => pergel_core::VarValue::Number(n.as_f64().unwrap_or(0.0) as f32),
                (Some(pergel_core::VarKind::Color), serde_json::Value::String(s)) => pergel_core::VarValue::Color(parse_color(s).ok_or("Bad colour")?),
                (_, serde_json::Value::String(s)) => pergel_core::VarValue::String(s.clone()),
                _ => return Err("Unsupported variable value".into()),
            })
        };
        let list = |v: Option<&serde_json::Value>| -> Result<Vec<pergel_core::Action>, String> {
            v.and_then(|v| v.as_array()).map(|a| a.iter().map(|x| self.parse_action(id, x, screens)).collect()).unwrap_or(Ok(Vec::new()))
        };
        Ok(match it.get("action").and_then(|v| v.as_str()).unwrap_or("navigate") {
            "back" => pergel_core::Action::Back,
            "url" | "open-url" => pergel_core::Action::OpenUrl { url: it.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string() },
            "overlay" | "open-overlay" => pergel_core::Action::OpenOverlay {
                to: target(it.get("to"), "Overlay")?,
                position: it.get("position").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default(),
                close_outside: it.get("closeOutside").and_then(|v| v.as_bool()).unwrap_or(true),
                dim: it.get("dim").and_then(|v| v.as_bool()).unwrap_or(false),
            },
            "close-overlay" | "close" => pergel_core::Action::CloseOverlay,
            "scroll-to" | "scroll" => pergel_core::Action::ScrollTo { to: target(it.get("to"), "Layer")? },
            "set-variable" | "set" => {
                let v = var(it.get("variable"))?;
                pergel_core::Action::SetVariable { value: value(&v, it.get("value"))?, variable: v }
            }
            "conditional" | "if" => {
                let v = var(it.get("variable"))?;
                pergel_core::Action::Conditional {
                    equals: value(&v, it.get("equals"))?,
                    then: list(it.get("then"))?,
                    otherwise: list(it.get("else").or_else(|| it.get("otherwise")))?,
                    variable: v,
                }
            }
            "change-to" | "change" | "variant" => {
                // A variant id, or property values ("State=Hover") of the layer's set.
                let main = self.scene.main_of(id).or(Some(id).filter(|i| self.scene.is_main(*i)));
                let set = main.and_then(|m| self.scene.variant_set_of(m));
                let to = it.get("to").and_then(|v| parse_id(v).filter(|t| self.scene.is_main(*t))).or_else(|| {
                    let wanted = it.get("to").and_then(|v| v.as_str())?;
                    let set = set?;
                    let mut props = pergel_core::parse_variant(&self.scene.node(main?)?.name);
                    for (k, v) in pergel_core::parse_variant(wanted) {
                        match props.iter_mut().find(|(pk, _)| *pk == k) {
                            Some(p) => p.1 = v,
                            None => props.push((k, v)),
                        }
                    }
                    self.scene.find_variant(set, &props)
                });
                let Some(to) = to else {
                    return Err("change-to needs `to`: a variant id or values like \"State=Hover\" of the layer's component set".into());
                };
                pergel_core::Action::ChangeTo { to }
            }
            _ => pergel_core::Action::Navigate { to: target(it.get("to"), "Destination")? },
        })
    }

    /// Edits one layer from an agent's patch (Figma-like names, positions relative to the parent).
    fn agent_update(&mut self, id: NodeId, args: &serde_json::Value) -> Result<(), String> {
        let node = self.scene.node(id).cloned().ok_or("No such layer")?;
        let parent = node.parent.and_then(|p| self.scene.node(p)).filter(|p| p.kind != Kind::Group);
        let (ox, oy) = parent.map_or((0.0, 0.0), |p| (p.bounds.x, p.bounds.y));
        let f = |k: &str| args.get(k).and_then(|v| v.as_f64()).map(|v| v as f32);
        let mut patch = Patch {
            x: f("x").map(|x| x + ox),
            y: f("y").map(|y| y + oy),
            w: f("width"),
            h: f("height"),
            rotation: f("rotation"),
            radius: f("radius"),
            fill: None,
            opacity: f("opacity").map(|o| if o <= 1.0 { o * 100.0 } else { o }),
            start_point: args.get("startPoint").and_then(|v| serde_json::from_value(v.clone()).ok()),
            end_point: args.get("endPoint").and_then(|v| serde_json::from_value(v.clone()).ok()),
            note: args.get("note").and_then(|v| v.as_str()).map(str::to_string),
            ready: args.get("readyForDev").and_then(|v| v.as_bool()),
            prop_values: args.get("properties").and_then(|v| v.as_object()).cloned(),
            scroll: args.get("scroll").and_then(|v| serde_json::from_value(v.clone()).ok()),
            ..Patch::default()
        };
        match args.get("fill").and_then(|v| v.as_str()) {
            Some("none") | Some("transparent") => patch.fill_on = Some(false),
            Some(c) => {
                let color = parse_color(c).ok_or_else(|| format!("Bad colour {c:?}; use #RRGGBB or #RRGGBBAA"))?;
                patch.fill = Some(format!("#{:06X}", color.argb() & 0xffffff));
                patch.fill_opacity = Some(((color.argb() >> 24) as f32 / 255.0 * 100.0).round());
            }
            None => {}
        }
        if let Some(st) = args.get("stroke") {
            if st.is_null() || st.as_str() == Some("none") {
                patch.stroke_on = Some(false);
            } else {
                patch.stroke_on = Some(true);
                if let Some(c) = st.get("color").and_then(|v| v.as_str()).and_then(parse_color) {
                    patch.stroke_color = Some(format!("#{:06X}", c.argb() & 0xffffff));
                    patch.stroke_opacity = Some(((c.argb() >> 24) as f32 / 255.0 * 100.0).round());
                }
                patch.stroke_width = st.get("width").and_then(|v| v.as_f64()).map(|v| v as f32);
                patch.stroke_align = st.get("align").and_then(|v| serde_json::from_value(v.clone()).ok());
            }
        }
        if let Some(l) = args.get("layout") {
            if l.is_null() {
                self.scene.update(id, |n| n.layout = None);
            } else {
                if node.layout.is_none() {
                    self.scene.update(id, |n| n.layout = Some(Box::default()));
                }
                let spec: pergel_core::NodeSpec =
                    serde_json::from_value(serde_json::json!({ "type": "frame", "layout": l })).map_err(|e| e.to_string())?;
                let mut scratch = Scene::default();
                let built = scratch.build_specs(&[spec], None, None);
                let layout = built.first().and_then(|b| scratch.node(*b)).and_then(|n| n.layout.clone());
                self.scene.update(id, |n| n.layout = layout);
            }
        }
        if let Some(sz) = args.get("sizing") {
            let parse = |k: &str| sz.get(k).and_then(|v| v.as_str()).and_then(|s| match s {
                "fixed" => Some(Sizing::Fixed),
                "hug" => Some(Sizing::Hug),
                "fill" => Some(Sizing::Fill),
                _ => None,
            });
            patch.sizing_h = parse("horizontal");
            patch.sizing_v = parse("vertical");
        }
        self.apply_patch(id, &patch);
        if let Some(name) = args.get("name").and_then(|v| v.as_str()) {
            let name = name.to_string();
            self.scene.update(id, |n| {
                n.name = name;
                if let Some(t) = n.text.as_mut() {
                    t.auto_name = false;
                }
            });
        }
        if let Some(v) = args.get("visible").and_then(|v| v.as_bool()) {
            self.scene.update(id, |n| n.visible = v);
        }
        if node.text.is_some() {
            let text = args.get("text").and_then(|v| v.as_str()).map(str::to_string);
            let family = args.get("fontFamily").and_then(|v| v.as_str()).map(str::to_string);
            let size = f("fontSize");
            let weight = args.get("fontWeight").and_then(|v| v.as_u64()).map(|w| w as u16);
            let line = args.get("lineHeight").and_then(parse_line_height);
            let spacing = f("letterSpacing");
            let align = args.get("textAlign").and_then(|v| v.as_str()).map(str::to_string);
            self.scene.update(id, |n| {
                let Some(t) = n.text.as_mut() else { return };
                if let Some(c) = text {
                    t.set_content(c);
                    if t.auto_name {
                        n.name = t.layer_name();
                    }
                }
                if let Some(v) = family {
                    t.family = v;
                }
                if let Some(v) = size {
                    t.size = v.max(1.0);
                }
                if let Some(v) = weight {
                    t.weight = v;
                }
                if let Some(v) = line {
                    t.line_height = v;
                }
                if let Some(v) = spacing {
                    t.letter_spacing = v;
                }
                if let Some(a) = align.and_then(|a| serde_json::from_value(serde_json::Value::String(a)).ok()) {
                    t.align = a;
                }
            });
            self.fit_text(id);
        }
        Ok(())
    }

    fn agent(&mut self, tool: &str, args: &serde_json::Value) -> Result<serde_json::Value, String> {
        use serde_json::json;
        match tool {
            "get_selection" => Ok(json!({ "selection": self.selection.iter().map(|id| self.summary(*id)).collect::<Vec<_>>() })),
            "get_metadata" => {
                let root = match arg_id(args, "nodeId") {
                    Some(id) if self.scene.node(id).is_some() => Some(id),
                    Some(id) => return Err(format!("No layer with id {id}")),
                    None => None,
                };
                let mut xml = self.scene.metadata_xml(root);
                if root.is_none() {
                    let name = self.pages.get(self.page).map_or("Page 1", |p| p.name.as_str());
                    xml = xml.replacen("<page name=\"Page 1\">", &format!("<page name=\"{}\">", name.replace('"', "&quot;")), 1);
                    if self.pages.len() > 1 {
                        let others: Vec<&str> = self.pages.iter().enumerate().filter(|(i, _)| *i != self.page).map(|(_, p)| p.name.as_str()).collect();
                        xml.push_str(&format!("<!-- Other pages (not shown): {} -->\n", others.join(", ")));
                    }
                }
                Ok(json!({ "xml": xml }))
            }
            "get_design_context" => {
                let id = self.target(args)?;
                let framework = Framework::parse(args.get("framework").and_then(|v| v.as_str()).unwrap_or("react"));
                let base = args.get("assetBase").and_then(|v| v.as_str()).unwrap_or("./assets/");
                let g = self.scene.generate_code_with(id, framework, base, &self.variables).ok_or("Could not generate code")?;
                let assets: Vec<serde_json::Value> = g.assets.iter().map(|a| self.asset_json(a, true)).collect();
                // Components used, with how often, so the agent builds each once and reuses it.
                let mut used: Vec<(NodeId, usize)> = Vec::new();
                for i in self.scene.subtree(id) {
                    if let Some(m) = self.scene.node(i).and_then(|n| match n.component {
                        Some(Component::Instance(m)) => Some(m),
                        _ => None,
                    }) {
                        match used.iter_mut().find(|(k, _)| *k == m) {
                            Some(e) => e.1 += 1,
                            None => used.push((m, 1)),
                        }
                    }
                }
                let components: Vec<serde_json::Value> = used
                    .iter()
                    .filter_map(|(m, count)| self.scene.node(*m).map(|n| json!({ "mainId": m.to_string(), "name": n.name, "uses": count })))
                    .collect();
                let texts: Vec<String> = self
                    .scene
                    .subtree(id)
                    .into_iter()
                    .filter_map(|i| self.scene.node(i).filter(|n| n.visible).and_then(|n| n.text.as_deref()).map(|t| t.content.clone()))
                    .collect();
                let variables: Vec<serde_json::Value> =
                    g.variables.iter().map(|(name, css, value)| json!({ "name": name, "css": css, "value": value })).collect();
                let interactions: Vec<serde_json::Value> = self
                    .scene
                    .subtree(id)
                    .into_iter()
                    .filter_map(|i| self.scene.node(i))
                    .flat_map(|n| n.reactions.iter().map(move |r| (n, r)))
                    .map(|(n, r)| {
                        let target = match &r.action {
                            pergel_core::Action::Navigate { to } => self.scene.node(*to).map_or("(missing)".to_string(), |t| format!("navigate to \"{}\" (id {})", t.name, to)),
                            pergel_core::Action::Back => "go back".to_string(),
                            pergel_core::Action::OpenUrl { url } => format!("open {url}"),
                            other => other.describe(
                                &|id| self.scene.node(id).map_or(format!("(missing {id})"), |t| format!("\"{}\" (id {id})", t.name)),
                                &|v| self.variables.get(v).map_or(v.to_string(), |x| x.name.clone()),
                            ),
                        };
                        json!({ "layer": n.name, "layerId": n.id.to_string(), "trigger": r.trigger, "action": target, "transition": r.transition, "duration": r.duration })
                    })
                    .collect();
                // Code Connect: components the code uses from the codebase.
                let mut connected: Vec<serde_json::Value> = Vec::new();
                for i in self.scene.subtree(id) {
                    let Some(main) = self.scene.main_of(i) else { continue };
                    let Some(link) = self.scene.code_link_of(main) else { continue };
                    if !connected.iter().any(|c| c["component"] == link.component.as_str()) {
                        connected.push(json!({ "component": link.component, "import": link.import, "source": link.source }));
                    }
                }
                let notes: Vec<serde_json::Value> = self
                    .scene
                    .subtree(id)
                    .into_iter()
                    .filter_map(|i| self.scene.node(i))
                    .filter_map(|n| n.note.as_deref().map(|t| json!({ "layer": n.name, "layerId": n.id.to_string(), "note": t })))
                    .collect();
                Ok(json!({
                    "annotations": notes,
                    "codeConnect": connected,
                    "interactions": interactions,
                    "variables": variables,
                    "node": self.summary(id),
                    "framework": if framework == Framework::Html { "html" } else { "react" },
                    "code": g.code,
                    "fonts": g.fonts,
                    "components": components,
                    "assets": assets,
                    "texts": texts,
                    "metadata": self.scene.metadata_xml(Some(id)),
                    "tokens": self.scene.design_tokens(Some(id)),
                }))
            }
            "get_comments" => {
                let include = args.get("includeResolved").and_then(|v| v.as_bool()).unwrap_or(false);
                let list: Vec<serde_json::Value> = self.comment_list().into_iter().filter(|c| include || c["resolved"] != json!(true)).collect();
                Ok(json!({ "comments": list, "hint": "Reply with add_comment { threadId, text }, mark done with resolve_comment. `node` is the layer a thread is pinned to." }))
            }
            "add_comment" => {
                let text = args.get("text").and_then(|v| v.as_str()).ok_or("Pass `text`")?.to_string();
                let author = args.get("author").and_then(|v| v.as_str()).unwrap_or("Claude").to_string();
                let req = match args.get("threadId").and_then(|v| v.as_str()) {
                    Some(t) => json!({ "op": "reply", "id": t, "text": text, "author": author }),
                    None => {
                        // On a layer (its top-right corner) or at a canvas point.
                        let (x, y) = match arg_id(args, "nodeId").and_then(|id| self.scene.node(id)) {
                            Some(n) => (n.bounds.right() - 8.0, n.bounds.y + 8.0),
                            None => (
                                args.get("x").and_then(|v| v.as_f64()).ok_or("Pass threadId, nodeId or x and y")? as f32,
                                args.get("y").and_then(|v| v.as_f64()).ok_or("Pass threadId, nodeId or x and y")? as f32,
                            ),
                        };
                        json!({ "op": "add", "x": x, "y": y, "text": text, "author": author, "nodeId": args.get("nodeId").and_then(|v| v.as_str()) })
                    }
                };
                let id = self.comment_op(&req).ok_or("No such thread")?;
                Ok(json!({ "threadId": id }))
            }
            "resolve_comment" => {
                let id = args.get("threadId").and_then(|v| v.as_str()).ok_or("Pass `threadId`")?;
                let resolved = args.get("resolved").and_then(|v| v.as_bool()).unwrap_or(true);
                self.comment_op(&json!({ "op": "resolve", "id": id, "resolved": resolved })).ok_or("No such thread")?;
                Ok(json!({ "threadId": id, "resolved": resolved }))
            }
            "get_pages" => {
                let pages: Vec<serde_json::Value> = self
                    .pages
                    .iter()
                    .enumerate()
                    .map(|(i, p)| {
                        let layers = if i == self.page { self.scene.roots().len() } else { p.scene.roots().len() };
                        json!({ "index": i, "name": p.name, "current": i == self.page, "topLevelLayers": layers })
                    })
                    .collect();
                Ok(json!({ "pages": pages }))
            }
            "switch_page" => {
                let by_name = args.get("name").and_then(|v| v.as_str()).and_then(|n| self.pages.iter().position(|p| p.name.eq_ignore_ascii_case(n.trim())));
                let i = by_name
                    .or_else(|| args.get("index").and_then(|v| v.as_u64()).map(|i| i as usize))
                    .filter(|i| *i < self.pages.len())
                    .ok_or("No such page; get_pages lists them")?;
                self.switch_page(i);
                Ok(json!({ "page": self.pages[i].name, "index": i }))
            }
            "set_variant" => {
                let ids: Vec<NodeId> = {
                    let mut v = arg_ids(args, "nodeIds");
                    v.extend(arg_id(args, "nodeId"));
                    v
                };
                let props = args.get("properties").and_then(|v| v.as_object()).cloned().unwrap_or_default();
                if ids.is_empty() || props.is_empty() {
                    return Err("Pass nodeIds (instances) and properties, e.g. {\"State\": \"Hover\"}".into());
                }
                let mut current = ids.clone();
                for (k, v) in props {
                    let Some(v) = v.as_str() else { continue };
                    if self.set_variant(&current, &k, v) > 0 {
                        current = self.selection.clone();
                    }
                }
                Ok(json!({ "instances": current.iter().map(|id| self.summary(*id)).collect::<Vec<_>>() }))
            }
            "set_interactions" => {
                let id = self.target(args)?;
                let list = args.get("interactions").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let screens = self.scene.screens();
                let mut out = Vec::new();
                for it in &list {
                    let trigger = match it.get("trigger").and_then(|v| v.as_str()).unwrap_or("click") {
                        "hover" | "on-hover" => pergel_core::Trigger::OnHover,
                        t if t.starts_with("after") || t == "delay" => pergel_core::Trigger::AfterDelay { ms: it.get("delay").and_then(|v| v.as_u64()).unwrap_or(800) as u32 },
                        _ => pergel_core::Trigger::OnClick,
                    };
                    let action = self.parse_action(id, it, &screens)?;
                    let transition = serde_json::from_value(it.get("transition").cloned().unwrap_or(json!("instant"))).unwrap_or_default();
                    let duration = it.get("duration").and_then(|v| v.as_u64()).unwrap_or(300) as u32;
                    out.push(pergel_core::Reaction { trigger, action, transition, duration });
                }
                let n = out.len();
                self.step(|e| e.scene.update(id, |node| node.reactions = out));
                Ok(json!({ "nodeId": id.to_string(), "interactions": n }))
            }
            "add_code_connect_map" => {
                let id = arg_id(args, "nodeId").filter(|id| self.scene.node(*id).is_some()).ok_or("nodeId must be a main component or component set (see get_components)")?;
                let n = self.scene.node(id).ok_or("No such layer")?;
                if !(n.variant_set || n.component == Some(Component::Main)) {
                    return Err("Code Connect maps main components or component sets; pass the main's id".into());
                }
                let link: pergel_core::CodeLink = serde_json::from_value(args.clone()).map_err(|e| format!("Bad mapping: {e}"))?;
                if link.component.trim().is_empty() {
                    return Err("Pass component (its name in code, e.g. \"Button\")".into());
                }
                self.step(|e| e.scene.update(id, |n| n.code = Some(Box::new(link))));
                Ok(json!({ "mapped": id.to_string() }))
            }
            "get_code_connect_map" => {
                let list: Vec<serde_json::Value> = self
                    .scene
                    .nodes()
                    .filter_map(|n| n.code.as_deref().map(|c| (n, c)))
                    .map(|(n, c)| json!({ "nodeId": n.id.to_string(), "name": n.name, "component": c.component, "import": c.import, "source": c.source, "props": c.props, "text": c.text }))
                    .collect();
                Ok(json!({ "mappings": list }))
            }
            "get_node_details" => {
                // Everything the Design panel knows about a layer (plugins read node properties).
                let id = self.target(args)?;
                let n = self.scene.node(id).ok_or("No such layer")?;
                let mut v = serde_json::to_value(NodeInfo::from(n)).map_err(|e| e.to_string())?;
                let s = self.summary(id);
                v["relX"] = s["x"].clone();
                v["relY"] = s["y"].clone();
                v["type"] = s["type"].clone();
                v["children"] = json!(n.children.iter().map(|c| c.to_string()).collect::<Vec<_>>());
                v["mainId"] = json!(self.scene.main_of(id).map(|m| m.to_string()));
                Ok(v)
            }
            "list_children" => {
                let ids: Vec<String> = match arg_id(args, "nodeId") {
                    Some(p) => self.scene.node(p).ok_or("No such layer")?.children.iter().map(|c| c.to_string()).collect(),
                    None => self.scene.roots().iter().map(|c| c.to_string()).collect(),
                };
                Ok(json!({ "children": ids }))
            }
            "move_node" => {
                let id = self.target(args)?;
                let parent = arg_id(args, "parentId");
                let index = args.get("index").and_then(|v| v.as_u64()).map(|i| i as usize);
                if self.scene.in_instance(id) {
                    return Err("Layers inside an instance can't be moved".into());
                }
                let ok = self.step(|e| e.scene.reparent(id, parent, index));
                if !ok {
                    return Err("That layer can't go there (a frame or group, outside instances)".into());
                }
                Ok(json!({ "moved": self.summary(id) }))
            }
            "clone_node" => {
                let id = self.target(args)?;
                let tree = self.scene.extract(id).ok_or("No such layer")?;
                let (parent, index) = self.scene.position(id).ok_or("No such layer")?;
                let new = self.step(|e| {
                    let new = e.scene.insert_tree(&tree, parent, Some(index + 1));
                    if let Some(pergel_core::Component::Main) = e.scene.node(id).and_then(|n| n.component) {
                        // A clone of a main component is an instance of it, as in Figma.
                        e.scene.relink_instance(new, id);
                    }
                    new
                });
                Ok(json!({ "created": self.summary(new) }))
            }
            "get_node" => {
                let id = self.target(args)?;
                let n = self.scene.node(id).ok_or("No such layer")?;
                let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
                let mut v = self.summary(id);
                v["absX"] = json!(b.x);
                v["absY"] = json!(b.y);
                Ok(v)
            }
            // The deepest visible layers under world points, for compare_to_design.
            "layers_at" => {
                let points = args.get("points").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let root = arg_id(args, "nodeId");
                let inside: Option<std::collections::HashSet<NodeId>> = root.map(|r| self.scene.subtree(r).into_iter().collect());
                let mut out = Vec::new();
                for p in points {
                    let (x, y) = (p[0].as_f64().unwrap_or(0.0) as f32, p[1].as_f64().unwrap_or(0.0) as f32);
                    let point = Rect::new(x, y, 0.0, 0.0);
                    let hit = self
                        .scene
                        .visible(&point)
                        .filter(|n| n.kind != Kind::Group && n.contains(x, y) && inside.as_ref().is_none_or(|s| s.contains(&n.id)))
                        .filter(|n| self.scene.reach(n.id).is_some_and(|clip| clip.iter().all(|c| c.contains_point(x, y))))
                        .last()
                        .map(|n| json!({ "id": n.id.to_string(), "name": n.name, "type": type_name(n) }));
                    out.push(hit.unwrap_or(serde_json::Value::Null));
                }
                Ok(json!({ "layers": out }))
            }
            "get_assets" => {
                let id = self.target(args)?;
                let assets: Vec<serde_json::Value> = self.scene.find_assets(id).iter().map(|a| self.asset_json(a, true)).collect();
                Ok(json!({ "node": self.summary(id), "assets": assets }))
            }
            "get_variable_defs" => {
                let page = args.get("page").and_then(|v| v.as_bool()).unwrap_or(false);
                let root = if page { None } else { arg_id(args, "nodeId").or_else(|| self.selection.first().copied()) };
                let mut tokens = self.scene.design_tokens(root);
                // Variables: the ones the layer uses, or all of them for the page.
                let used: std::collections::BTreeSet<String> = match root {
                    Some(r) => self
                        .scene
                        .subtree(r)
                        .into_iter()
                        .filter_map(|i| self.scene.node(i).and_then(|n| n.bindings.as_deref()).cloned())
                        .flat_map(|b| b.into_values())
                        .collect(),
                    None => self.variables.variables.iter().map(|v| v.id.clone()).collect(),
                };
                let mut defs = serde_json::Map::new();
                for v in self.variables.variables.iter().filter(|v| used.contains(&v.id)) {
                    if let Some(value) = self.variables.resolve(&v.id) {
                        defs.insert(v.name.clone(), json!(pergel_core::Variables::display(&value)));
                    }
                }
                tokens["variables"] = serde_json::Value::Object(defs);
                tokens["collections"] = json!(self
                    .variables
                    .collections
                    .iter()
                    .map(|c| json!({ "name": c.name, "modes": c.modes.iter().map(|m| m.name.clone()).collect::<Vec<_>>(),
                        "activeMode": c.modes.iter().find(|m| m.id == c.active).map(|m| m.name.clone()) }))
                    .collect::<Vec<_>>());
                let used_styles: std::collections::BTreeSet<String> = match root {
                    Some(r) => self
                        .scene
                        .subtree(r)
                        .into_iter()
                        .filter_map(|i| self.scene.node(i).and_then(|n| n.styles.as_deref()).cloned())
                        .flat_map(|l| [l.text, l.fill, l.stroke, l.effect].into_iter().flatten())
                        .collect(),
                    None => self.styles.list.iter().map(|s| s.id.clone()).collect(),
                };
                let mut styles = serde_json::Map::new();
                for s in self.styles.list.iter().filter(|s| used_styles.contains(&s.id)) {
                    let value = match &s.value {
                        pergel_core::StyleValue::Text(t) => json!(t.summary()),
                        pergel_core::StyleValue::Color { color, .. } => json!(pergel_core::Variables::display(&pergel_core::VarValue::Color(*color))),
                        pergel_core::StyleValue::Effect { effects } => json!(format!("{} effect(s)", effects.len())),
                    };
                    styles.insert(s.name.clone(), value);
                }
                tokens["styles"] = serde_json::Value::Object(styles);
                tokens["note"] = json!("variables: name → value in the active mode; styles: text/colour/effect styles by name (map them to the project's tokens and typography classes). colors/typography/...: raw values with use counts.");
                Ok(tokens)
            }
            "set_variables" => {
                let collection = args.get("collection").and_then(|v| v.as_str()).unwrap_or("Tokens").to_string();
                let list = args.get("variables").and_then(|v| v.as_array()).cloned().unwrap_or_default();
                let parse = |v: &serde_json::Value, vars: &pergel_core::Variables| -> Option<pergel_core::VarValue> {
                    match v {
                        serde_json::Value::Bool(b) => Some(pergel_core::VarValue::Boolean(*b)),
                        serde_json::Value::Number(n) => Some(pergel_core::VarValue::Number(n.as_f64()? as f32)),
                        serde_json::Value::String(s) if s.starts_with('{') && s.ends_with('}') => {
                            vars.by_name(&s[1..s.len() - 1]).map(|t| pergel_core::VarValue::Alias(t.id.clone()))
                        }
                        serde_json::Value::String(s) => Some(match parse_color(s) {
                            Some(c) if s.starts_with('#') => pergel_core::VarValue::Color(c),
                            _ => pergel_core::VarValue::String(s.clone()),
                        }),
                        _ => None,
                    }
                };
                let mut names = Vec::new();
                for item in &list {
                    let Some(name) = item.get("name").and_then(|v| v.as_str()) else { continue };
                    let mut per_mode: Vec<(String, pergel_core::VarValue)> = Vec::new();
                    if let Some(modes) = item.get("modes").and_then(|v| v.as_object()) {
                        for (mode, v) in modes {
                            if let Some(val) = parse(v, &self.variables) {
                                per_mode.push((mode.clone(), val));
                            }
                        }
                    }
                    let value = item.get("value").and_then(|v| parse(v, &self.variables)).or_else(|| per_mode.first().map(|m| m.1.clone()));
                    let Some(value) = value else { continue };
                    self.variables.set(&collection, name, value, &per_mode);
                    names.push(name.to_string());
                }
                if let Some(mode) = args.get("activeMode").and_then(|v| v.as_str())
                    && let Some(c) = self.variables.collections.iter_mut().find(|c| c.name == collection)
                    && let Some(m) = c.modes.iter().find(|m| m.name == mode)
                {
                    c.active = m.id.clone();
                }
                self.page_rev += 1;
                let updated = self.apply_variables();
                Ok(json!({ "set": names, "layersUpdated": updated }))
            }
            "bind_variable" => {
                let ids: Vec<NodeId> = {
                    let mut v = arg_ids(args, "nodeIds");
                    v.extend(arg_id(args, "nodeId"));
                    v
                };
                let prop: pergel_core::BindProp = serde_json::from_value(args.get("property").cloned().unwrap_or_default())
                    .map_err(|_| "property must be one of fill, stroke, strokeWidth, radius, opacity, gap, padding, width, height, fontSize, text, visible")?;
                let var = match args.get("variable").and_then(|v| v.as_str()) {
                    Some(name) => Some(self.variables.by_name(name).map(|v| v.id.clone()).ok_or_else(|| format!("No variable named {name:?}; create it with set_variables"))?),
                    None => None,
                };
                let vars = self.variables.clone();
                let n = self.step(|e| {
                    let mut n = 0;
                    for id in &ids {
                        if e.scene.node(*id).is_none() {
                            continue;
                        }
                        let var = var.clone();
                        e.scene.update(*id, |node| {
                            let mut b = node.bindings.take().map(|b| *b).unwrap_or_default();
                            match var {
                                Some(v) => {
                                    b.insert(prop, v);
                                }
                                None => {
                                    b.remove(&prop);
                                }
                            }
                            node.bindings = (!b.is_empty()).then(|| Box::new(b));
                        });
                        n += 1;
                    }
                    e.scene.apply_variables(&vars);
                    n
                });
                Ok(json!({ "bound": n }))
            }
            "generate_diagram" => {
                let src = args.get("mermaid").or_else(|| args.get("source")).and_then(|v| v.as_str()).ok_or("Pass the flowchart as `mermaid`")?;
                let name = args.get("name").and_then(|v| v.as_str()).unwrap_or("Diagram");
                let mut spec = pergel_core::mermaid_spec(src, name)?;
                // Next to what is already on the page unless a position is given.
                let page = self.scene.bounds();
                let empty = self.scene.roots().is_empty();
                spec.x = args.get("x").and_then(|v| v.as_f64()).map_or(if empty { 0.0 } else { page.right() + 120.0 }, |v| v as f32);
                spec.y = args.get("y").and_then(|v| v.as_f64()).map_or(if empty { 0.0 } else { page.y }, |v| v as f32);
                let (ids, warnings) = self.step(|e| {
                    let (ids, warnings) = e.scene.build_specs_reporting(std::slice::from_ref(&spec), None, None);
                    let texts: Vec<NodeId> = ids
                        .iter()
                        .flat_map(|id| e.scene.subtree(*id))
                        .filter(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Text))
                        .collect();
                    for t in texts {
                        e.fit_text(t);
                    }
                    e.selection = ids.clone();
                    (ids, warnings)
                });
                if let Some(b) = self.selection_bounds() {
                    self.view.fit(b, FIT_MARGIN);
                }
                let root = ids.first().copied();
                Ok(json!({
                    "created": ids.iter().map(|id| self.summary(*id)).collect::<Vec<_>>(),
                    "layers": root.map_or(0, |r| self.scene.subtree(r).len()),
                    "warnings": warnings,
                    "hint": "Restyle with update_node (each shape is a frame with a text child; arrows are line layers with points). get_screenshot to check it.",
                }))
            }
            "search_design_system" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("").to_lowercase();
                let words: Vec<&str> = query.split(|c: char| c.is_whitespace() || c == '/' || c == '=' || c == ',').filter(|w| !w.is_empty()).collect();
                let wants = |kind: &str| match args.get("types").and_then(|v| v.as_array()) {
                    Some(list) if !list.is_empty() => list.iter().any(|t| t.as_str().is_some_and(|t| t.trim_end_matches('s') == kind)),
                    _ => true,
                };
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(20).clamp(1, 100) as usize;
                // Every word must appear; whole-name and prefix matches rank first.
                let score = |name: &str, extra: &str| -> Option<u32> {
                    let hay = format!("{} {}", name.to_lowercase(), extra.to_lowercase());
                    if words.is_empty() {
                        return Some(1);
                    }
                    if !words.iter().all(|w| hay.contains(w)) {
                        return None;
                    }
                    let n = name.to_lowercase();
                    Some(if n == query { 100 } else if n.starts_with(&query) { 50 } else if n.contains(&query) { 20 } else { 10 })
                };
                let mut components: Vec<(u32, serde_json::Value)> = Vec::new();
                if wants("component") {
                    for (pi, slot) in self.pages.iter().enumerate() {
                        let scene = if pi == self.page { &self.scene } else { &slot.scene };
                        let page = slot.name.clone();
                        for id in scene.mains() {
                            let Some(n) = scene.node(id) else { continue };
                            let set = scene.variant_set_of(id).and_then(|s| scene.node(s));
                            let note = n.note.as_deref().map_or("", |s| s.as_str());
                            let Some(sc) = score(&n.name, &format!("{} {note}", set.map_or("", |s| s.name.as_str()))) else { continue };
                            let mut v = json!({
                                "id": id.to_string(), "name": n.name, "page": page, "width": n.bounds.w, "height": n.bounds.h,
                                "instances": scene.instances_of(id).len(),
                            });
                            if let Some(s) = set {
                                v["componentSet"] = json!(s.name);
                                v["variant"] = json!(pergel_core::parse_variant(&n.name).into_iter().collect::<std::collections::BTreeMap<_, _>>());
                            }
                            if let Some(c) = scene.code_link_of(id) {
                                v["codeConnect"] = json!({ "component": c.component, "import": c.import });
                            }
                            if pi != self.page {
                                v["note"] = json!("On another page: switch_page to it before create_instance");
                            }
                            components.push((sc + if pi == self.page { 1 } else { 0 }, v));
                        }
                    }
                }
                let mut styles: Vec<(u32, serde_json::Value)> = Vec::new();
                if wants("style") {
                    for s in &self.styles.list {
                        if let Some(sc) = score(&s.name, &s.description) {
                            let (kind, value) = match &s.value {
                                pergel_core::StyleValue::Text(t) => ("text", json!(t.summary())),
                                pergel_core::StyleValue::Color { color, paint } => ("color", if paint.is_some() { json!("gradient/image") } else { json!(hex(*color)) }),
                                pergel_core::StyleValue::Effect { effects } => ("effect", json!(effects)),
                            };
                            styles.push((sc, json!({ "id": s.id, "name": s.name, "type": kind, "value": value, "description": s.description })));
                        }
                    }
                }
                let mut variables: Vec<(u32, serde_json::Value)> = Vec::new();
                if wants("variable") {
                    for v in &self.variables.variables {
                        let collection = self.variables.collections.iter().find(|c| c.id == v.collection).map_or(v.collection.as_str(), |c| c.name.as_str());
                        if let Some(sc) = score(&v.name, &format!("{collection} {}", v.description)) {
                            let value = self.variables.resolve(&v.id).map(|x| pergel_core::Variables::display(&x));
                            variables.push((sc, json!({ "name": v.name, "collection": collection, "type": v.kind, "value": value, "css": format!("var({})", pergel_core::Variables::css_name(&v.name)) })));
                        }
                    }
                }
                let top = |mut list: Vec<(u32, serde_json::Value)>| {
                    list.sort_by(|a, b| b.0.cmp(&a.0));
                    list.into_iter().take(limit).map(|x| x.1).collect::<Vec<_>>()
                };
                Ok(json!({
                    "components": top(components),
                    "styles": top(styles),
                    "variables": top(variables),
                    "hint": "Use components with create_instance (or set_variant on an instance), bind variables with bind_variable or a spec's bind, and search_icons for icons that are not in the file.",
                }))
            }
            "get_components" => {
                let list: Vec<serde_json::Value> = self
                    .scene
                    .mains()
                    .into_iter()
                    .map(|id| {
                        let mut v = self.summary(id);
                        v["instances"] = json!(self.scene.instances_of(id).len());
                        if let Some(set) = self.scene.variant_set_of(id) {
                            v["componentSet"] = json!(self.scene.node(set).map(|s| s.name.clone()));
                            v["componentSetId"] = json!(set.to_string());
                            v["variant"] = json!(self.scene.node(id).map(|n| pergel_core::parse_variant(&n.name).into_iter().collect::<std::collections::BTreeMap<_, _>>()));
                        }
                        v
                    })
                    .collect();
                let sets: Vec<serde_json::Value> = self
                    .scene
                    .nodes()
                    .filter(|n| n.variant_set)
                    .map(|n| {
                        let props: serde_json::Map<String, serde_json::Value> =
                            self.scene.variant_props(n.id).into_iter().map(|(k, v)| (k, json!(v))).collect();
                        json!({ "id": n.id.to_string(), "name": n.name, "properties": props })
                    })
                    .collect();
                Ok(json!({ "components": list, "componentSets": sets }))
            }
            "create_nodes" => {
                let specs: Vec<NodeSpec> = serde_json::from_value(args.get("nodes").cloned().unwrap_or_default())
                    .map_err(|e| format!("Bad node specs: {e}"))?;
                if specs.is_empty() {
                    return Err("Pass at least one node in `nodes`".into());
                }
                let parent = arg_id(args, "parentId");
                if let Some(p) = parent
                    && (self.scene.node(p).is_none_or(|n| !n.kind.is_container()) || self.scene.in_instance(p) || self.scene.is_instance(p))
                {
                    return Err(format!("Layer {p} can't take children (use a frame or group outside instances)"));
                }
                let (ids, warnings) = self.step(|e| {
                    let (ids, warnings) = e.scene.build_specs_reporting(&specs, parent, None);
                    e.link_named_bindings(&ids);
                    let vars = e.variables.clone();
                    e.scene.apply_variables(&vars);
                    let texts: Vec<NodeId> = ids
                        .iter()
                        .flat_map(|id| e.scene.subtree(*id))
                        .filter(|id| e.scene.node(*id).is_some_and(|n| n.kind == Kind::Text))
                        .collect();
                    for t in texts {
                        e.fit_text(t);
                    }
                    e.selection = ids.clone();
                    (ids, warnings)
                });
                Ok(json!({ "created": ids.iter().map(|id| self.summary(*id)).collect::<Vec<_>>(), "warnings": warnings }))
            }
            "update_node" => {
                let id = self.target(args)?;
                if self.scene.in_instance(id) && ["x", "y", "width", "height"].iter().any(|k| args.get(*k).is_some()) {
                    return Err("Layers inside an instance can't be moved or resized; edit the main component".into());
                }
                let result = self.step(|e| e.agent_update(id, args));
                result.map(|_| json!({ "updated": self.summary(id) }))
            }
            "delete_nodes" => {
                let ids: Vec<NodeId> =
                    arg_ids(args, "nodeIds").into_iter().filter(|id| !self.scene.in_instance(*id)).collect();
                let n = self.step(|e| {
                    let ids = e.scene.outermost(&ids);
                    ids.into_iter().filter(|id| e.scene.remove(*id)).count()
                });
                Ok(json!({ "deleted": n }))
            }
            "select_nodes" => {
                let ids: Vec<NodeId> = arg_ids(args, "nodeIds").into_iter().filter(|id| self.scene.node(*id).is_some()).collect();
                self.selection = ids.clone();
                if args.get("zoom").and_then(|v| v.as_bool()).unwrap_or(true)
                    && let Some(b) = self.selection_bounds()
                {
                    self.view.fit(b, FIT_MARGIN);
                }
                Ok(json!({ "selection": ids.iter().map(|id| self.summary(*id)).collect::<Vec<_>>() }))
            }
            "create_instance" => {
                let main = arg_id(args, "componentId").filter(|m| self.scene.is_main(*m)).ok_or("componentId must be a main component (see get_components)")?;
                let parent = arg_id(args, "parentId");
                let (ox, oy) = parent.and_then(|p| self.scene.node(p)).map_or((0.0, 0.0), |p| (p.bounds.x, p.bounds.y));
                let size = self.scene.node(main).map(|n| (n.bounds.w, n.bounds.h)).unwrap_or_default();
                let x = args.get("x").and_then(|v| v.as_f64()).map(|v| v as f32 + ox + size.0 / 2.0);
                let y = args.get("y").and_then(|v| v.as_f64()).map(|v| v as f32 + oy + size.1 / 2.0);
                let at = match (x, y) {
                    (Some(x), Some(y)) => (x, y),
                    _ => self.view.world_area().center(),
                };
                let values: std::collections::BTreeMap<String, serde_json::Value> =
                    args.get("properties").and_then(|v| v.as_object()).map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect()).unwrap_or_default();
                let mut unknown = Vec::new();
                let id = self.step(|e| {
                    let id = e.scene.create_instance(main, parent, None, at);
                    if let Some(i) = id
                        && !values.is_empty()
                    {
                        unknown = e.scene.set_prop_values_by_name(i, &values);
                    }
                    e.selection = id.into_iter().collect();
                    id
                });
                if !unknown.is_empty() {
                    return id.map(|id| json!({ "created": self.summary(id), "warnings": [format!("Unknown properties: {}", unknown.join(", "))] })).ok_or_else(|| "Could not create the instance".into());
                }
                id.map(|id| json!({ "created": self.summary(id) })).ok_or_else(|| "Could not create the instance".into())
            }
            other => Err(format!("Unknown tool {other}")),
        }
    }
}

/// Runs an MCP tool. The text buffer holds `{"tool": …, "args": {…}}`; returns
/// `{"ok": true, "result": …}` or `{"ok": false, "error": "…"}`.
#[unsafe(no_mangle)]
pub extern "C" fn pergel_agent() -> *const c_char {
    let request: serde_json::Value = serde_json::from_str(&text()).unwrap_or_default();
    let tool = request.get("tool").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let args = request.get("args").cloned().unwrap_or_else(|| serde_json::json!({}));
    emit(with_engine(|e| {
        let reply = match e.agent(&tool, &args) {
            Ok(result) => serde_json::json!({ "ok": true, "result": result }),
            Err(error) => serde_json::json!({ "ok": false, "error": error }),
        };
        reply.to_string()
    }))
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_text_buffer(len: usize) -> *mut u8 {
    TEXT.with_borrow_mut(|buf| {
        buf.clear();
        buf.resize(len, 0);
        buf.as_mut_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_rename(id: u32) -> i32 {
    let name = text().trim().to_string();
    if name.is_empty() {
        return 0;
    }
    with_engine(|e| {
        e.step(|e| {
            e.scene.update(id, |n| {
                n.name = name;
                if let Some(t) = n.text.as_mut() {
                    t.auto_name = false;
                }
            }) as i32
        })
    })
    .unwrap_or(0)
}

#[derive(Serialize)]
struct StrokeInfo {
    color: String,
    opacity: f32,
    width: f32,
    align: StrokeAlign,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NodeInfo<'a> {
    id: NodeId,
    name: &'a str,
    kind: &'static str,
    parent: i64,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rotation: f32,
    radius: f32,
    corners: Option<[f32; 4]>,
    fill: String,
    fill_opacity: f32,
    fill_on: bool,
    stroke: Option<StrokeInfo>,
    /// Layer opacity in percent.
    opacity: f32,
    blend: Blend,
    lock_ratio: bool,
    layout: Option<&'a AutoLayout>,
    sizing: [Sizing; 2],
    absolute: bool,
    /// The flow of the auto-layout parent placing this layer, if any.
    parent_flow: Option<Flow>,
    effects: Vec<EffectInfo>,
    exports: &'a [ExportSetting],
    /// "main", "instance" or null.
    component: Option<&'static str>,
    main: Option<NodeId>,
    main_name: Option<String>,
    in_instance: bool,
    overrides: u16,
    visible: bool,
    locked: bool,
    clip: bool,
    children: usize,
    text: Option<TextInfo<'a>>,
    paint: Option<&'a Paint>,
    mask: bool,
    bindings: Option<&'a pergel_core::Bindings>,
    constraints: [pergel_core::Constraint; 2],
    styles: Option<&'a pergel_core::StyleLinks>,
    variant_set: bool,
    /// For instances of a variant: each property with its values and the current one.
    variants: Option<Vec<serde_json::Value>>,
    reactions: &'a [pergel_core::Reaction],
    code: Option<&'a pergel_core::CodeLink>,
    grids: &'a [pergel_core::LayoutGrid],
    dash: Option<[f32; 2]>,
    note: Option<&'a str>,
    ready: bool,
    section: bool,
    /// Start and end points, for vectors with an open path.
    end_points: Option<[pergel_core::EndPoint; 2]>,
    scroll: pergel_core::Scroll,
    sides: Option<[f32; 4]>,
    /// Fills above the own fill, bottom to top.
    extra_fills: Vec<serde_json::Value>,
    /// Component properties: definitions with this instance's values ("define" on a main or set,
    /// "values" on an instance, "link" on a layer inside a component).
    component_props: Option<Vec<serde_json::Value>>,
    props_mode: Option<&'static str>,
    /// The main or set holding the definitions (for "define" and "link").
    props_owner: Option<NodeId>,
    prop_refs: Option<pergel_core::PropRefs>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TextInfo<'a> {
    #[serde(flatten)]
    style: &'a Text,
    weight_name: String,
    /// Line height and laid-out text height in pixels, for the editing overlay.
    line_px: f32,
    text_height: f32,
}

impl<'a> From<&'a Node> for NodeInfo<'a> {
    fn from(n: &'a Node) -> Self {
        Self {
            id: n.id,
            name: &n.name,
            kind: n.kind.label(),
            parent: n.parent.map_or(-1, i64::from),
            x: n.bounds.x,
            y: n.bounds.y,
            w: n.bounds.w,
            h: n.bounds.h,
            rotation: n.rotation,
            radius: n.radius,
            corners: n.corners,
            fill: hex(n.fill),
            fill_opacity: alpha(n.fill),
            fill_on: n.fill_on,
            stroke: n.stroke.map(|(c, width)| StrokeInfo { color: hex(c), opacity: alpha(c), width, align: n.stroke_align }),
            opacity: (n.opacity * 100.0).round(),
            blend: n.blend,
            lock_ratio: n.lock_ratio,
            layout: n.layout.as_deref(),
            sizing: n.sizing,
            absolute: n.absolute,
            parent_flow: None,
            effects: n.effects.iter().map(EffectInfo::from).collect(),
            exports: &n.exports,
            component: role(n),
            main: None,
            main_name: None,
            in_instance: false,
            overrides: n.overrides,
            visible: n.visible,
            locked: n.locked,
            clip: n.clip,
            children: n.children.len(),
            text: None,
            paint: n.paint.as_deref(),
            mask: n.mask,
            bindings: n.bindings.as_deref(),
            constraints: n.constraints,
            styles: n.styles.as_deref(),
            variant_set: n.variant_set,
            variants: None,
            reactions: &n.reactions,
            code: n.code.as_deref(),
            grids: &n.grids,
            dash: n.dash,
            note: n.note.as_deref().map(|s| s.as_str()),
            ready: n.ready,
            section: n.section,
            end_points: n.vector.as_deref().filter(|v| v.is_open()).map(|v| [v.start, v.end]),
            scroll: n.scroll,
            sides: n.sides,
            extra_fills: n
                .extra_fills
                .iter()
                .map(|f| serde_json::json!({ "color": hex(f.color), "opacity": ((f.color.argb() >> 24) as f32 / 255.0 * 100.0).round(), "paint": f.paint, "visible": f.visible }))
                .collect(),
            component_props: None,
            props_mode: None,
            props_owner: None,
            prop_refs: n.prop_refs.as_deref().cloned(),
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_selection_json() -> *const c_char {
    emit(with_engine(|e| {
        let typesetter = &e.renderer.text;
        let nodes: Vec<NodeInfo> = e
            .selected()
            .into_iter()
            .map(|n| {
                let mut info = NodeInfo::from(n);
                info.main = e.scene.main_of(n.id);
                info.main_name = info.main.and_then(|m| e.scene.node(m)).map(|m| m.name.clone());
                info.in_instance = e.scene.in_instance(n.id);
                if let Some(main) = info.main
                    && let Some(set) = e.scene.variant_set_of(main)
                {
                    let current = e.scene.node(main).map(|m| pergel_core::parse_variant(&m.name)).unwrap_or_default();
                    info.variants = Some(
                        e.scene
                            .variant_props(set)
                            .into_iter()
                            .map(|(k, values)| {
                                let cur = current.iter().find(|(ck, _)| *ck == k).map(|(_, v)| v.clone());
                                serde_json::json!({ "name": k, "values": values, "current": cur })
                            })
                            .collect(),
                    );
                    info.main_name = e.scene.node(set).map(|s| s.name.clone());
                }
                // Component properties: what this layer can define, set or link.
                let defs_json = |defs: &[pergel_core::ComponentProp], inst: Option<NodeId>| -> Vec<serde_json::Value> {
                    defs.iter()
                        .map(|d| {
                            let mut v = serde_json::json!({ "id": d.id, "name": d.name, "kind": d.kind, "default": d.default });
                            if let Some(i) = inst {
                                v["value"] = serde_json::json!(e.scene.prop_value(i, d));
                            }
                            v
                        })
                        .collect()
                };
                let is_main = n.component == Some(Component::Main);
                if (is_main || n.variant_set) && !info.in_instance {
                    let owner = if n.variant_set { n.id } else { e.scene.props_owner(n.id) };
                    let defs = e.scene.node(owner).map(|o| o.props.clone()).unwrap_or_default();
                    info.component_props = Some(defs_json(&defs, None));
                    info.props_mode = Some("define");
                    info.props_owner = Some(owner);
                } else if matches!(n.component, Some(Component::Instance(_))) && !info.in_instance {
                    let defs = info.main.map(|m| e.scene.prop_defs(m)).unwrap_or_default();
                    if !defs.is_empty() {
                        info.component_props = Some(defs_json(&defs, Some(n.id)));
                        info.props_mode = Some("values");
                    }
                } else if !info.in_instance
                    && let Some(c) = e.scene.component_of(n.id)
                {
                    let owner = if e.scene.node(c).is_some_and(|x| x.variant_set) { c } else { e.scene.props_owner(c) };
                    let defs = e.scene.node(owner).map(|o| o.props.clone()).unwrap_or_default();
                    info.component_props = Some(defs_json(&defs, None));
                    info.props_mode = Some("link");
                    info.props_owner = Some(owner);
                }
                info.sizing = e.scene.effective_sizing(n);
                info.parent_flow = n
                    .parent
                    .and_then(|p| e.scene.node(p))
                    .and_then(|p| p.layout.as_ref())
                    .filter(|_| n.visible)
                    .map(|l| l.flow);
                info.text = n.text.as_deref().map(|style| {
                    let (line_px, text_height) = typesetter.metrics(style, n.bounds.w);
                    TextInfo { style, weight_name: style.weight_name(), line_px, text_height }
                });
                info
            })
            .collect();
        serde_json::to_string(&nodes).unwrap_or_default()
    }))
}

fn role(n: &Node) -> Option<&'static str> {
    match n.component {
        Some(Component::Main) => Some("main"),
        Some(Component::Instance(_)) => Some("instance"),
        None => None,
    }
}

#[derive(Serialize)]
struct LayerInfo<'a> {
    id: NodeId,
    name: &'a str,
    kind: &'static str,
    depth: u32,
    parent: i64,
    children: usize,
    visible: bool,
    locked: bool,
    component: Option<&'static str>,
    /// Inside a component or instance (named in purple).
    purple: bool,
}

/// Every layer depth-first in layers-panel order (topmost first).
#[unsafe(no_mangle)]
pub extern "C" fn pergel_layers_json() -> *const c_char {
    emit(with_engine(|e| {
        let scene = &e.scene;
        let mut layers = Vec::with_capacity(scene.len());
        let mut stack: Vec<(NodeId, u32, bool)> = scene.roots().iter().map(|id| (*id, 0, false)).collect();
        while let Some((id, depth, inside)) = stack.pop() {
            let Some(n) = scene.node(id) else { continue };
            let purple = inside || n.component.is_some();
            layers.push(LayerInfo {
                id,
                name: &n.name,
                // Image layers get their own icon in the layers panel, as in Figma.
                kind: if n.mask {
                    "Mask"
                } else if n.children.is_empty() && matches!(n.paint.as_deref(), Some(Paint::Image(_))) {
                    "Image"
                } else if n.section {
                    "Section"
                } else if n.slice {
                    "Slice"
                } else {
                    n.kind.label()
                },
                depth,
                parent: n.parent.map_or(-1, i64::from),
                children: n.children.len(),
                visible: n.visible,
                locked: n.locked,
                component: role(n),
                purple,
            });
            stack.extend(n.children.iter().map(|c| (*c, depth + 1, purple)));
        }
        serde_json::to_string(&layers).unwrap_or_default()
    }))
}

#[derive(Serialize)]
struct Label<'a> {
    id: NodeId,
    name: &'a str,
    component: bool,
    ready: bool,
    section: bool,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    width: f32,
    height: f32,
}

#[derive(Serialize)]
struct ViewState<'a> {
    zoom: f32,
    /// World point at the canvas's top-left corner.
    x: f32,
    y: f32,
    rotation: f32,
    frames: Vec<Label<'a>>,
    selection: Option<Label<'a>>,
    undo: bool,
    redo: bool,
}

#[unsafe(no_mangle)]
pub extern "C" fn pergel_view_json() -> *const c_char {
    emit(with_engine(|e| {
        let view = e.view;
        let label = |id: NodeId, name, b: Rect| {
            let (x, y) = view.to_screen(b.x, b.y);
            Label { id, name, component: false, ready: false, section: false, x, y, w: b.w * view.zoom, h: b.h * view.zoom, width: b.w, height: b.h }
        };
        let area = view.world_area();
        // Top-level frames, and the frames directly inside a top-level section, get a title.
        let frames = e
            .scene
            .roots()
            .iter()
            .filter_map(|id| e.scene.node(*id))
            .flat_map(|n| std::iter::once(n).chain(n.children.iter().filter(|_| n.section).filter_map(|c| e.scene.node(*c))))
            .filter(|n| n.kind == Kind::Frame && n.visible && n.bounds.intersects(&area))
            .map(|n| Label { component: n.component.is_some(), ready: n.ready, section: n.section, ..label(n.id, n.name.as_str(), n.bounds) })
            .collect();
        let mut selection = e.selection_bounds().map(|b| label(e.selection[0], "", b));
        let rotation = match e.selected().as_slice() {
            [one] if one.kind != Kind::Group => one.rotation,
            _ => 0.0,
        };
        // The size badge shows a rotated layer's own size, not its bounding box.
        if let (Some(l), [one]) = (selection.as_mut(), e.selected().as_slice())
            && one.kind != Kind::Group
        {
            (l.width, l.height) = (one.bounds.w, one.bounds.h);
        }
        let state = ViewState {
            zoom: view.zoom,
            x: view.x,
            y: view.y,
            rotation,
            frames,
            selection,
            undo: e.scene.can_undo(),
            redo: e.scene.can_redo(),
        };
        serde_json::to_string(&state).unwrap_or_default()
    }))
}

fn main() {}
