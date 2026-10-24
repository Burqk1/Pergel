//! Figma import: a file from Figma's REST API (`GET /v1/files/:key?geometry=paths`) becomes a
//! Pergel document — pages, frames, groups, shapes, vectors (from their path geometry), text,
//! auto layout, components and instances, fills (solid, gradient, image), strokes and effects.

use std::collections::HashMap;

use serde_json::Value;

use crate::clipboard::Tree;
use crate::document::{Document, Page};
use crate::geom::{Color, Rect};
use crate::import::blank;
use crate::layout::{Align, AutoLayout, Constraint, Flow, Justify, Sizing};
use crate::paint::{Cap, Gradient, GradientKind, ImageFill, ImageMode, Images, Join, Paint, Stop, VectorPath};
use crate::scene::{Kind, NodeId};
use crate::prototype::{Action, Reaction, Transition, Trigger};
use crate::style::{Blend, Effect, StrokeAlign};
use crate::text::{Decoration, LineHeight, Text, TextAlign, TextCase, TextResize, VerticalAlign};

/// What the import did, for the user.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct FigmaReport {
    pub pages: usize,
    pub layers: usize,
    pub images: usize,
    pub missing_images: usize,
    pub components: usize,
    /// Fonts the file uses (Pergel falls back to Inter for those it doesn't have).
    pub fonts: Vec<String>,
}

struct Ctx<'a> {
    images: &'a Images,
    /// Figma node id → the id saved with its layer (prototype links point at these).
    ids: HashMap<String, NodeId>,
    /// Figma component id → numeric id used to link instances to their main.
    components: HashMap<String, NodeId>,
    report: FigmaReport,
}

/// Converts a Figma file (REST JSON). `images` must hold the file's image fills keyed by their
/// `imageRef`; they are embedded in the document.
pub fn figma_to_document(file: &Value, images: &Images) -> Result<(Document, FigmaReport), String> {
    let doc = file.get("document").ok_or("Not a Figma file: no \"document\"")?;
    let mut ctx = Ctx { images, ids: HashMap::new(), components: HashMap::new(), report: FigmaReport::default() };
    let mut counter: NodeId = 1_000_000;
    number_nodes(doc, &mut ctx.ids, &mut counter);
    // Number every main component first so instances anywhere can point at it.
    let mut next: NodeId = 1;
    collect_components(doc, &mut ctx.components, &mut next);
    ctx.report.components = ctx.components.len();
    let mut pages = Vec::new();
    for canvas in arr(doc, "children") {
        if canvas["type"] != "CANVAS" {
            continue;
        }
        let name = str_of(canvas, "name").unwrap_or("Page").to_string();
        let mut layers = Vec::new();
        for child in arr(canvas, "children") {
            if let Some(t) = convert(child, None, &mut ctx) {
                layers.push(t);
            }
        }
        pages.push(Page { name, layers, view: Value::Null, guides: Vec::new() });
    }
    if pages.is_empty() {
        return Err("The Figma file has no pages".into());
    }
    ctx.report.pages = pages.len();
    ctx.report.fonts.sort();
    ctx.report.fonts.dedup();
    let mut document = Document::new(pages);
    document.embed_images(images);
    ctx.report.images = document.images.len();
    Ok((document, ctx.report))
}

/// Component properties: definitions on components and sets, references on their layers,
/// values on instances. Property ids are Figma's keys ("Label#12:0").
fn component_properties(n: &Value, t: &mut Tree, ctx: &Ctx) {
    use crate::component_props::{ComponentProp, PropKind, PropRefs, PropValue};
    let swap = |v: &Value| PropValue::Swap(v.as_str().and_then(|id| ctx.ids.get(id)).copied().unwrap_or(0));
    if let Some(defs) = n.get("componentPropertyDefinitions").and_then(Value::as_object) {
        for (key, d) in defs {
            let (kind, default) = match str_of(d, "type") {
                Some("BOOLEAN") => (PropKind::Boolean, PropValue::Bool(d["defaultValue"].as_bool().unwrap_or(true))),
                Some("TEXT") => (PropKind::Text, PropValue::Text(d["defaultValue"].as_str().unwrap_or("").to_string())),
                Some("INSTANCE_SWAP") => (PropKind::InstanceSwap, swap(&d["defaultValue"])),
                _ => continue,
            };
            t.props.push(ComponentProp { id: key.clone(), name: key.split('#').next().unwrap_or(key).to_string(), kind, default });
        }
    }
    if let Some(r) = n.get("componentPropertyReferences") {
        let refs = PropRefs {
            visible: str_of(r, "visible").map(str::to_string),
            text: str_of(r, "characters").map(str::to_string),
            swap: str_of(r, "mainComponent").map(str::to_string),
        };
        if !refs.is_empty() {
            t.prop_refs = Some(refs);
        }
    }
    if str_of(n, "type") == Some("INSTANCE")
        && let Some(values) = n.get("componentProperties").and_then(Value::as_object)
    {
        for (key, v) in values {
            let value = match str_of(v, "type") {
                Some("BOOLEAN") => PropValue::Bool(v["value"].as_bool().unwrap_or(true)),
                Some("TEXT") => PropValue::Text(v["value"].as_str().unwrap_or("").to_string()),
                Some("INSTANCE_SWAP") => swap(&v["value"]),
                _ => continue,
            };
            t.prop_values.insert(key.clone(), value);
        }
    }
}

fn number_nodes(node: &Value, map: &mut HashMap<String, NodeId>, next: &mut NodeId) {
    if let Some(id) = str_of(node, "id") {
        map.insert(id.to_string(), *next);
        *next += 1;
    }
    for c in arr(node, "children") {
        number_nodes(c, map, next);
    }
}

/// Figma's REST `interactions` as Pergel reactions.
fn reactions(n: &Value, ctx: &Ctx) -> Vec<Reaction> {
    let mut out = Vec::new();
    for i in arr(n, "interactions") {
        let trigger = match str_of(&i["trigger"], "type") {
            Some("ON_HOVER") | Some("MOUSE_ENTER") => Trigger::OnHover,
            Some("AFTER_TIMEOUT") => Trigger::AfterDelay { ms: (f(&i["trigger"], "timeout").unwrap_or(0.8) * 1000.0) as u32 },
            Some("ON_CLICK") | Some("ON_PRESS") | Some("MOUSE_UP") | Some("MOUSE_DOWN") => Trigger::OnClick,
            _ => continue,
        };
        for a in arr(i, "actions") {
            let action = match (str_of(a, "type"), str_of(a, "navigation")) {
                (Some("BACK"), _) => Action::Back,
                (Some("URL"), _) => Action::OpenUrl { url: str_of(a, "url").unwrap_or("").to_string() },
                (Some("NODE"), Some("OVERLAY") | Some("SWAP_STATE")) => match str_of(a, "destinationId").and_then(|d| ctx.ids.get(d)) {
                    Some(to) => Action::OpenOverlay {
                        to: *to,
                        position: crate::prototype::OverlayPosition::Center,
                        close_outside: a.get("overlayRelativePosition").is_none(),
                        dim: false,
                    },
                    None => continue,
                },
                (Some("NODE"), Some("SCROLL_TO")) => match str_of(a, "destinationId").and_then(|d| ctx.ids.get(d)) {
                    Some(to) => Action::ScrollTo { to: *to },
                    None => continue,
                },
                (Some("CLOSE"), _) => Action::CloseOverlay,
                (Some("NODE"), Some("CHANGE_TO")) => match str_of(a, "destinationId").and_then(|d| ctx.ids.get(d)) {
                    Some(to) => Action::ChangeTo { to: *to },
                    None => continue,
                },
                (Some("NODE"), Some("NAVIGATE") | Some("SWAP") | None) => match str_of(a, "destinationId").and_then(|d| ctx.ids.get(d)) {
                    Some(to) => Action::Navigate { to: *to },
                    None => continue,
                },
                _ => continue,
            };
            let tr = &a["transition"];
            let transition = match (str_of(tr, "type"), str_of(tr, "direction")) {
                (Some("DISSOLVE"), _) => Transition::Dissolve,
                (Some("SMART_ANIMATE"), _) => Transition::SmartAnimate,
                (Some("MOVE_IN") | Some("SLIDE_IN"), Some("LEFT")) => Transition::SlideLeft,
                (Some("MOVE_IN") | Some("SLIDE_IN"), Some("RIGHT")) => Transition::SlideRight,
                (Some("MOVE_IN") | Some("SLIDE_IN"), Some("TOP")) => Transition::SlideUp,
                (Some("MOVE_IN") | Some("SLIDE_IN"), Some("BOTTOM")) => Transition::SlideDown,
                (Some("PUSH"), _) => Transition::Push,
                _ => Transition::Instant,
            };
            let duration = (f(tr, "duration").unwrap_or(0.3) * 1000.0) as u32;
            out.push(Reaction { trigger, action, transition, duration });
        }
    }
    out
}

fn collect_components(node: &Value, map: &mut HashMap<String, NodeId>, next: &mut NodeId) {
    if node["type"] == "COMPONENT"
        && let Some(id) = str_of(node, "id")
    {
        map.insert(id.to_string(), *next);
        *next += 1;
    }
    for c in arr(node, "children") {
        collect_components(c, map, next);
    }
}

fn arr<'a>(v: &'a Value, key: &str) -> impl Iterator<Item = &'a Value> {
    v.get(key).and_then(Value::as_array).map(|a| a.iter()).into_iter().flatten()
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

fn f(v: &Value, key: &str) -> Option<f32> {
    v.get(key).and_then(Value::as_f64).map(|x| x as f32)
}

/// Figma colours are 0–1 floats; `opacity` multiplies the alpha.
fn color(c: &Value, opacity: f32) -> Color {
    let ch = |k: &str| (f(c, k).unwrap_or(0.0).clamp(0.0, 1.0) * 255.0).round() as u8;
    let a = (f(c, "a").unwrap_or(1.0) * opacity).clamp(0.0, 1.0);
    Color::rgba(ch("r"), ch("g"), ch("b"), (a * 255.0).round() as u8)
}

fn blend(mode: Option<&str>, container: bool) -> Blend {
    match mode.unwrap_or("") {
        "PASS_THROUGH" => Blend::PassThrough,
        "DARKEN" => Blend::Darken,
        "MULTIPLY" => Blend::Multiply,
        "COLOR_BURN" | "LINEAR_BURN" => Blend::ColorBurn,
        "LIGHTEN" => Blend::Lighten,
        "SCREEN" => Blend::Screen,
        "COLOR_DODGE" | "LINEAR_DODGE" => Blend::ColorDodge,
        "OVERLAY" => Blend::Overlay,
        "SOFT_LIGHT" => Blend::SoftLight,
        "HARD_LIGHT" => Blend::HardLight,
        "DIFFERENCE" => Blend::Difference,
        "EXCLUSION" => Blend::Exclusion,
        "HUE" => Blend::Hue,
        "SATURATION" => Blend::Saturation,
        "COLOR" => Blend::Color,
        "LUMINOSITY" => Blend::Luminosity,
        "NORMAL" => Blend::Normal,
        _ if container => Blend::PassThrough,
        _ => Blend::Normal,
    }
}

/// The node's unrotated box in absolute coordinates and its rotation (degrees, CCW).
fn geometry(n: &Value) -> Option<(Rect, f32)> {
    let bb = n.get("absoluteBoundingBox").filter(|v| !v.is_null())?;
    let aabb = Rect::new(f(bb, "x")?, f(bb, "y")?, f(bb, "width").unwrap_or(0.0), f(bb, "height").unwrap_or(0.0));
    let rotation = n
        .get("relativeTransform")
        .and_then(|m| Some((m.get(0)?.get(0)?.as_f64()? as f32, m.get(0)?.get(1)?.as_f64()? as f32)))
        .map_or(0.0, |(cos, sin)| crate::normalize_degrees(sin.atan2(cos).to_degrees()));
    let rotation = if rotation.abs() < 0.01 { 0.0 } else { rotation };
    let size = n.get("size").and_then(|s| Some((f(s, "x")?, f(s, "y")?)));
    match size {
        // The absolute box is the rotated layer's extent; its centre is the layer's centre.
        Some((w, h)) if rotation != 0.0 => {
            let (cx, cy) = aabb.center();
            Some((Rect::new(cx - w / 2.0, cy - h / 2.0, w, h), rotation))
        }
        Some((w, h)) => Some((Rect::new(aabb.x, aabb.y, w, h), 0.0)),
        None => Some((aabb, 0.0)),
    }
}

fn convert(n: &Value, parent_layout: Option<&AutoLayout>, ctx: &mut Ctx) -> Option<Tree> {
    let ty = str_of(n, "type")?;
    let kind = match ty {
        "FRAME" | "COMPONENT" | "COMPONENT_SET" | "INSTANCE" | "SECTION" => Kind::Frame,
        "GROUP" => Kind::Group,
        "RECTANGLE" | "SLICE" => Kind::Rectangle,
        "ELLIPSE" => Kind::Ellipse,
        "TEXT" => Kind::Text,
        "VECTOR" | "STAR" | "LINE" | "REGULAR_POLYGON" | "BOOLEAN_OPERATION" | "ARROW" => Kind::Vector,
        // Slices, stickies, connectors, tables, widgets…: nothing to draw here.
        _ => return None,
    };
    let (bounds, rotation) = geometry(n)?;
    let mut t = blank(kind, str_of(n, "name").unwrap_or(ty), bounds);
    t.rotation = rotation;
    t.visible = n.get("visible").and_then(Value::as_bool).unwrap_or(true);
    t.locked = n.get("locked").and_then(Value::as_bool).unwrap_or(false);
    t.opacity = f(n, "opacity").unwrap_or(1.0);
    t.blend = blend(str_of(n, "blendMode"), kind.is_container());
    t.clip = kind == Kind::Frame && n.get("clipsContent").and_then(Value::as_bool).unwrap_or(ty != "SECTION");
    t.stroke_align = match str_of(n, "strokeAlign") {
        Some("OUTSIDE") => StrokeAlign::Outside,
        Some("CENTER") => StrokeAlign::Center,
        _ if kind == Kind::Vector => StrokeAlign::Center,
        _ => StrokeAlign::Inside,
    };
    ctx.report.layers += 1;

    // Corners.
    if let Some(r) = f(n, "cornerRadius") {
        t.radius = r;
    }
    if let Some(c) = n.get("rectangleCornerRadii").and_then(Value::as_array)
        && c.len() == 4
    {
        let v: Vec<f32> = c.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect();
        if v.iter().any(|x| *x != v[0]) {
            t.corners = Some([v[0], v[1], v[2], v[3]]);
        } else {
            t.radius = v[0];
        }
    }

    // Fills: Figma stacks them bottom to top; the bottom one is the layer's own fill and the
    // rest are drawn above it.
    t.fill_on = false;
    t.fill = Color(0x00ff_ffff);
    let fills: Vec<&Value> = arr(n, "fills").filter(|p| p.get("visible").and_then(Value::as_bool).unwrap_or(true)).collect();
    if kind != Kind::Text && fills.len() > 1 {
        for p in &fills[1..] {
            let mut tmp = blank(kind, "", bounds);
            apply_paint(&mut tmp, p, bounds, ctx);
            if tmp.fill_on {
                t.extra_fills.push(crate::paint::FillLayer { color: tmp.fill, paint: tmp.paint, visible: true });
            }
        }
    }
    let primary = if kind == Kind::Text { fills.last() } else { fills.first() };
    if let Some(p) = primary {
        apply_paint(&mut t, p, bounds, ctx);
    }
    if kind == Kind::Group {
        t.fill_on = false;
    }

    // Strokes.
    let weight = f(n, "strokeWeight").unwrap_or(1.0);
    if let Some(s) = arr(n, "strokes").filter(|p| p.get("visible").and_then(Value::as_bool).unwrap_or(true)).last()
        && weight > 0.0
    {
        let c = match s.get("color") {
            Some(c) => color(c, f(s, "opacity").unwrap_or(1.0)),
            None => s.get("gradientStops").and_then(|g| g.get(0)).and_then(|g| g.get("color")).map_or(Color(0xff00_0000), |c| color(c, 1.0)),
        };
        t.stroke = Some((c, weight));
        if let Some(w) = n.get("individualStrokeWeights") {
            let side = |k: &str| f(w, k).unwrap_or(weight);
            let sides = [side("top"), side("right"), side("bottom"), side("left")];
            if sides.iter().any(|s| (*s - weight).abs() > 1e-3) {
                t.sides = Some(sides);
                t.stroke = Some((c, sides.iter().copied().fold(0.0, f32::max)));
            }
        }
    }

    if let Some(d) = n.get("strokeDashes").and_then(Value::as_array)
        && d.len() >= 2
    {
        t.dash = Some([d[0].as_f64().unwrap_or(4.0) as f32, d[1].as_f64().unwrap_or(4.0) as f32]);
    }

    // Effects.
    for e in arr(n, "effects") {
        let visible = e.get("visible").and_then(Value::as_bool).unwrap_or(true);
        let radius = f(e, "radius").unwrap_or(0.0);
        let (x, y) = e.get("offset").map_or((0.0, 0.0), |o| (f(o, "x").unwrap_or(0.0), f(o, "y").unwrap_or(0.0)));
        let spread = f(e, "spread").unwrap_or(0.0);
        let c = e.get("color").map_or(Color(0x4000_0000), |c| color(c, 1.0));
        t.effects.push(match str_of(e, "type") {
            Some("DROP_SHADOW") => Effect::DropShadow { color: c, x, y, blur: radius, spread, visible },
            Some("INNER_SHADOW") => Effect::InnerShadow { color: c, x, y, blur: radius, spread, visible },
            Some("LAYER_BLUR") => Effect::LayerBlur { radius, visible },
            Some("BACKGROUND_BLUR") => Effect::BackgroundBlur { radius, visible },
            _ => continue,
        });
    }

    // Auto layout of this frame, and how it sits in its parent's.
    let layout = auto_layout(n);
    if let Some(l) = &layout {
        let primary_auto = str_of(n, "primaryAxisSizingMode") == Some("AUTO");
        let counter_auto = str_of(n, "counterAxisSizingMode") == Some("AUTO");
        let (h, v) = if l.flow == Flow::Vertical { (counter_auto, primary_auto) } else { (primary_auto, counter_auto) };
        t.sizing = [if h { Sizing::Hug } else { Sizing::Fixed }, if v { Sizing::Hug } else { Sizing::Fixed }];
    }
    let sizing = |k: &str| match str_of(n, k) {
        Some("HUG") => Some(Sizing::Hug),
        Some("FILL") => Some(Sizing::Fill),
        Some("FIXED") => Some(Sizing::Fixed),
        _ => None,
    };
    if let Some(s) = sizing("layoutSizingHorizontal") {
        t.sizing[0] = s;
    }
    if let Some(s) = sizing("layoutSizingVertical") {
        t.sizing[1] = s;
    }
    if parent_layout.is_none() {
        // Fill only makes sense inside auto layout.
        t.sizing = t.sizing.map(|s| if s == Sizing::Fill { Sizing::Fixed } else { s });
    }
    t.absolute = str_of(n, "layoutPositioning") == Some("ABSOLUTE");
    t.mask = n.get("isMask").and_then(Value::as_bool).unwrap_or(false);
    t.section = ty == "SECTION";
    if ty == "SLICE" {
        t.slice = true;
        t.fill_on = false;
    }
    t.scroll = match str_of(n, "overflowDirection") {
        Some("VERTICAL_SCROLLING") => crate::prototype::Scroll::Vertical,
        Some("HORIZONTAL_SCROLLING") => crate::prototype::Scroll::Horizontal,
        Some("HORIZONTAL_AND_VERTICAL_SCROLLING") => crate::prototype::Scroll::Both,
        _ => crate::prototype::Scroll::None,
    };
    t.ready = str_of(n, "devStatus").is_some() || n.get("devStatus").and_then(|d| str_of(d, "type")) == Some("READY_FOR_DEV");
    for g in arr(n, "layoutGrids") {
        use crate::style::{GridAlign, GridKind, LayoutGrid};
        let kind = match str_of(g, "pattern") {
            Some("ROWS") => GridKind::Rows,
            Some("GRID") => GridKind::Grid,
            _ => GridKind::Columns,
        };
        t.grids.push(LayoutGrid {
            kind,
            count: g.get("count").and_then(Value::as_u64).unwrap_or(5).min(500) as u16,
            gutter: f(g, "gutterSize").unwrap_or(20.0),
            margin: f(g, "offset").unwrap_or(0.0),
            size: f(g, "sectionSize").unwrap_or(10.0),
            align: match str_of(g, "alignment") {
                Some("MIN") => GridAlign::Start,
                Some("MAX") => GridAlign::End,
                Some("CENTER") => GridAlign::Center,
                _ => GridAlign::Stretch,
            },
            color: g.get("color").map_or(Color::rgba(255, 0, 0, 26), |c| color(c, 1.0)),
            visible: g.get("visible").and_then(Value::as_bool).unwrap_or(true),
        });
    }
    t.saved_id = str_of(n, "id").and_then(|id| ctx.ids.get(id)).copied();
    t.reactions = reactions(n, ctx);
    t.variant_set = ty == "COMPONENT_SET";
    component_properties(n, &mut t, ctx);
    if let Some(c) = n.get("constraints") {
        let axis = |k: &str| match str_of(c, k) {
            Some("RIGHT") | Some("BOTTOM") => Constraint::Max,
            Some("LEFT_RIGHT") | Some("TOP_BOTTOM") => Constraint::Stretch,
            Some("CENTER") => Constraint::Center,
            Some("SCALE") => Constraint::Scale,
            _ => Constraint::Min,
        };
        t.constraints = [axis("horizontal"), axis("vertical")];
    }

    // Components.
    match ty {
        "COMPONENT" => {
            if let Some(id) = str_of(n, "id").and_then(|id| ctx.components.get(id)) {
                t.main = Some(*id);
                t.is_main = true;
            }
        }
        "INSTANCE" => {
            t.main = str_of(n, "componentId").and_then(|id| ctx.components.get(id)).copied();
        }
        _ => {}
    }

    match kind {
        Kind::Text => text(&mut t, n, ctx),
        Kind::Vector => vector(&mut t, n),
        _ => {}
    }

    for c in arr(n, "children") {
        if let Some(child) = convert(c, layout.as_ref(), ctx) {
            t.children.push(child);
        }
    }
    // Pergel's frames hug their auto-layout content by flow; groups fit their members.
    if kind == Kind::Group && t.children.is_empty() {
        return None;
    }
    t.layout = layout;
    Some(t)
}

fn auto_layout(n: &Value) -> Option<AutoLayout> {
    let flow = match (str_of(n, "layoutMode"), str_of(n, "layoutWrap")) {
        (Some("HORIZONTAL"), Some("WRAP")) => Flow::Wrap,
        (Some("HORIZONTAL"), _) => Flow::Horizontal,
        (Some("VERTICAL"), _) => Flow::Vertical,
        (Some("GRID"), _) => Flow::Grid,
        _ => return None,
    };
    let gap = f(n, "itemSpacing").unwrap_or(0.0);
    let mut l = AutoLayout {
        flow,
        gap,
        counter_gap: f(n, "counterAxisSpacing").unwrap_or(gap),
        padding: [
            f(n, "paddingTop").unwrap_or(0.0),
            f(n, "paddingRight").unwrap_or(0.0),
            f(n, "paddingBottom").unwrap_or(0.0),
            f(n, "paddingLeft").unwrap_or(0.0),
        ],
        justify: match str_of(n, "primaryAxisAlignItems") {
            Some("CENTER") => Justify::Center,
            Some("MAX") => Justify::End,
            Some("SPACE_BETWEEN") => Justify::SpaceBetween,
            _ => Justify::Start,
        },
        align: match str_of(n, "counterAxisAlignItems") {
            Some("CENTER") => Align::Center,
            Some("MAX") => Align::End,
            Some("BASELINE") => Align::Baseline,
            _ => Align::Start,
        },
        ..AutoLayout::default()
    };
    if flow == Flow::Grid {
        l.columns = n.get("gridColumnCount").and_then(Value::as_u64).unwrap_or(2) as u16;
        l.rows = n.get("gridRowCount").and_then(Value::as_u64).unwrap_or(2) as u16;
        l.gap = f(n, "gridColumnGap").unwrap_or(l.gap);
        l.counter_gap = f(n, "gridRowGap").unwrap_or(l.counter_gap);
    }
    Some(l)
}

fn apply_paint(t: &mut Tree, p: &Value, bounds: Rect, ctx: &mut Ctx) {
    let opacity = f(p, "opacity").unwrap_or(1.0);
    t.fill_on = true;
    match str_of(p, "type").unwrap_or("") {
        "SOLID" => {
            t.fill = p.get("color").map_or(Color(0xff00_0000), |c| color(c, opacity));
        }
        g @ ("GRADIENT_LINEAR" | "GRADIENT_RADIAL" | "GRADIENT_ANGULAR" | "GRADIENT_DIAMOND") => {
            let stops: Vec<Stop> = arr(p, "gradientStops")
                .map(|s| Stop { at: f(s, "position").unwrap_or(0.0), color: s.get("color").map_or(Color(0xff00_0000), |c| color(c, opacity)) })
                .collect();
            let handles: Vec<(f32, f32)> = arr(p, "gradientHandlePositions").filter_map(|h| Some((f(h, "x")?, f(h, "y")?))).collect();
            if stops.is_empty() {
                return;
            }
            let kind = match g {
                "GRADIENT_RADIAL" => GradientKind::Radial,
                "GRADIENT_ANGULAR" => GradientKind::Angular,
                "GRADIENT_DIAMOND" => GradientKind::Diamond,
                _ => GradientKind::Linear,
            };
            let mut grad = Gradient::new(kind, stops[0].color, stops[stops.len() - 1].color);
            grad.stops = stops;
            if let [a, b, c, ..] = handles.as_slice() {
                grad.from = *a;
                grad.to = *b;
                // Second axis length relative to the first, measured in the layer's pixels.
                let len = |p: (f32, f32), q: (f32, f32)| (((q.0 - p.0) * bounds.w).powi(2) + ((q.1 - p.1) * bounds.h).powi(2)).sqrt();
                let main = len(*a, *b);
                if main > 0.0 {
                    grad.width = len(*a, *c) / main;
                }
            }
            t.fill = grad.stops[0].color;
            t.paint = Some(Paint::Gradient(grad));
        }
        "IMAGE" => {
            let Some(r) = str_of(p, "imageRef") else { return };
            let mode = match str_of(p, "scaleMode") {
                Some("FIT") => ImageMode::Fit,
                Some("CROP") | Some("STRETCH") => ImageMode::Crop,
                Some("TILE") => ImageMode::Tile,
                _ => ImageMode::Fill,
            };
            t.fill = Color(0xffd9_d9d9);
            match ctx.images.get(r).and_then(|b| imagesize::blob_size(b).ok()) {
                Some(size) => {
                    t.paint = Some(Paint::Image(ImageFill {
                        hash: r.to_string(),
                        mode,
                        width: size.width as u32,
                        height: size.height as u32,
                        scale: f(p, "scalingFactor").unwrap_or(1.0),
                        opacity,
                        adjust: p.get("filters").map(|fl| crate::paint::ImageAdjust {
                            exposure: f(fl, "exposure").unwrap_or(0.0),
                            contrast: f(fl, "contrast").unwrap_or(0.0),
                            saturation: f(fl, "saturation").unwrap_or(0.0),
                            temperature: f(fl, "temperature").unwrap_or(0.0),
                            tint: f(fl, "tint").unwrap_or(0.0),
                            highlights: f(fl, "highlights").unwrap_or(0.0),
                            shadows: f(fl, "shadows").unwrap_or(0.0),
                        }).unwrap_or_default(),
                        flip: [false, false],
                    }))
                }
                None => ctx.report.missing_images += 1,
            }
        }
        // Video, pattern, noise…: keep a placeholder colour.
        _ => t.fill = Color(0xffd9_d9d9),
    }
}

fn text(t: &mut Tree, n: &Value, ctx: &mut Ctx) {
    let s = n.get("style").cloned().unwrap_or(Value::Null);
    let size = f(&s, "fontSize").unwrap_or(12.0);
    let family = str_of(&s, "fontFamily").unwrap_or("Inter").to_string();
    ctx.report.fonts.push(family.clone());
    let mut text = Text::new(str_of(n, "characters").unwrap_or(""), size, f(&s, "fontWeight").unwrap_or(400.0) as u16);
    text.family = family;
    text.italic = s.get("italic").and_then(Value::as_bool).unwrap_or(false);
    text.auto_name = false;
    text.letter_spacing = if size > 0.0 { f(&s, "letterSpacing").unwrap_or(0.0) / size * 100.0 } else { 0.0 };
    text.line_height = match str_of(&s, "lineHeightUnit") {
        Some("PIXELS") => f(&s, "lineHeightPx").map_or(LineHeight::Auto, LineHeight::Px),
        Some("FONT_SIZE_%") => f(&s, "lineHeightPercentFontSize").map_or(LineHeight::Auto, LineHeight::Percent),
        _ => LineHeight::Auto,
    };
    text.align = match str_of(&s, "textAlignHorizontal") {
        Some("CENTER") => TextAlign::Center,
        Some("RIGHT") => TextAlign::Right,
        Some("JUSTIFIED") => TextAlign::Justify,
        _ => TextAlign::Left,
    };
    text.vertical = match str_of(&s, "textAlignVertical") {
        Some("CENTER") => VerticalAlign::Middle,
        Some("BOTTOM") => VerticalAlign::Bottom,
        _ => VerticalAlign::Top,
    };
    text.resize = match str_of(&s, "textAutoResize") {
        Some("WIDTH_AND_HEIGHT") => TextResize::AutoWidth,
        Some("HEIGHT") => TextResize::AutoHeight,
        _ => TextResize::Fixed,
    };
    text.decoration = match str_of(&s, "textDecoration") {
        Some("UNDERLINE") => Decoration::Underline,
        Some("STRIKETHROUGH") => Decoration::Strikethrough,
        _ => Decoration::None,
    };
    text.case = match str_of(&s, "textCase") {
        Some("UPPER") => TextCase::Upper,
        Some("LOWER") => TextCase::Lower,
        Some("TITLE") => TextCase::Title,
        _ => TextCase::Original,
    };
    let line_types: Vec<&str> = arr(n, "lineTypes").filter_map(Value::as_str).collect();
    text.list = if line_types.contains(&"ORDERED") {
        crate::text::ListStyle::Numbered
    } else if line_types.contains(&"UNORDERED") {
        crate::text::ListStyle::Bullet
    } else {
        crate::text::ListStyle::None
    };
    if let Some(flags) = s.get("opentypeFlags").and_then(Value::as_object) {
        for (k, v) in flags {
            text.features.insert(k.to_ascii_lowercase(), v.as_u64().unwrap_or(1) as u32);
        }
    }
    // Mixed styles: Figma lists a style id per character plus a table of overrides.
    if let (Some(ids), Some(table)) = (n.get("characterStyleOverrides").and_then(Value::as_array), n.get("styleOverrideTable").and_then(Value::as_object)) {
        let mut start = 0usize;
        let ids: Vec<u64> = ids.iter().map(|v| v.as_u64().unwrap_or(0)).collect();
        while start < ids.len() {
            let id = ids[start];
            let mut end = start;
            while end < ids.len() && ids[end] == id {
                end += 1;
            }
            if id != 0
                && let Some(o) = table.get(&id.to_string())
            {
                let mut run = crate::text::TextRun { start, end, ..Default::default() };
                run.family = str_of(o, "fontFamily").map(str::to_string);
                run.weight = f(o, "fontWeight").map(|w| w as u16);
                run.italic = o.get("italic").and_then(Value::as_bool);
                run.size = f(o, "fontSize");
                run.decoration = match str_of(o, "textDecoration") {
                    Some("UNDERLINE") => Some(Decoration::Underline),
                    Some("STRIKETHROUGH") => Some(Decoration::Strikethrough),
                    Some("NONE") => Some(Decoration::None),
                    _ => None,
                };
                if let Some(c) = arr(o, "fills").find(|p| p["type"] == "SOLID").and_then(|p| p.get("color").map(|c| color(c, f(p, "opacity").unwrap_or(1.0)))) {
                    run.color = Some(c.argb());
                }
                run.link = o.get("hyperlink").and_then(|h| str_of(h, "url")).map(str::to_string);
                if let Some(fam) = &run.family {
                    ctx.report.fonts.push(fam.clone());
                }
                if !run.is_plain() {
                    text.runs.push(run);
                }
            }
            start = end;
        }
    }
    t.text = Some(text);
}

/// Vector-like layers keep Figma's exact outline: the fill geometry, or for open paths (lines,
/// strokes only) the stroke outline filled with the stroke colour.
fn vector(t: &mut Tree, n: &Value) {
    let paths = |key: &str| -> (String, bool) {
        let mut d = String::new();
        let mut even_odd = false;
        for g in arr(n, key) {
            if let Some(p) = str_of(g, "path") {
                d.push_str(p);
                even_odd |= str_of(g, "windingRule") == Some("EVENODD");
            }
        }
        (d, even_odd)
    };
    let (fill_d, fill_eo) = paths("fillGeometry");
    let (stroke_d, stroke_eo) = paths("strokeGeometry");
    let size = (t.bounds.w, t.bounds.h);
    let cap = match str_of(n, "strokeCap") {
        Some("ROUND") => Cap::Round,
        Some("SQUARE") => Cap::Square,
        _ => Cap::Butt,
    };
    let join = match str_of(n, "strokeJoin") {
        Some("ROUND") => Join::Round,
        Some("BEVEL") => Join::Bevel,
        _ => Join::Miter,
    };
    let filled = t.fill_on && !fill_d.is_empty();
    if filled {
        t.vector = Some(VectorPath { d: absolute(&fill_d), size, even_odd: fill_eo, cap, join, start: Default::default(), end: Default::default() });
        if !stroke_d.is_empty() && t.stroke.is_some() {
            // Stroke outlines can't be stroked again; keep the stroke on the fill's path.
            t.stroke_align = StrokeAlign::Center;
        }
    } else if let (false, Some((c, _))) = (stroke_d.is_empty(), t.stroke) {
        // An open path: draw its stroke outline as a filled shape, exactly as Figma does.
        t.vector = Some(VectorPath { d: absolute(&stroke_d), size, even_odd: stroke_eo, cap, join, start: Default::default(), end: Default::default() });
        t.fill = c;
        t.fill_on = true;
        t.stroke = None;
    } else {
        t.vector = Some(VectorPath { d: absolute(&fill_d), size, even_odd: fill_eo, cap, join, start: Default::default(), end: Default::default() });
    }
}

/// Figma's geometry is absolute M/L/Q/C/Z already; normalise separators.
fn absolute(d: &str) -> String {
    crate::paint::map_path(d, |x, y| (x, y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Scene;

    #[test]
    fn a_figma_file_becomes_pages_of_layers() {
        let file: Value = serde_json::from_str(
            r##"{"document":{"type":"DOCUMENT","children":[
              {"type":"CANVAS","name":"Cover","children":[
                {"id":"1:2","type":"COMPONENT","name":"Button","absoluteBoundingBox":{"x":0,"y":0,"width":120,"height":40},
                 "size":{"x":120,"y":40},"relativeTransform":[[1,0,0],[0,1,0]],
                 "fills":[{"type":"SOLID","color":{"r":0.486,"g":0.227,"b":0.929,"a":1}}],"cornerRadius":8,
                 "layoutMode":"HORIZONTAL","itemSpacing":8,"paddingLeft":16,"paddingRight":16,"paddingTop":10,"paddingBottom":10,
                 "primaryAxisAlignItems":"CENTER","counterAxisAlignItems":"CENTER","primaryAxisSizingMode":"AUTO","counterAxisSizingMode":"AUTO",
                 "children":[{"type":"TEXT","name":"Label","characters":"Buy","absoluteBoundingBox":{"x":16,"y":10,"width":30,"height":20},
                   "fills":[{"type":"SOLID","color":{"r":1,"g":1,"b":1,"a":1}}],
                   "style":{"fontFamily":"Inter","fontWeight":600,"fontSize":14,"letterSpacing":0.28,"lineHeightUnit":"PIXELS","lineHeightPx":20,"textAutoResize":"WIDTH_AND_HEIGHT"}}]},
                {"id":"1:9","type":"INSTANCE","componentId":"1:2","name":"Button","absoluteBoundingBox":{"x":0,"y":100,"width":120,"height":40},
                 "fills":[{"type":"SOLID","color":{"r":0.486,"g":0.227,"b":0.929,"a":1}}],
                 "children":[{"type":"TEXT","name":"Label","characters":"Sell","absoluteBoundingBox":{"x":16,"y":110,"width":30,"height":20},
                   "fills":[{"type":"SOLID","color":{"r":1,"g":1,"b":1,"a":1}}],"style":{"fontFamily":"Inter","fontWeight":600,"fontSize":14}}]},
                {"type":"VECTOR","name":"Check","absoluteBoundingBox":{"x":200,"y":0,"width":24,"height":24},"size":{"x":24,"y":24},
                 "fills":[],"strokes":[{"type":"SOLID","color":{"r":0,"g":0,"b":0,"a":1}}],"strokeWeight":2,
                 "fillGeometry":[],"strokeGeometry":[{"path":"M0 0L24 0L24 2L0 2Z","windingRule":"NONZERO"}]},
                {"type":"RECTANGLE","name":"Photo","absoluteBoundingBox":{"x":300,"y":0,"width":200,"height":100},
                 "fills":[{"type":"IMAGE","imageRef":"abc","scaleMode":"FILL"}],
                 "effects":[{"type":"DROP_SHADOW","visible":true,"color":{"r":0,"g":0,"b":0,"a":0.25},"offset":{"x":0,"y":4},"radius":8,"spread":0}]},
                {"type":"RECTANGLE","name":"Gradient","absoluteBoundingBox":{"x":0,"y":300,"width":100,"height":100},
                 "fills":[{"type":"GRADIENT_LINEAR","gradientHandlePositions":[{"x":0.5,"y":0},{"x":0.5,"y":1},{"x":0,"y":0}],
                   "gradientStops":[{"position":0,"color":{"r":1,"g":0,"b":0,"a":1}},{"position":1,"color":{"r":0,"g":0,"b":1,"a":1}}]}]}
              ]},
              {"type":"CANVAS","name":"Screens","children":[]}
            ]}}"##,
        )
        .unwrap();
        let mut images = Images::default();
        let png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 4, 0, 0, 0, 2, 8, 6, 0, 0, 0];
        images.insert("abc".into(), std::sync::Arc::new(png));
        let (doc, report) = figma_to_document(&file, &images).unwrap();
        assert_eq!(doc.pages.len(), 2);
        assert_eq!((report.components, report.images, report.missing_images), (1, 1, 0));
        let layers = &doc.pages[0].layers;
        let button = &layers[0];
        assert!(button.is_main);
        assert_eq!(button.layout.as_ref().unwrap().padding, [10.0, 16.0, 10.0, 16.0]);
        assert_eq!(button.sizing, [Sizing::Hug, Sizing::Hug]);
        assert_eq!(button.fill, Color::rgba(124, 58, 237, 255));
        let label = button.children[0].text.as_ref().unwrap();
        assert_eq!((label.weight, label.line_height), (600, LineHeight::Px(20.0)));
        assert!((label.letter_spacing - 2.0).abs() < 0.01);
        assert_eq!(layers[1].main, button.main, "instance links to its main");
        let check = &layers[2];
        assert_eq!(check.kind, Kind::Vector);
        assert!(check.fill_on && check.stroke.is_none(), "stroke outline drawn as a fill");
        let Some(Paint::Image(img)) = &layers[3].paint else { panic!("image fill") };
        assert_eq!((img.width, img.height), (4, 2));
        assert_eq!(layers[3].effects.len(), 1);
        let Some(Paint::Gradient(g)) = &layers[4].paint else { panic!("gradient") };
        assert_eq!((g.from, g.to), ((0.5, 0.0), (0.5, 1.0)));

        // It loads into a scene with the component link intact.
        let scene = Scene::from_trees(&doc.pages[0].layers);
        assert_eq!(scene.mains().len(), 1);
        assert_eq!(scene.instances_of(scene.mains()[0]).len(), 1);
        assert_eq!(doc.image_store().len(), 1);
    }
}
