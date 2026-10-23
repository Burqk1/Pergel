//! Design to code: React + Tailwind or plain HTML + CSS for a layer subtree. Auto layout becomes
//! flexbox / grid, free-form children become absolutely positioned inside a relative box.

use std::collections::HashMap;
use std::fmt::Write;

use crate::assets::{Asset, AssetKind};
use crate::components::Component;
use crate::paint::{Gradient, GradientKind, ImageMode, Paint};
use crate::style::Effect;
use crate::variables::{BindProp, Variables};
use crate::geom::Color;
use crate::layout::{Align, Constraint, Flow, Justify, Sizing};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::style::StrokeAlign;
use crate::text::{Decoration, LineHeight, TextAlign, TextCase, TextResize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framework {
    ReactTailwind,
    Html,
}

impl Framework {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "html" | "html-css" | "css" => Framework::Html,
            _ => Framework::ReactTailwind,
        }
    }
}

fn hex(c: Color) -> String {
    let a = c.argb() >> 24;
    if a == 255 { format!("#{:06x}", c.argb() & 0xffffff) } else { format!("#{:06x}{:02x}", c.argb() & 0xffffff, a) }
}

fn px(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{}px", r as i64) } else { format!("{r}px") }
}

fn escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('{', "&#123;").replace('}', "&#125;")
}

/// One CSS declaration, written once for Tailwind (as an arbitrary-value class) and once as CSS.
struct Decl {
    tw: String,
    css: String,
}

fn d(tw: impl Into<String>, css: impl Into<String>) -> Decl {
    Decl { tw: tw.into(), css: css.into() }
}

pub struct Generated {
    pub code: String,
    pub fonts: Vec<String>,
    pub components: Vec<String>,
    /// Files the code references (icons, illustrations, images), with `{asset_base}{file}` urls.
    pub assets: Vec<Asset>,
    /// Variables the code uses: (variable name, CSS custom property, current value).
    pub variables: Vec<(String, String, String)>,
}

struct Gen<'a> {
    scene: &'a Scene,
    framework: Framework,
    out: String,
    fonts: Vec<String>,
    components: Vec<String>,
    /// Layer → index into `assets`.
    asset_of: HashMap<NodeId, usize>,
    assets: Vec<Asset>,
    asset_base: String,
    vars: &'a Variables,
    used_vars: Vec<(String, String, String)>,
    /// Code Connect imports the code needs.
    imports: Vec<String>,
}

impl Scene {
    /// Code for the subtree at `id`. Icons and images are referenced as files under
    /// `asset_base` (a URL or path prefix such as `/assets/`), never redrawn in code.
    pub fn generate_code(&self, id: NodeId, framework: Framework, asset_base: &str) -> Option<Generated> {
        self.generate_code_with(id, framework, asset_base, &Variables::default())
    }

    /// Like `generate_code`; properties bound to `vars` become CSS custom properties.
    pub fn generate_code_with(&self, id: NodeId, framework: Framework, asset_base: &str, vars: &Variables) -> Option<Generated> {
        self.node(id)?;
        let assets = self.find_assets(id);
        let asset_of = assets.iter().enumerate().flat_map(|(i, a)| a.nodes.iter().map(move |n| (*n, i))).collect();
        let mut g = Gen {
            scene: self,
            framework,
            out: String::new(),
            fonts: Vec::new(),
            components: Vec::new(),
            asset_of,
            assets,
            asset_base: asset_base.to_string(),
            vars,
            used_vars: Vec::new(),
            imports: Vec::new(),
        };
        g.node(id, None, 0, true);
        let mut code = String::new();
        if framework == Framework::ReactTailwind {
            for i in &g.imports {
                let _ = writeln!(code, "{i}");
            }
            if !g.imports.is_empty() {
                code.push('\n');
            }
            for a in &g.assets {
                let _ = writeln!(code, "const {} = \"{}{}\";", asset_var(a), g.asset_base, a.file);
            }
            if !g.assets.is_empty() {
                code.push('\n');
            }
            let name = self.node(id).map(|n| component_name(&n.name)).unwrap_or_else(|| "Design".into());
            let _ = writeln!(code, "export default function {name}() {{\n  return (");
            for line in g.out.lines() {
                let _ = writeln!(code, "    {line}");
            }
            code.push_str("  );\n}\n");
        } else {
            code = g.out;
        }
        Some(Generated { code, fonts: g.fonts, components: g.components, assets: g.assets, variables: g.used_vars })
    }
}

/// The React constant holding an asset's URL: `search.svg` → `imgSearch`.
fn asset_var(a: &Asset) -> String {
    let stem = a.file.rsplit_once('.').map_or(a.file.as_str(), |(s, _)| s);
    format!("img{}", component_name(stem).trim_start_matches("Design"))
}

/// CSS for a gradient fill on a `w`×`h` box.
fn gradient_css(g: &Gradient, w: f32, h: f32) -> String {
    let stops = g.sorted_stops();
    let (fx, fy) = (g.from.0 * w, g.from.1 * h);
    let (tx, ty) = (g.to.0 * w, g.to.1 * h);
    let list = |map: &dyn Fn(f32) -> f32| -> String {
        stops.iter().map(|s| format!("{} {}%", hex(s.color), (map(s.at) * 1000.0).round() / 10.0)).collect::<Vec<_>>().join(", ")
    };
    match g.kind {
        GradientKind::Linear => {
            // CSS measures stops along a line through the centre whose length covers the box;
            // project Figma's handles onto it.
            let angle = g.css_angle(w, h);
            let rad = angle.to_radians();
            let (dx, dy) = (rad.sin(), -rad.cos());
            let len = (w * rad.sin()).abs() + (h * rad.cos()).abs();
            let at = |x: f32, y: f32| if len > 0.0 { ((x - w / 2.0) * dx + (y - h / 2.0) * dy) / len + 0.5 } else { 0.0 };
            let (a, b) = (at(fx, fy), at(tx, ty));
            format!("linear-gradient({}deg, {})", angle, list(&|s| a + s * (b - a)))
        }
        GradientKind::Angular => {
            let from = (ty - fy).atan2(tx - fx).to_degrees() + 90.0;
            format!("conic-gradient(from {}deg at {} {}, {})", (from.rem_euclid(360.0) * 10.0).round() / 10.0, px(fx), px(fy), list(&|s| s))
        }
        GradientKind::Radial | GradientKind::Diamond => {
            let main = ((tx - fx).powi(2) + (ty - fy).powi(2)).sqrt();
            let second = main * g.width;
            let vertical = (ty - fy).abs() > (tx - fx).abs();
            let (rx, ry) = if vertical { (second, main) } else { (main, second) };
            format!("radial-gradient(ellipse {} {} at {} {}, {})", px(rx), px(ry), px(fx), px(fy), list(&|s| s))
        }
    }
}

/// Tailwind arbitrary values cannot contain spaces.
fn tw_value(css: &str) -> String {
    css.replace(", ", ",").replace(' ', "_")
}

fn component_name(name: &str) -> String {
    let mut out = String::new();
    let mut up = true;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if up {
                out.extend(c.to_uppercase());
            } else {
                out.push(c);
            }
            up = false;
        } else {
            up = true;
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) { format!("Design{out}") } else { out }
}

impl Gen<'_> {
    /// `fallback`, or `var(--name, fallback)` when the property is bound to a variable.
    fn bound(&mut self, n: &Node, prop: BindProp, fallback: String) -> String {
        let Some(var) = n.bindings.as_ref().and_then(|b| b.get(&prop)).and_then(|id| self.vars.get(id)) else {
            return fallback;
        };
        let css = Variables::css_name(&var.name);
        let mut value = self.vars.resolve(&var.id).map(|v| Variables::display(&v)).unwrap_or_default();
        // Lengths need a unit in CSS.
        if matches!(prop, BindProp::Radius | BindProp::Gap | BindProp::Padding | BindProp::Width | BindProp::Height | BindProp::FontSize | BindProp::StrokeWidth)
            && value.parse::<f32>().is_ok()
        {
            value.push_str("px");
        }
        if !self.used_vars.iter().any(|(name, _, _)| *name == var.name) {
            self.used_vars.push((var.name.clone(), css.clone(), value));
        }
        format!("var({css}, {fallback})")
    }

    fn node(&mut self, id: NodeId, parent: Option<&Node>, depth: usize, root: bool) {
        let Some(n) = self.scene.node(id) else { return };
        if !n.visible || n.slice {
            return;
        }
        let mut decls = Vec::new();
        self.position(n, parent, root, &mut decls);
        self.size(n, parent, root, &mut decls);
        if let Some(&i) = self.asset_of.get(&id)
            && !self.assets[i].background
        {
            self.asset(n, i, depth, decls);
            return;
        }
        self.box_style(n, &mut decls);
        if let Some(layout) = n.layout.as_deref().filter(|_| n.kind == Kind::Frame) {
            let gap = self.bound(n, BindProp::Gap, px(layout.gap));
            let pad = n.bindings.as_ref().is_some_and(|b| b.contains_key(&BindProp::Padding)).then(|| self.bound(n, BindProp::Padding, px(layout.padding[0])));
            self.container(layout, &mut decls, &gap, pad.as_deref());
        } else if n.kind != Kind::Text && !n.children.is_empty() {
            decls.push(d("relative", "position: relative"));
        }
        let pad = "  ".repeat(depth);
        if let Some(note) = n.note.as_deref() {
            let _ = writeln!(self.out, "{pad}{}", self.comment(&format!("Note: {}", note.replace("*/", "* /").replace("-->", "- ->"))));
        }
        if self.framework == Framework::ReactTailwind
            && matches!(n.component, Some(Component::Instance(_)))
            && let Some(jsx) = self.scene.code_connect_jsx(id)
        {
            // Code Connect: the codebase's own component, positioned by a wrapper when needed.
            if let Some(main) = self.scene.main_of(id)
                && let Some(link) = self.scene.code_link_of(main)
                && !self.imports.contains(&link.import)
                && !link.import.is_empty()
            {
                self.imports.push(link.import.clone());
            }
            let place: Vec<Decl> = decls.into_iter().filter(|d| d.tw.starts_with("absolute") || d.tw.starts_with("left-") || d.tw.starts_with("top-") || d.tw.starts_with("right-") || d.tw.starts_with("bottom-") || d.tw.contains("self-stretch") || d.tw.starts_with("flex-1")).collect();
            if place.is_empty() {
                let _ = writeln!(self.out, "{pad}{jsx}");
            } else {
                let attrs = self.attrs(n, place);
                let _ = writeln!(self.out, "{pad}<div{attrs}>{jsx}</div>");
            }
            return;
        }
        if let Some(Component::Instance(main)) = n.component {
            let name = match self.scene.variant_set_of(main) {
                Some(set) => format!(
                    "{} ({})",
                    self.scene.node(set).map_or(String::new(), |s| s.name.clone()),
                    self.scene.node(main).map_or(String::new(), |m| m.name.clone())
                ),
                None => self.scene.node(main).map_or(n.name.clone(), |m| m.name.clone()),
            };
            if !self.components.contains(&name) {
                self.components.push(name.clone());
            }
            let _ = writeln!(self.out, "{pad}{}", self.comment(&format!("Instance of \"{name}\"")));
        }
        if n.kind == Kind::Text {
            self.text(n, depth, decls);
            return;
        }
        let attrs = self.attrs(n, decls);
        let kids: Vec<NodeId> = n.children.clone();
        if kids.iter().all(|c| self.scene.node(*c).is_none_or(|c| !c.visible)) {
            let _ = match self.framework {
                Framework::ReactTailwind => writeln!(self.out, "{pad}<div{attrs} />"),
                Framework::Html => writeln!(self.out, "{pad}<div{attrs}></div>"),
            };
            return;
        }
        let _ = writeln!(self.out, "{pad}<div{attrs}>");
        let me = n.clone();
        for c in kids {
            self.node(c, Some(&me), depth + 1, false);
        }
        let _ = writeln!(self.out, "{pad}</div>");
    }

    fn comment(&self, text: &str) -> String {
        match self.framework {
            Framework::ReactTailwind => format!("{{/* {text} */}}"),
            Framework::Html => format!("<!-- {text} -->"),
        }
    }

    fn attrs(&self, n: &Node, decls: Vec<Decl>) -> String {
        let name = escape(&n.name).replace('"', "&quot;");
        let id = n.id;
        match self.framework {
            Framework::ReactTailwind => {
                let classes: Vec<String> = decls.into_iter().map(|d| d.tw).filter(|t| !t.is_empty()).collect();
                format!(" data-node-id=\"{id}\" data-name=\"{name}\" className=\"{}\"", classes.join(" "))
            }
            Framework::Html => {
                let css: Vec<String> = decls.into_iter().map(|d| d.css).filter(|c| !c.is_empty()).collect();
                format!(" data-node-id=\"{id}\" data-name=\"{name}\" style=\"{}\"", css.join("; "))
            }
        }
    }

    /// An icon, illustration or image layer: an `<img>` of its exported file.
    fn asset(&mut self, n: &Node, i: usize, depth: usize, mut decls: Vec<Decl>) {
        let a = &self.assets[i];
        let kind = a.kind;
        let src = match self.framework {
            Framework::ReactTailwind => format!("{{{}}}", asset_var(a)),
            Framework::Html => format!("\"{}{}\"", self.asset_base, a.file),
        };
        if kind == AssetKind::Image {
            if let Some(Paint::Image(img)) = n.paint.as_deref() {
                let (tw, css) = match img.mode {
                    ImageMode::Fill => ("object-cover", "object-fit: cover"),
                    ImageMode::Fit => ("object-contain", "object-fit: contain"),
                    ImageMode::Crop | ImageMode::Tile => ("object-fill", "object-fit: fill"),
                };
                decls.push(d(tw, css));
                if let Some(f) = img.adjust.css_filter() {
                    decls.push(d(format!("[filter:{}]", f.replace(' ', "_")), format!("filter: {f}")));
                }
            }
            if n.kind == Kind::Ellipse {
                decls.push(d("rounded-full", "border-radius: 50%"));
            } else if n.radius > 0.0 {
                decls.push(d(format!("rounded-[{}]", px(n.radius)), format!("border-radius: {}", px(n.radius))));
            }
        } else {
            decls.push(d("block max-w-none", "display: block; max-width: none"));
            // Lines and arrows: the file includes the stroke and arrowheads around the box.
            let over = self.scene.svg_overflow(n.id);
            if over > 0.0 {
                let (w, h) = (n.bounds.w + 2.0 * over, n.bounds.h + 2.0 * over);
                decls.retain(|x| !x.tw.starts_with("w-[") && !x.tw.starts_with("h-["));
                decls.push(d(format!("w-[{}]", px(w)), format!("width: {}", px(w))));
                decls.push(d(format!("h-[{}]", px(h)), format!("height: {}", px(h))));
                decls.push(d(format!("-m-[{}]", px(over)), format!("margin: -{}", px(over))));
            }
        }
        if n.opacity < 1.0 {
            let o = (n.opacity * 100.0).round() / 100.0;
            decls.push(d(format!("opacity-[{o}]"), format!("opacity: {o}")));
        }
        if n.rotation != 0.0 {
            let r = (-n.rotation * 100.0).round() / 100.0;
            decls.push(d(format!("rotate-[{r}deg]"), format!("transform: rotate({r}deg)")));
        }
        let alt = if kind == AssetKind::Image { escape(&n.name).replace('"', "&quot;") } else { String::new() };
        let attrs = self.attrs(n, decls);
        let pad = "  ".repeat(depth);
        let close = if self.framework == Framework::ReactTailwind { " /" } else { "" };
        let _ = writeln!(self.out, "{pad}<img{attrs} src={src} alt=\"{alt}\"{close}>");
    }

    fn in_flow(&self, n: &Node, parent: Option<&Node>) -> bool {
        parent.is_some_and(|p| p.layout.is_some()) && !n.absolute
    }

    fn position(&self, n: &Node, parent: Option<&Node>, root: bool, decls: &mut Vec<Decl>) {
        if root || self.in_flow(n, parent) {
            return;
        }
        let Some(p) = parent else { return };
        let origin = if p.kind == Kind::Group { p.aabb() } else { p.bounds };
        let (x, y) = (n.bounds.x - origin.x, n.bounds.y - origin.y);
        decls.push(d("absolute", "position: absolute"));
        // Constraints make the position follow the parent's size, as in Figma.
        let [ch, cv] = if p.kind == Kind::Frame { n.constraints } else { [Constraint::Min; 2] };
        let far = |start: f32, len: f32, parent: f32| parent - start - len;
        let pct = |v: f32, of: f32| if of > 0.0 { format!("{}%", (v / of * 10000.0).round() / 100.0) } else { "0%".into() };
        let axis = |c: Constraint, start: f32, len: f32, parent: f32, (lo, hi, size): (&str, &str, &str)| -> Vec<Decl> {
            let tw_lo = if lo == "left" { "left" } else { "top" };
            let tw_hi = if hi == "right" { "right" } else { "bottom" };
            match c {
                Constraint::Min => vec![d(format!("{tw_lo}-[{}]", px(start)), format!("{lo}: {}", px(start)))],
                Constraint::Max => vec![d(format!("{tw_hi}-[{}]", px(far(start, len, parent))), format!("{hi}: {}", px(far(start, len, parent))))],
                Constraint::Stretch => vec![
                    d(format!("{tw_lo}-[{}]", px(start)), format!("{lo}: {}", px(start))),
                    d(format!("{tw_hi}-[{}]", px(far(start, len, parent))), format!("{hi}: {}", px(far(start, len, parent)))),
                ],
                Constraint::Center => {
                    let off = start + len / 2.0 - parent / 2.0;
                    vec![d(format!("{tw_lo}-[calc(50%+{})]", px(off - len / 2.0)), format!("{lo}: calc(50% + {})", px(off - len / 2.0)))]
                }
                Constraint::Scale => vec![
                    d(format!("{tw_lo}-[{}]", pct(start, parent)), format!("{lo}: {}", pct(start, parent))),
                    d(format!("{}-[{}]", if size == "width" { "w" } else { "h" }, pct(len, parent)), format!("{size}: {}", pct(len, parent))),
                ],
            }
        };
        decls.extend(axis(ch, x, n.bounds.w, origin.w, ("left", "right", "width")));
        decls.extend(axis(cv, y, n.bounds.h, origin.h, ("top", "bottom", "height")));
    }

    fn size(&self, n: &Node, parent: Option<&Node>, root: bool, decls: &mut Vec<Decl>) {
        let sizing = self.scene.effective_sizing(n);
        let flow = parent.and_then(|p| p.layout.as_ref()).filter(|_| self.in_flow(n, parent)).map(|l| l.flow);
        let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
        let free = !root && !self.in_flow(n, parent) && parent.is_some_and(|p| p.kind == Kind::Frame);
        for (axis, s) in sizing.into_iter().enumerate() {
            let (tw, css, v) = if axis == 0 { ("w", "width", b.w) } else { ("h", "height", b.h) };
            // Stretched and scaled layers get their size from the constraints.
            if free && matches!(n.constraints[axis], Constraint::Stretch | Constraint::Scale) {
                continue;
            }
            match s {
                Sizing::Fixed => decls.push(d(format!("{tw}-[{}]", px(v)), format!("{css}: {}", px(v)))),
                Sizing::Hug => {}
                Sizing::Fill => {
                    let main = flow.is_some_and(|f| f.is_horizontal() == (axis == 0) && f != Flow::Grid);
                    if main {
                        decls.push(d("flex-1", "flex: 1 1 0"));
                        decls.push(d(format!("min-{tw}-0"), format!("min-{css}: 0")));
                    } else {
                        decls.push(d("self-stretch", "align-self: stretch"));
                    }
                }
            }
        }
        let _ = root;
    }

    fn box_style(&mut self, n: &Node, decls: &mut Vec<Decl>) {
        let paints = n.kind != Kind::Text && n.kind != Kind::Group && n.fill_on;
        match n.paint.as_deref() {
            _ if !paints => {}
            Some(Paint::Gradient(g)) => {
                let css = gradient_css(g, n.bounds.w, n.bounds.h);
                decls.push(d(format!("bg-[{}]", tw_value(&css)), format!("background: {css}")));
            }
            Some(Paint::Image(img)) => {
                // An image behind other layers: a CSS background.
                if let Some(&i) = self.asset_of.get(&n.id) {
                    let url = format!("{}{}", self.asset_base, self.assets[i].file);
                    let (tw, css) = match img.mode {
                        ImageMode::Fill => ("bg-cover bg-center", "background-size: cover; background-position: center"),
                        ImageMode::Fit => ("bg-contain bg-center bg-no-repeat", "background-size: contain; background-position: center; background-repeat: no-repeat"),
                        ImageMode::Crop => ("bg-[length:100%_100%]", "background-size: 100% 100%"),
                        ImageMode::Tile => ("bg-repeat", "background-repeat: repeat"),
                    };
                    decls.push(d(format!("bg-[url('{url}')] {tw}"), format!("background-image: url('{url}'); {css}")));
                }
            }
            None if n.fill.argb() >> 24 != 0 => {
                let c = self.bound(n, BindProp::Fill, hex(n.fill));
                decls.push(d(format!("bg-[{}]", tw_value(&c)), format!("background: {c}")));
            }
            None => {}
        }
        if paints && n.extra_fills.iter().any(|f| f.visible) {
            // Several fills: CSS backgrounds stacked top first; colours as flat gradients.
            let layer = |color: Color, paint: Option<&Paint>| -> Option<String> {
                match paint {
                    Some(Paint::Gradient(g)) => Some(gradient_css(g, n.bounds.w, n.bounds.h)),
                    Some(Paint::Image(_)) => None,
                    None => Some(format!("linear-gradient({0}, {0})", hex(color))),
                }
            };
            let primary = decls.iter().position(|x| x.css.starts_with("background:"));
            let base = primary.map(|i| decls.remove(i).css.trim_start_matches("background:").trim().to_string());
            let mut layers: Vec<String> = n.extra_fills.iter().rev().filter(|f| f.visible).filter_map(|f| layer(f.color, f.paint.as_ref())).collect();
            if let Some(b) = base {
                layers.push(if b.contains("gradient(") || b.starts_with("url(") || b.starts_with("var(") { b } else { format!("linear-gradient({b}, {b})") });
            }
            let css = layers.join(", ");
            decls.push(d(format!("[background:{}]", tw_value(&css)), format!("background: {css}")));
        }
        if n.kind == Kind::Ellipse {
            decls.push(d("rounded-full", "border-radius: 50%"));
        } else if let Some(c) = n.corners {
            let v = c.map(px);
            decls.push(d(
                format!("rounded-tl-[{}] rounded-tr-[{}] rounded-br-[{}] rounded-bl-[{}]", v[0], v[1], v[2], v[3]),
                format!("border-radius: {} {} {} {}", v[0], v[1], v[2], v[3]),
            ));
        } else if n.radius > 0.0 && n.kind != Kind::Text {
            let r = self.bound(n, BindProp::Radius, px(n.radius));
            decls.push(d(format!("rounded-[{}]", tw_value(&r)), format!("border-radius: {r}")));
        }
        // Shadows and an inside stroke share one box-shadow list.
        let mut shadows: Vec<String> = Vec::new();
        for e in n.effects.iter().filter(|e| e.visible()) {
            match *e {
                Effect::DropShadow { color, x, y, blur, spread, .. } if n.kind != Kind::Text => {
                    shadows.push(format!("{} {} {} {} {}", px(x), px(y), px(blur), px(spread), hex(color)))
                }
                Effect::DropShadow { color, x, y, blur, .. } => {
                    let css = format!("{} {} {} {}", px(x), px(y), px(blur), hex(color));
                    decls.push(d(format!("[text-shadow:{}]", tw_value(&css)), format!("text-shadow: {css}")));
                }
                Effect::InnerShadow { color, x, y, blur, spread, .. } => {
                    shadows.push(format!("inset {} {} {} {} {}", px(x), px(y), px(blur), px(spread), hex(color)))
                }
                Effect::LayerBlur { radius, .. } => {
                    let r = px(radius / 2.0);
                    decls.push(d(format!("blur-[{r}]"), format!("filter: blur({r})")));
                }
                Effect::BackgroundBlur { radius, .. } => {
                    let r = px(radius / 2.0);
                    decls.push(d(format!("backdrop-blur-[{r}]"), format!("backdrop-filter: blur({r})")));
                }
            }
        }
        if let Some(([t, r, b, l], (color, _))) = n.sides.zip(n.stroke).filter(|_| n.kind != Kind::Text) {
            // Individual strokes: one inset shadow per side, or real borders outside the box.
            let c = self.bound(n, BindProp::Stroke, hex(color));
            if n.stroke_align == StrokeAlign::Inside {
                for (w, dx, dy) in [(t, 0.0, 1.0), (r, -1.0, 0.0), (b, 0.0, -1.0), (l, 1.0, 0.0)] {
                    if w > 0.0 {
                        shadows.push(format!("inset {} {} 0 0 {c}", px(dx * w), px(dy * w)));
                    }
                }
            } else {
                let widths = format!("{} {} {} {}", px(t), px(r), px(b), px(l));
                decls.push(d(
                    format!("border-solid border-[{}] border-t-[{}] border-r-[{}] border-b-[{}] border-l-[{}]", tw_value(&c), px(t), px(r), px(b), px(l)),
                    format!("border-style: solid; border-color: {c}; border-width: {widths}"),
                ));
            }
        } else if let Some((color, width)) = n.stroke.filter(|s| s.1 > 0.0 && n.kind != Kind::Text) {
            let (c, w) = (self.bound(n, BindProp::Stroke, hex(color)), px(width));
            match n.stroke_align {
                StrokeAlign::Inside => shadows.push(format!("inset 0 0 0 {w} {c}")),
                _ if n.dash.is_some() => {
                    // Dashed strokes need a real border (box-shadow can't dash).
                    decls.push(d(format!("border-[{w}] border-dashed border-[{}]", tw_value(&c)), format!("border: {w} dashed {c}")));
                }
                StrokeAlign::Center => decls.push(d(
                    format!("outline outline-[{w}] outline-[{c}] -outline-offset-[{}]", px(width / 2.0)),
                    format!("outline: {w} solid {c}; outline-offset: -{}", px(width / 2.0)),
                )),
                StrokeAlign::Outside => {
                    decls.push(d(format!("outline outline-[{w}] outline-[{c}]"), format!("outline: {w} solid {c}")))
                }
            }
        }
        if !shadows.is_empty() {
            let css = shadows.join(", ");
            decls.push(d(format!("shadow-[{}]", tw_value(&css)), format!("box-shadow: {css}")));
        }
        if n.opacity < 1.0 {
            let o = (n.opacity * 100.0).round();
            decls.push(d(format!("opacity-[{}]", o / 100.0), format!("opacity: {}", o / 100.0)));
        }
        if n.rotation != 0.0 {
            // Figma's angles turn counter-clockwise; CSS rotates clockwise.
            let r = (-n.rotation * 100.0).round() / 100.0;
            decls.push(d(format!("rotate-[{r}deg]"), format!("transform: rotate({r}deg)")));
        }
        if n.kind == Kind::Frame && n.clip && !n.children.is_empty() {
            use crate::prototype::Scroll;
            match n.scroll {
                Scroll::None => decls.push(d("overflow-hidden", "overflow: hidden")),
                Scroll::Vertical => decls.push(d("overflow-x-hidden overflow-y-auto", "overflow-x: hidden; overflow-y: auto")),
                Scroll::Horizontal => decls.push(d("overflow-x-auto overflow-y-hidden", "overflow-x: auto; overflow-y: hidden")),
                Scroll::Both => decls.push(d("overflow-auto", "overflow: auto")),
            }
        }
    }

    fn container(&self, l: &crate::layout::AutoLayout, decls: &mut Vec<Decl>, gap: &str, pad_var: Option<&str>) {
        let gap = tw_value(gap);
        if l.flow == Flow::Grid {
            decls.push(d("grid", "display: grid"));
            decls.push(d(
                format!("grid-cols-[repeat({},minmax(0,1fr))]", l.columns.max(1)),
                format!("grid-template-columns: repeat({}, minmax(0, 1fr))", l.columns.max(1)),
            ));
            decls.push(d(format!("gap-x-[{gap}] gap-y-[{}]", px(l.counter_gap)), format!("column-gap: {gap}; row-gap: {}", px(l.counter_gap))));
        } else {
            let row = l.flow.is_horizontal();
            decls.push(d(if row { "flex" } else { "flex flex-col" }, if row { "display: flex" } else { "display: flex; flex-direction: column" }));
            if l.flow == Flow::Wrap {
                decls.push(d(format!("flex-wrap gap-y-[{}]", px(l.counter_gap)), format!("flex-wrap: wrap; row-gap: {}", px(l.counter_gap))));
            }
            if l.justify != Justify::SpaceBetween && l.gap > 0.0 {
                let (tw, css) = if row { ("gap-x", "column-gap") } else { ("gap-y", "row-gap") };
                decls.push(d(format!("{tw}-[{gap}]"), format!("{css}: {gap}")));
            }
            let (tw, css) = match l.justify {
                Justify::Start => ("", ""),
                Justify::Center => ("justify-center", "justify-content: center"),
                Justify::End => ("justify-end", "justify-content: flex-end"),
                Justify::SpaceBetween => ("justify-between", "justify-content: space-between"),
            };
            decls.push(d(tw, css));
        }
        let (tw, css) = match l.align {
            Align::Start => ("items-start", "align-items: flex-start"),
            Align::Center => ("items-center", "align-items: center"),
            Align::End => ("items-end", "align-items: flex-end"),
            Align::Baseline => ("items-baseline", "align-items: baseline"),
        };
        decls.push(d(tw, css));
        if let Some(p) = pad_var {
            decls.push(d(format!("p-[{}]", tw_value(p)), format!("padding: {p}")));
            return;
        }
        let [t, r, b, lft] = l.padding.map(px);
        if l.padding.iter().any(|p| *p > 0.0) {
            decls.push(d(format!("pt-[{t}] pr-[{r}] pb-[{b}] pl-[{lft}]"), format!("padding: {t} {r} {b} {lft}")));
        }
    }

    fn text(&mut self, n: &Node, depth: usize, mut decls: Vec<Decl>) {
        let Some(t) = n.text.as_deref() else { return };
        if !self.fonts.contains(&t.family) {
            self.fonts.push(t.family.clone());
        }
        let family = if t.family.contains(' ') { format!("'{}'", t.family) } else { t.family.clone() };
        decls.push(d("m-0", "margin: 0"));
        decls.push(d(format!("font-['{}']", t.family.replace(' ', "_")), format!("font-family: {family}, sans-serif")));
        let size = self.bound(n, BindProp::FontSize, px(t.size));
        decls.push(d(format!("text-[{}]", tw_value(&size)), format!("font-size: {size}")));
        decls.push(d(format!("font-[{}]", t.weight), format!("font-weight: {}", t.weight)));
        if t.italic {
            decls.push(d("italic", "font-style: italic"));
        }
        match t.line_height {
            LineHeight::Auto => {}
            LineHeight::Px(v) => decls.push(d(format!("leading-[{}]", px(v)), format!("line-height: {}", px(v)))),
            LineHeight::Percent(p) => {
                decls.push(d(format!("leading-[{}]", (p / 100.0 * 1000.0).round() / 1000.0), format!("line-height: {}%", p)))
            }
        }
        if t.letter_spacing != 0.0 {
            let em = (t.letter_spacing / 100.0 * 1000.0).round() / 1000.0;
            decls.push(d(format!("tracking-[{em}em]"), format!("letter-spacing: {em}em")));
        }
        if n.fill_on {
            let c = self.bound(n, BindProp::Fill, hex(n.fill));
            decls.push(d(format!("text-[{}]", tw_value(&c)), format!("color: {c}")));
        }
        match t.align {
            TextAlign::Left => {}
            TextAlign::Center => decls.push(d("text-center", "text-align: center")),
            TextAlign::Right => decls.push(d("text-right", "text-align: right")),
            TextAlign::Justify => decls.push(d("text-justify", "text-align: justify")),
        }
        match t.decoration {
            Decoration::None => {}
            Decoration::Underline => decls.push(d("underline", "text-decoration: underline")),
            Decoration::Strikethrough => decls.push(d("line-through", "text-decoration: line-through")),
        }
        match t.case {
            TextCase::Original => {}
            TextCase::Upper => decls.push(d("uppercase", "text-transform: uppercase")),
            TextCase::Lower => decls.push(d("lowercase", "text-transform: lowercase")),
            TextCase::Title => decls.push(d("capitalize", "text-transform: capitalize")),
        }
        if let Some(f) = t.feature_css() {
            decls.push(d(format!("[font-feature-settings:{}]", f.replace(' ', "_")), format!("font-feature-settings: {f}")));
        }
        if t.list != crate::text::ListStyle::None {
            // A real list: one item per line.
            let (tag, tw, css) = if t.list == crate::text::ListStyle::Numbered {
                ("ol", "list-decimal", "list-style: decimal")
            } else {
                ("ul", "list-disc", "list-style: disc")
            };
            decls.push(d(format!("{tw} pl-[1.4em]"), format!("{css}; padding-left: 1.4em")));
            let pad = "  ".repeat(depth);
            let attrs = self.attrs(n, decls);
            let _ = writeln!(self.out, "{pad}<{tag}{attrs}>");
            for line in t.content.split('\n') {
                let _ = writeln!(self.out, "{pad}  <li>{}</li>", escape(line));
            }
            let _ = writeln!(self.out, "{pad}</{tag}>");
            return;
        }
        if t.resize == TextResize::AutoWidth {
            decls.push(d("whitespace-pre", "white-space: pre"));
        } else {
            decls.push(d("whitespace-pre-wrap", "white-space: pre-wrap"));
        }
        if t.vertical != crate::text::VerticalAlign::Top && t.resize == TextResize::Fixed {
            let (tw, css) = if t.vertical == crate::text::VerticalAlign::Middle {
                ("flex flex-col justify-center", "display: flex; flex-direction: column; justify-content: center")
            } else {
                ("flex flex-col justify-end", "display: flex; flex-direction: column; justify-content: flex-end")
            };
            decls.push(d(tw, css));
        }
        let pad = "  ".repeat(depth);
        let br = match self.framework {
            Framework::ReactTailwind => "<br />",
            Framework::Html => "<br>",
        };
        let content = if t.runs.is_empty() {
            escape(&t.content).replace('\n', br)
        } else {
            // Mixed styles: spans for the styled ranges (links become <a>).
            let mut out = String::new();
            for (piece, run) in t.pieces() {
                let text = escape(&piece).replace('\n', br);
                let Some(r) = run else {
                    out.push_str(&text);
                    continue;
                };
                let mut rd: Vec<Decl> = Vec::new();
                if let Some(f) = &r.family {
                    rd.push(d(format!("font-['{}']", f.replace(' ', "_")), format!("font-family: '{f}', sans-serif")));
                    if !self.fonts.contains(f) {
                        self.fonts.push(f.clone());
                    }
                }
                if let Some(w) = r.weight {
                    rd.push(d(format!("font-[{w}]"), format!("font-weight: {w}")));
                }
                if let Some(i) = r.italic {
                    rd.push(if i { d("italic", "font-style: italic") } else { d("not-italic", "font-style: normal") });
                }
                if let Some(s) = r.size {
                    rd.push(d(format!("text-[{}]", px(s)), format!("font-size: {}", px(s))));
                }
                if let Some(c) = r.color {
                    let c = hex(Color(c));
                    rd.push(d(format!("text-[{c}]"), format!("color: {c}")));
                }
                if let Some(dec) = r.decoration {
                    rd.push(match dec {
                        Decoration::None => d("no-underline", "text-decoration: none"),
                        Decoration::Underline => d("underline", "text-decoration: underline"),
                        Decoration::Strikethrough => d("line-through", "text-decoration: line-through"),
                    });
                }
                let (tag, href) = match &r.link {
                    Some(url) => ("a", format!(" href=\"{}\"", url.replace('"', "&quot;"))),
                    None => ("span", String::new()),
                };
                let style = match self.framework {
                    Framework::ReactTailwind => format!(" className=\"{}\"", rd.iter().map(|x| x.tw.clone()).collect::<Vec<_>>().join(" ")),
                    Framework::Html => format!(" style=\"{}\"", rd.iter().map(|x| x.css.clone()).collect::<Vec<_>>().join("; ")),
                };
                let _ = write!(out, "<{tag}{href}{style}>{text}</{tag}>");
            }
            out
        };
        let attrs = self.attrs(n, decls);
        let _ = writeln!(self.out, "{pad}<p{attrs}>{content}</p>");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::AutoLayout;
    use crate::scene::tests::{frame, rect};
    use crate::{Node, Text};

    #[test]
    fn auto_layout_becomes_flex_and_text_gets_typography() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Card Row", 0.0, 0.0, 300.0, 100.0));
        scene.update(f, |n| {
            n.fill = Color::rgba(255, 255, 255, 255);
            n.layout = Some(Box::new(AutoLayout { gap: 12.0, padding: [8.0; 4], align: Align::Center, ..AutoLayout::default() }));
        });
        let a = scene.insert(rect("Swatch", 0.0, 0.0, 40.0, 40.0).radius(6.0), Some(f), None);
        scene.update(a, |n| n.sizing = [Sizing::Fill, Sizing::Fixed]);
        scene.insert(Node::text(Text::new("Hi <you>", 16.0, 600), 0.0, 0.0, Color::rgba(17, 24, 39, 255)), Some(f), None);
        let g = scene.generate_code(f, Framework::ReactTailwind, "/assets/").unwrap();
        assert!(g.code.contains("export default function CardRow()"));
        assert!(g.code.contains("className=\"w-[300px] h-[100px] bg-[#ffffff] flex gap-x-[12px]  items-center pt-[8px]") || g.code.contains("flex gap-x-[12px]"));
        assert!(g.code.contains("flex-1"));
        assert!(g.code.contains("text-[16px] font-[600]"));
        assert!(g.code.contains("Hi &lt;you&gt;"));
        assert_eq!(g.fonts, ["Inter"]);
        let html = scene.generate_code(f, Framework::Html, "/assets/").unwrap();
        assert!(html.code.contains("display: flex"));
        assert!(html.code.contains("border-radius: 6px"));
    }

    #[test]
    fn free_form_children_are_positioned_absolutely() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Screen", 100.0, 100.0, 400.0, 300.0));
        scene.insert(rect("Box", 120.0, 150.0, 50.0, 50.0), Some(f), None);
        let g = scene.generate_code(f, Framework::ReactTailwind, "/assets/").unwrap();
        assert!(g.code.contains("relative"));
        assert!(g.code.contains("absolute left-[20px] top-[50px]"));
    }

    #[test]
    fn icons_are_referenced_as_files_and_gradients_become_css() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Toolbar", 0.0, 0.0, 200.0, 40.0));
        scene.update(f, |n| {
            n.paint = Some(Box::new(Paint::Gradient(Gradient::new(GradientKind::Linear, Color(0xffffffff), Color(0xff000000)))));
            n.effects = vec![Effect::DropShadow { color: Color(0x40000000), x: 0.0, y: 4.0, blur: 8.0, spread: 0.0, visible: true }];
        });
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2h20v20H2z"/></svg>"#;
        let mut t = crate::import_svg(svg, "lucide:search", &mut scene.images.clone()).unwrap().tree;
        t.translate(8.0, 8.0);
        scene.insert_tree(&t, Some(f), None);
        let g = scene.generate_code(f, Framework::ReactTailwind, "http://127.0.0.1:3855/assets/").unwrap();
        assert!(g.code.contains("const imgSearch = \"http://127.0.0.1:3855/assets/search.svg\";"), "{}", g.code);
        assert!(g.code.contains("src={imgSearch}"));
        assert!(g.code.contains("left-[8px] top-[8px] w-[24px] h-[24px]"));
        assert!(g.code.contains("bg-[linear-gradient(180deg,#ffffff_0%,#000000_100%)]"), "{}", g.code);
        assert!(g.code.contains("shadow-[0px_4px_8px_0px_#00000040]"));
        let html = scene.generate_code(f, Framework::Html, "/assets/").unwrap();
        assert!(html.code.contains("src=\"/assets/search.svg\""));
        assert_eq!(html.assets.len(), 1);
    }
}
