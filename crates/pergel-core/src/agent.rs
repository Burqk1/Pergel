//! What AI agents see and do through MCP: a compact layer tree, the design tokens in use, and
//! node specs (an LLM-friendly JSON shape) that build new layers. Positions in specs and in
//! the metadata are relative to the parent frame, as in Figma's API.

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::components::Component;
use crate::geom::{Color, Rect};
use crate::paint::{Cap, EndPoint, Gradient, GradientKind, ImageFill, ImageMode, Join, Paint, Stop, VectorPath};
use crate::style::Effect;
use crate::layout::{Align, AutoLayout, Flow, Justify, Sizing};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::style::StrokeAlign;
use crate::text::{LineHeight, Text, TextAlign, TextResize, VerticalAlign};

const METADATA_LIMIT: usize = 3000;

pub fn parse_color(s: &str) -> Option<Color> {
    let h = s.trim().trim_start_matches('#');
    let h = match h.len() {
        3 | 4 => h.chars().flat_map(|c| [c, c]).collect::<String>(),
        _ => h.to_string(),
    };
    let v = u32::from_str_radix(&h, 16).ok()?;
    match h.len() {
        6 => Some(Color(0xff00_0000 | v)),
        8 => Some(Color(v.rotate_right(8))),
        _ => None,
    }
}

fn hex(c: Color) -> String {
    let a = c.argb() >> 24;
    if a == 255 { format!("#{:06X}", c.argb() & 0xffffff) } else { format!("#{:06X}{a:02X}", c.argb() & 0xffffff) }
}

fn attr(s: &str) -> String {
    s.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;").replace('\n', " ")
}

fn num(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

pub fn type_name(n: &Node) -> &'static str {
    match (n.component, n.kind) {
        (Some(Component::Main), _) => "component",
        (Some(Component::Instance(_)), _) => "instance",
        (_, Kind::Frame) if n.section => "section",
        (_, Kind::Rectangle) if n.slice => "slice",
        (_, Kind::Frame) => "frame",
        (_, Kind::Group) => "group",
        (_, Kind::Rectangle) => "rectangle",
        (_, Kind::Ellipse) => "ellipse",
        (_, Kind::Text) => "text",
        (_, Kind::Vector) => "vector",
    }
}

impl Scene {
    /// The coordinate origin children of `id` are positioned against (groups don't have one).
    fn origin_of(&self, parent: Option<NodeId>) -> (f32, f32) {
        let mut cur = parent;
        while let Some(p) = cur {
            let Some(n) = self.node(p) else { break };
            if n.kind != Kind::Group {
                return (n.bounds.x, n.bounds.y);
            }
            cur = n.parent;
        }
        (0.0, 0.0)
    }

    /// Figma-style `get_metadata`: layer types, ids, names, positions and sizes as XML.
    pub fn metadata_xml(&self, root: Option<NodeId>) -> String {
        let mut out = String::new();
        let mut count = 0;
        let roots: Vec<NodeId> = match root {
            Some(id) => vec![id],
            None => self.roots().to_vec(),
        };
        if root.is_none() {
            out.push_str("<page name=\"Page 1\">\n");
        }
        for id in roots {
            self.metadata_node(id, if root.is_none() { 1 } else { 0 }, &mut out, &mut count);
        }
        if root.is_none() {
            out.push_str("</page>\n");
        }
        if count > METADATA_LIMIT {
            let _ = writeln!(out, "<!-- truncated after {METADATA_LIMIT} layers; ask for a node id to see more -->");
        }
        out
    }

    fn metadata_node(&self, id: NodeId, depth: usize, out: &mut String, count: &mut usize) {
        let Some(n) = self.node(id) else { return };
        *count += 1;
        if *count > METADATA_LIMIT {
            return;
        }
        let (ox, oy) = self.origin_of(n.parent);
        let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
        let tag = type_name(n);
        let pad = "  ".repeat(depth);
        let _ = write!(
            out,
            "{pad}<{tag} id=\"{}\" name=\"{}\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"",
            n.id,
            attr(&n.name),
            num(b.x - ox),
            num(b.y - oy),
            num(b.w),
            num(b.h)
        );
        if n.rotation != 0.0 {
            let _ = write!(out, " rotation=\"{}\"", num(n.rotation));
        }
        if !n.visible {
            out.push_str(" hidden=\"true\"");
        }
        if let Some(Component::Instance(main)) = n.component {
            match self.variant_set_of(main) {
                Some(set) => {
                    let set_name = self.node(set).map_or(String::new(), |s| s.name.clone());
                    let variant = self.node(main).map_or(String::new(), |m| m.name.clone());
                    let _ = write!(out, " main-component=\"{}\" variant=\"{}\" main-id=\"{main}\"", attr(&set_name), attr(&variant));
                }
                None => {
                    let name = self.node(main).map_or("(deleted)".to_string(), |m| m.name.clone());
                    let _ = write!(out, " main-component=\"{}\" main-id=\"{main}\"", attr(&name));
                }
            }
        }
        if n.ready {
            out.push_str(" ready-for-dev=\"true\"");
        }
        if let Some(t) = n.text.as_deref() {
            if t.list != crate::text::ListStyle::None {
                let _ = write!(out, " list=\"{}\"", if t.list == crate::text::ListStyle::Numbered { "numbered" } else { "bullet" });
            }
            if let Some(f) = t.feature_css() {
                let _ = write!(out, " opentype=\"{}\"", attr(&f));
            }
        }
        if n.text.as_deref().is_some_and(|t| t.path.is_some()) {
            out.push_str(" text-on-path=\"true\"");
        }
        if let Some([t, r, b, l]) = n.sides.filter(|_| n.stroke.is_some()) {
            let _ = write!(out, " stroke-sides=\"{} {} {} {}\"", num(t), num(r), num(b), num(l));
        }
        if !n.scroll.is_none() {
            let _ = write!(out, " overflow-scroll=\"{}\"", n.scroll.label());
        }
        if let Some(v) = n.vector.as_deref() {
            if !v.start.is_none() {
                let _ = write!(out, " start-point=\"{}\"", v.start.label());
            }
            if !v.end.is_none() {
                let _ = write!(out, " end-point=\"{}\"", v.end.label());
            }
        }
        if let Some(note) = n.note.as_deref() {
            let _ = write!(out, " annotation=\"{}\"", attr(note));
        }
        if !n.grids.is_empty() {
            let g: Vec<String> = n.grids.iter().filter(|g| g.visible).map(|g| g.summary()).collect();
            if !g.is_empty() {
                let _ = write!(out, " layout-grid=\"{}\"", attr(&g.join("; ")));
            }
        }
        if !n.reactions.is_empty() {
            let list: Vec<String> = n
                .reactions
                .iter()
                .map(|r| {
                    let t = match r.trigger {
                        crate::prototype::Trigger::OnClick => "click".to_string(),
                        crate::prototype::Trigger::OnHover => "hover".to_string(),
                        crate::prototype::Trigger::AfterDelay { ms } => format!("after {ms}ms"),
                    };
                    let a = match &r.action {
                        crate::prototype::Action::Navigate { to } => format!("→ {}", self.node(*to).map_or("?".to_string(), |n| n.name.clone())),
                        crate::prototype::Action::Back => "→ back".to_string(),
                        crate::prototype::Action::OpenUrl { url } => format!("→ {url}"),
                        other => other.describe(&|id| self.node(id).map_or("?".to_string(), |n| n.name.clone()), &|v| v.to_string()),
                    };
                    format!("{t} {a}")
                })
                .collect();
            let _ = write!(out, " interactions=\"{}\"", attr(&list.join("; ")));
        }
        if n.variant_set {
            let props: Vec<String> = self.variant_props(n.id).into_iter().map(|(k, v)| format!("{k}: {}", v.join("|"))).collect();
            let _ = write!(out, " component-set=\"{}\"", attr(&props.join("; ")));
        }
        if !n.extra_fills.is_empty() {
            let _ = write!(out, " fills=\"{}\"", n.extra_fills.len() + 1);
        }
        if let Some(defs) = self.prop_defs_summary(n.id) {
            let _ = write!(out, " component-properties=\"{}\"", attr(&defs));
        }
        if matches!(n.component, Some(Component::Instance(_)))
            && let Some(values) = self.prop_summary(n.id)
        {
            let _ = write!(out, " properties=\"{}\"", attr(&values));
        }
        if let Some(l) = n.layout.as_deref() {
            let flow = match l.flow {
                Flow::Horizontal => "horizontal",
                Flow::Vertical => "vertical",
                Flow::Wrap => "wrap",
                Flow::Grid => "grid",
            };
            let _ = write!(out, " auto-layout=\"{flow}\"");
        }
        match n.paint.as_deref() {
            Some(Paint::Image(img)) => {
                let _ = write!(out, " image-fill=\"{}x{}\"", img.width, img.height);
                if let Some(f) = img.adjust.css_filter() {
                    let _ = write!(out, " image-adjust=\"{}\"", attr(&f));
                }
            }
            Some(Paint::Gradient(g)) => {
                let _ = write!(out, " gradient=\"{}\"", g.kind.label().to_ascii_lowercase());
            }
            None => {}
        }
        if let Some(t) = n.text.as_deref() {
            let mut s: String = t.content.chars().take(120).collect();
            if t.content.chars().count() > 120 {
                s.push('…');
            }
            let _ = write!(out, " characters=\"{}\"", attr(&s));
        }
        if n.children.is_empty() {
            out.push_str(" />\n");
            return;
        }
        out.push_str(">\n");
        for c in &n.children {
            self.metadata_node(*c, depth + 1, out, count);
        }
        let _ = writeln!(out, "{pad}</{tag}>");
    }

    /// The design tokens a subtree uses (colours, type styles, radii, spacing), for
    /// `get_variable_defs`. Pergel has no variables yet, so these are the raw values.
    pub fn design_tokens(&self, root: Option<NodeId>) -> Value {
        let ids: Vec<NodeId> = match root {
            Some(id) => self.subtree(id),
            None => self.nodes().map(|n| n.id).collect(),
        };
        let mut colors: BTreeMap<String, usize> = BTreeMap::new();
        let mut type_styles: BTreeMap<String, usize> = BTreeMap::new();
        let mut radii: BTreeMap<String, usize> = BTreeMap::new();
        let mut spacing: BTreeMap<String, usize> = BTreeMap::new();
        for id in ids {
            let Some(n) = self.node(id) else { continue };
            if n.fill_on && n.kind != Kind::Group && n.fill.argb() >> 24 != 0 {
                *colors.entry(hex(n.fill)).or_default() += 1;
            }
            if let Some((c, _)) = n.stroke {
                *colors.entry(hex(c)).or_default() += 1;
            }
            if let Some(t) = n.text.as_deref() {
                let lh = match t.line_height {
                    LineHeight::Auto => "auto".to_string(),
                    LineHeight::Px(v) => format!("{}px", num(v)),
                    LineHeight::Percent(p) => format!("{}%", num(p)),
                };
                let key = format!(
                    "{} {} {}px / {lh}{}",
                    t.family,
                    t.weight_name(),
                    num(t.size),
                    if t.letter_spacing != 0.0 { format!(", letter-spacing {}%", num(t.letter_spacing)) } else { String::new() }
                );
                *type_styles.entry(key).or_default() += 1;
            }
            if n.radius > 0.0 && n.kind != Kind::Ellipse {
                *radii.entry(num(n.radius)).or_default() += 1;
            }
            if let Some(l) = n.layout.as_deref() {
                *spacing.entry(format!("gap {}", num(l.gap))).or_default() += 1;
                if l.padding.iter().any(|p| *p > 0.0) {
                    let p = l.padding.map(num).join(" ");
                    *spacing.entry(format!("padding {p}")).or_default() += 1;
                }
            }
        }
        serde_json::json!({
            "variables": {},
            "colors": colors,
            "typography": type_styles,
            "cornerRadius": radii,
            "spacing": spacing,
            "note": "Raw values with their use counts. Named variables, when the design has them, are listed under \"variables\" (see get_variable_defs).",
        })
    }

    /// Builds layers from specs under `parent` (on the canvas when `None`); returns their ids.
    pub fn build_specs(&mut self, specs: &[NodeSpec], parent: Option<NodeId>, index: Option<usize>) -> Vec<NodeId> {
        self.build_specs_reporting(specs, parent, index).0
    }

    /// Like `build_specs`, also returning problems worth telling the agent about.
    pub fn build_specs_reporting(&mut self, specs: &[NodeSpec], parent: Option<NodeId>, index: Option<usize>) -> (Vec<NodeId>, Vec<String>) {
        let mut ids = Vec::new();
        let mut warnings = Vec::new();
        for (k, spec) in specs.iter().enumerate() {
            ids.push(self.build_spec(spec, parent, index.map(|i| i + k), &mut warnings));
        }
        (ids, warnings)
    }

    /// An SVG (or resolved icon) spec: the artwork as a frame of vectors, sized and recoloured.
    fn build_svg(&mut self, spec: &NodeSpec, svg: &str, parent: Option<NodeId>, index: Option<usize>, warnings: &mut Vec<String>) -> Option<NodeId> {
        let (ox, oy) = self.origin_of(parent);
        let name = spec.name.clone().or_else(|| spec.icon.clone()).unwrap_or_else(|| "SVG".into());
        let mut images = std::mem::take(&mut self.images);
        let imported = crate::import_svg(svg, &name, &mut images);
        self.images = images;
        let mut tree = match imported {
            Ok(i) => i.tree,
            Err(e) => {
                warnings.push(format!("{name}: {e}"));
                return None;
            }
        };
        let (w, h) = (tree.bounds.w.max(0.01), tree.bounds.h.max(0.01));
        let (tw, th) = match (spec.width, spec.height) {
            (Some(tw), Some(th)) => (tw, th),
            (Some(tw), None) => (tw, tw * h / w),
            (None, Some(th)) => (th * w / h, th),
            (None, None) => (w, h),
        };
        if (tw, th) != (w, h) {
            tree.scale(tw / w, th / h);
        }
        if let Some(c) = spec.color.as_deref().and_then(parse_color) {
            tree.recolor(c);
        }
        tree.translate(ox + spec.x, oy + spec.y);
        Some(self.insert_tree(&tree, parent, index))
    }

    fn build_spec(&mut self, spec: &NodeSpec, parent: Option<NodeId>, index: Option<usize>, warnings: &mut Vec<String>) -> NodeId {
        let (ox, oy) = self.origin_of(parent);
        let kind = spec.kind();
        if let Some(svg) = spec.svg.clone() {
            if let Some(id) = self.build_svg(spec, &svg, parent, index, warnings) {
                if let Some(o) = spec.opacity {
                    self.update(id, |n| n.opacity = if o > 1.0 { o / 100.0 } else { o }.clamp(0.0, 1.0));
                }
                return id;
            }
        } else if let Some(icon) = &spec.icon {
            warnings.push(format!("Icon {icon:?} could not be fetched; a placeholder frame was made"));
        }
        let (x, y) = (ox + spec.x, oy + spec.y);
        let w = spec.width.unwrap_or(if kind == Kind::Text { 0.0 } else { 100.0 });
        let h = spec.height.unwrap_or(if kind == Kind::Text { 0.0 } else { 100.0 });
        let default_fill = match kind {
            Kind::Frame => Color::rgba(255, 255, 255, 255),
            Kind::Text => Color::rgba(0, 0, 0, 255),
            Kind::Group => Color(0),
            _ => Color::rgba(217, 217, 217, 255),
        };
        let mut node = if kind == Kind::Text {
            let mut n = Node::text(spec.text_style(), x, y, default_fill);
            n.bounds = Rect::new(x, y, w, h);
            n
        } else {
            Node::new(kind, spec.name.clone().unwrap_or_else(|| self.next_name(kind)), Rect::new(x, y, w, h), default_fill)
        };
        if kind == Kind::Vector {
            // Lines and arrows: a stroked polyline through `points` (or across width × height).
            let arrow = spec.kind.eq_ignore_ascii_case("arrow");
            let pts: Vec<(f32, f32)> = match spec.points.as_deref() {
                Some(p) if p.len() >= 2 => p.iter().map(|q| (ox + q[0], oy + q[1])).collect(),
                _ => vec![(x, y), (x + w, y + spec.height.unwrap_or(0.0))],
            };
            let (x0, y0) = pts.iter().fold((f32::MAX, f32::MAX), |a, p| (a.0.min(p.0), a.1.min(p.1)));
            let (x1, y1) = pts.iter().fold((f32::MIN, f32::MIN), |a, p| (a.0.max(p.0), a.1.max(p.1)));
            let closed = spec.closed.unwrap_or(false);
            let mut d: String = pts.iter().enumerate().map(|(i, p)| format!("{}{} {}", if i == 0 { "M" } else { "L" }, num(p.0 - x0), num(p.1 - y0))).collect();
            if closed {
                d.push('Z');
            }
            node.bounds = Rect::new(x0, y0, x1 - x0, y1 - y0);
            node.vector = Some(Box::new(VectorPath {
                d,
                size: (x1 - x0, y1 - y0),
                even_odd: false,
                cap: Cap::Round,
                join: Join::Round,
                start: spec.start_point.unwrap_or_default(),
                end: spec.end_point.unwrap_or(if arrow { EndPoint::LineArrow } else { EndPoint::None }),
            }));
            node.stroke_align = StrokeAlign::Center;
            if closed {
                node.stroke = None;
            } else {
                node.fill_on = false;
                node.stroke = Some((Color::rgba(0, 0, 0, 255), 1.0));
            }
            if spec.name.is_none() {
                node.name = self.next_name_for(if arrow { "Arrow" } else if closed { "Vector" } else { "Line" });
            }
        }
        if let Some(name) = &spec.name {
            node.name = name.clone();
            if let Some(t) = node.text.as_mut() {
                t.auto_name = false;
            }
        }
        match spec.fill.as_deref() {
            Some("none") | Some("transparent") => node.fill_on = false,
            Some(c) => {
                if let Some(c) = parse_color(c) {
                    node.fill = c;
                }
            }
            None if spec.kind.eq_ignore_ascii_case("component") => node.fill_on = false,
            None => {}
        }
        if let Some(s) = &spec.stroke {
            let color = s.color.as_deref().and_then(parse_color).unwrap_or(Color::rgba(0, 0, 0, 255));
            node.stroke = Some((color, s.width.unwrap_or(1.0)));
            node.dash = s.dash;
            if let Some(sides) = s.sides.filter(|_| matches!(kind, Kind::Rectangle | Kind::Frame)) {
                node.sides = Some(sides.map(|w| w.max(0.0)));
                node.stroke = Some((color, sides.iter().copied().fold(0.0, f32::max)));
            }
            node.stroke_align = match s.align.as_deref() {
                Some("center") => StrokeAlign::Center,
                Some("outside") => StrokeAlign::Outside,
                _ => StrokeAlign::Inside,
            };
        }
        if let Some(data) = spec.image.as_deref() {
            let raw = data.split_once("base64,").map_or(data, |(_, b)| b);
            use base64::Engine;
            match base64::engine::general_purpose::STANDARD.decode(raw.trim()).ok().and_then(|b| self.images.add(b)) {
                Some((hash, iw, ih)) => {
                    let mode = match spec.fit.as_deref() {
                        Some("fit") | Some("contain") => ImageMode::Fit,
                        Some("crop") | Some("stretch") => ImageMode::Crop,
                        Some("tile") => ImageMode::Tile,
                        _ => ImageMode::Fill,
                    };
                    if spec.width.is_none() && spec.height.is_none() {
                        node.bounds.w = iw as f32;
                        node.bounds.h = ih as f32;
                    } else if spec.height.is_none() {
                        node.bounds.h = node.bounds.w * ih as f32 / iw.max(1) as f32;
                    } else if spec.width.is_none() {
                        node.bounds.w = node.bounds.h * iw as f32 / ih.max(1) as f32;
                    }
                    node.paint = Some(Box::new(Paint::Image(ImageFill { hash, mode, width: iw, height: ih, scale: 1.0, opacity: 1.0, adjust: Default::default(), flip: [false, false] })));
                    node.fill_on = true;
                    if spec.name.is_none() {
                        node.name = "Image".into();
                    }
                }
                None => warnings.push(format!("{}: the image data is not a PNG, JPEG, GIF or WebP", node.name)),
            }
        }
        if let Some(g) = &spec.gradient {
            match g.to_gradient() {
                Some(grad) => {
                    node.fill = grad.stops[0].color;
                    node.fill_on = true;
                    node.paint = Some(Box::new(Paint::Gradient(grad)));
                }
                None => warnings.push(format!("{}: a gradient needs at least two stops", node.name)),
            }
        }
        if !spec.effects.is_empty() {
            node.effects = spec.effects.iter().filter_map(EffectSpec::to_effect).collect();
        }
        if let Some(r) = spec.radius {
            node.radius = r.max(0.0);
        }
        if let Some(note) = spec.note.as_deref().filter(|n| !n.trim().is_empty()) {
            node.note = Some(Box::new(note.trim().to_string()));
        }
        if !spec.layout_grids.is_empty() {
            node.grids = spec.layout_grids.clone();
        }
        if let Some(c) = &spec.constraints {
            let parse = |s: Option<&str>| match s.unwrap_or("").to_ascii_lowercase().as_str() {
                "right" | "bottom" | "max" => crate::layout::Constraint::Max,
                "left-right" | "top-bottom" | "stretch" | "both" => crate::layout::Constraint::Stretch,
                "center" => crate::layout::Constraint::Center,
                "scale" => crate::layout::Constraint::Scale,
                _ => crate::layout::Constraint::Min,
            };
            node.constraints = [parse(c.horizontal.as_deref()), parse(c.vertical.as_deref())];
        }
        if !spec.bind.is_empty() {
            let b: crate::variables::Bindings = spec
                .bind
                .iter()
                .filter_map(|(k, v)| serde_json::from_value(Value::String(k.clone())).ok().map(|p| (p, format!("name:{v}"))))
                .collect();
            if b.len() < spec.bind.len() {
                warnings.push(format!("{}: unknown bind property (use fill, stroke, strokeWidth, radius, opacity, gap, padding, width, height, fontSize, text, visible)", node.name));
            }
            node.bindings = (!b.is_empty()).then(|| Box::new(b));
        }
        if let Some(c) = spec.corners {
            node.corners = Some(c.map(|v| v.max(0.0)));
        }
        if let Some(o) = spec.opacity {
            node.opacity = if o > 1.0 { o / 100.0 } else { o }.clamp(0.0, 1.0);
        }
        if let Some(c) = spec.clip {
            node.clip = c;
        }
        if let Some(v) = spec.visible {
            node.visible = v;
        }
        if let Some(a) = spec.absolute {
            node.absolute = a;
        }
        if let Some(l) = &spec.layout {
            node.layout = Some(Box::new(l.to_layout()));
            node.sizing = [Sizing::Hug, Sizing::Hug];
            if spec.width.is_some() {
                node.sizing[0] = Sizing::Fixed;
            }
            if spec.height.is_some() {
                node.sizing[1] = Sizing::Fixed;
            }
        }
        if let Some(s) = &spec.sizing {
            if let Some(v) = s.horizontal.as_deref().and_then(parse_sizing) {
                node.sizing[0] = v;
            }
            if let Some(v) = s.vertical.as_deref().and_then(parse_sizing) {
                node.sizing[1] = v;
            }
        }
        if spec.kind.eq_ignore_ascii_case("component") {
            node.component = Some(Component::Main);
        }
        if let Some(s) = spec.scroll.filter(|_| kind == Kind::Frame) {
            node.scroll = s;
            node.clip = true;
        }
        if spec.kind.eq_ignore_ascii_case("section") {
            node.section = true;
            node.clip = spec.clip.unwrap_or(false);
            if spec.name.is_none() {
                node.name = self.next_name_for("Section");
            }
        }
        let sticky = spec.kind.eq_ignore_ascii_case("sticky");
        if sticky {
            // FigJam's sticky note: a square note that grows with its text.
            if spec.fill.is_none() {
                node.fill = Color::rgba(255, 243, 163, 255);
            }
            node.radius = spec.radius.unwrap_or(2.0);
            if spec.effects.is_empty() {
                node.effects = vec![Effect::DropShadow { color: Color::rgba(0, 0, 0, 38), x: 0.0, y: 2.0, blur: 6.0, spread: 0.0, visible: true }];
            }
            node.layout = Some(Box::new(AutoLayout { flow: Flow::Vertical, padding: [16.0; 4], gap: 8.0, ..AutoLayout::default() }));
            // A square note, as in FigJam.
            node.sizing = [Sizing::Fixed, Sizing::Fixed];
            node.bounds.w = spec.width.unwrap_or(240.0);
            node.bounds.h = spec.height.unwrap_or(node.bounds.w);
            if spec.name.is_none() {
                node.name = self.next_name_for("Sticky");
            }
        }
        if !spec.properties.is_empty() && (spec.kind.eq_ignore_ascii_case("component") || node.variant_set) {
            use crate::component_props::{ComponentProp, PropKind, PropValue, new_prop_id};
            for p in &spec.properties {
                let kind = match p.kind.to_ascii_lowercase().as_str() {
                    "boolean" | "bool" => PropKind::Boolean,
                    "instance-swap" | "instance_swap" | "swap" => PropKind::InstanceSwap,
                    _ => PropKind::Text,
                };
                let default = match (kind, &p.default) {
                    (PropKind::Boolean, d) => PropValue::Bool(d.as_ref().and_then(Value::as_bool).unwrap_or(true)),
                    (PropKind::Text, d) => PropValue::Text(d.as_ref().and_then(Value::as_str).unwrap_or("").to_string()),
                    (PropKind::InstanceSwap, d) => {
                        let by_name = d.as_ref().and_then(Value::as_str).and_then(|s| self.mains().into_iter().find(|m| self.node(*m).is_some_and(|n| n.name.eq_ignore_ascii_case(s))));
                        PropValue::Swap(d.as_ref().and_then(Value::as_u64).map(|i| i as NodeId).or(by_name).unwrap_or(0))
                    }
                };
                let pid = new_prop_id(&node.props);
                node.props.push(ComponentProp { id: pid, name: p.name.clone(), kind, default });
            }
        }
        for f in &spec.fills {
            let layer = match f {
                Value::String(c) => parse_color(c).map(|color| crate::paint::FillLayer { color, paint: None, visible: true }),
                other => serde_json::from_value::<GradientSpec>(other.clone()).ok().and_then(|g| g.to_gradient()).map(|g| crate::paint::FillLayer {
                    color: g.stops.first().map_or(Color(0xff00_0000), |s| s.color),
                    paint: Some(Paint::Gradient(g)),
                    visible: true,
                }),
            };
            match layer {
                Some(l) => node.extra_fills.push(l),
                None => warnings.push(format!("Fill {f} is not a colour or gradient")),
            }
        }
        let id = self.insert(node, parent, index);
        if sticky {
            let text = NodeSpec {
                kind: "text".into(),
                text: Some(spec.text.clone().unwrap_or_default()),
                font_size: Some(spec.font_size.unwrap_or(18.0)),
                font_weight: spec.font_weight,
                fill: Some("#1E1E1E".into()),
                auto_resize: Some("height".into()),
                width: Some(spec.width.unwrap_or(240.0) - 32.0),
                sizing: Some(SizingSpec { horizontal: Some("fill".into()), vertical: None }),
                name: Some("Note".into()),
                ..NodeSpec::default()
            };
            self.build_spec(&text, Some(id), None, warnings);
        }
        if let Some(r) = &spec.property {
            self.bind_props_by_name(id, r.visible.as_deref(), r.text.as_deref(), r.swap.as_deref(), warnings);
        }
        // Group members are positioned against the group's frame, like any frame's children.
        for child in &spec.children {
            self.build_spec(child, Some(id), None, warnings);
        }
        if let Some(r) = spec.rotation.filter(|r| *r != 0.0) {
            let (cx, cy) = self.node(id).map(|n| n.bounds.center()).unwrap_or_default();
            self.rotate(id, r, cx, cy);
        }
        id
    }
}

fn parse_sizing(s: &str) -> Option<Sizing> {
    match s.to_ascii_lowercase().as_str() {
        "fixed" => Some(Sizing::Fixed),
        "hug" | "hug-contents" | "auto" => Some(Sizing::Hug),
        "fill" | "fill-container" | "stretch" => Some(Sizing::Fill),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StrokeSpec {
    pub color: Option<String>,
    pub width: Option<f32>,
    /// [dash, gap] for a dashed stroke.
    pub dash: Option<[f32; 2]>,
    /// "inside" (default), "center" or "outside".
    pub align: Option<String>,
    /// Individual weights on rectangles and frames: [top, right, bottom, left].
    pub sides: Option<[f32; 4]>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutSpec {
    /// "horizontal" (default), "vertical", "wrap" or "grid".
    pub direction: Option<String>,
    pub gap: Option<f32>,
    pub counter_gap: Option<f32>,
    /// One number, [vertical, horizontal] or [top, right, bottom, left].
    pub padding: Option<Value>,
    /// Along the flow: "start", "center", "end" or "space-between".
    pub justify: Option<String>,
    /// Across the flow: "start", "center", "end" or "baseline".
    pub align: Option<String>,
    pub columns: Option<u16>,
    pub rows: Option<u16>,
}

impl LayoutSpec {
    fn to_layout(&self) -> AutoLayout {
        let mut l = AutoLayout { gap: self.gap.unwrap_or(0.0), ..AutoLayout::default() };
        l.flow = match self.direction.as_deref() {
            Some("vertical") | Some("column") => Flow::Vertical,
            Some("wrap") => Flow::Wrap,
            Some("grid") => Flow::Grid,
            _ => Flow::Horizontal,
        };
        l.counter_gap = self.counter_gap.unwrap_or(l.gap);
        l.padding = match &self.padding {
            Some(Value::Number(n)) => [n.as_f64().unwrap_or(0.0) as f32; 4],
            Some(Value::Array(a)) => {
                let v: Vec<f32> = a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect();
                match v.as_slice() {
                    [all] => [*all; 4],
                    [vert, hor] => [*vert, *hor, *vert, *hor],
                    [t, r, b, l] => [*t, *r, *b, *l],
                    _ => [0.0; 4],
                }
            }
            _ => [0.0; 4],
        };
        l.justify = match self.justify.as_deref() {
            Some("center") => Justify::Center,
            Some("end") => Justify::End,
            Some("space-between") | Some("between") => Justify::SpaceBetween,
            _ => Justify::Start,
        };
        l.align = match self.align.as_deref() {
            Some("center") => Align::Center,
            Some("end") => Align::End,
            Some("baseline") => Align::Baseline,
            _ => Align::Start,
        };
        l.columns = self.columns.unwrap_or(2);
        l.rows = self.rows.unwrap_or(2);
        l
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PropSpec {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub default: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct PropRefSpec {
    pub visible: Option<String>,
    pub text: Option<String>,
    pub swap: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SizingSpec {
    /// "fixed", "hug" or "fill".
    pub horizontal: Option<String>,
    pub vertical: Option<String>,
}

/// A gradient fill: `angle` (CSS degrees, 180 = top to bottom) for linear ones, and stops.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct GradientSpec {
    /// "linear" (default), "radial", "angular" or "diamond".
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub angle: Option<f32>,
    pub stops: Vec<StopSpec>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StopSpec {
    pub color: String,
    /// 0–1 (or 0–100).
    pub position: Option<f32>,
}

impl GradientSpec {
    pub fn to_gradient(&self) -> Option<Gradient> {
        let kind = match self.kind.as_deref() {
            Some("radial") => GradientKind::Radial,
            Some("angular") | Some("conic") => GradientKind::Angular,
            Some("diamond") => GradientKind::Diamond,
            _ => GradientKind::Linear,
        };
        let n = self.stops.len();
        if n < 2 {
            return None;
        }
        let stops = self
            .stops
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let at = s.position.map_or(i as f32 / (n - 1) as f32, |p| if p > 1.0 { p / 100.0 } else { p });
                Stop { at, color: parse_color(&s.color).unwrap_or(Color(0xff00_0000)) }
            })
            .collect();
        let mut g = Gradient::new(kind, Color(0), Color(0));
        g.stops = stops;
        if kind == GradientKind::Linear {
            // CSS angle: 0 points up, 90 right; the handles span the box through its centre.
            let a = self.angle.unwrap_or(180.0).to_radians();
            let (dx, dy) = (a.sin() / 2.0, -a.cos() / 2.0);
            g.from = (0.5 - dx, 0.5 - dy);
            g.to = (0.5 + dx, 0.5 + dy);
        }
        Some(g)
    }
}

/// A shadow or blur, as in Figma's Effects panel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EffectSpec {
    /// "drop-shadow" (default), "inner-shadow", "layer-blur" or "background-blur".
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// "#RRGGBBAA" (shadows).
    pub color: Option<String>,
    pub x: f32,
    pub y: f32,
    /// Blur radius as in Figma (CSS blur is half of it).
    pub blur: f32,
    pub spread: f32,
}

impl EffectSpec {
    fn to_effect(&self) -> Option<Effect> {
        let color = self.color.as_deref().and_then(parse_color).unwrap_or(Color::rgba(0, 0, 0, 64));
        let (x, y, blur, spread) = (self.x, self.y, self.blur.max(0.0), self.spread);
        Some(match self.kind.as_deref().unwrap_or("drop-shadow") {
            "drop-shadow" | "shadow" => Effect::DropShadow { color, x, y, blur, spread, visible: true },
            "inner-shadow" => Effect::InnerShadow { color, x, y, blur, spread, visible: true },
            "layer-blur" | "blur" => Effect::LayerBlur { radius: blur, visible: true },
            "background-blur" | "backdrop-blur" => Effect::BackgroundBlur { radius: blur, visible: true },
            _ => return None,
        })
    }
}

/// A layer to create. Only `type` is required.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct NodeSpec {
    /// "frame", "rectangle", "ellipse", "text", "group", "component", "svg", "icon" or "image".
    #[serde(rename = "type")]
    pub kind: String,
    /// SVG markup ("svg" layers): imported as vector layers, scaled to width/height.
    pub svg: Option<String>,
    /// An Iconify icon id such as "lucide:arrow-right" or "mdi:home" ("icon" layers).
    pub icon: Option<String>,
    /// Recolours an svg/icon (every fill and stroke), e.g. "#111827".
    pub color: Option<String>,
    /// Image file contents as base64 or a data: URI ("image" layers; Pergel resolves `src`).
    pub image: Option<String>,
    /// How an image fills its box: "fill" (cover, default), "fit", "crop" (stretch) or "tile".
    pub fit: Option<String>,
    pub gradient: Option<GradientSpec>,
    pub effects: Vec<EffectSpec>,
    /// Properties bound to variables by name: { "fill": "color/primary", "radius": "radius/md" }.
    pub bind: BTreeMap<String, String>,
    /// An annotation for developers.
    pub note: Option<String>,
    /// Layout guides on a frame: [{ "kind": "columns", "count": 12, "gutter": 24, "margin": 80 }].
    pub layout_grids: Vec<crate::style::LayoutGrid>,
    /// Frames: "vertical" | "horizontal" | "both" — the content scrolls inside the frame.
    pub scroll: Option<crate::prototype::Scroll>,
    /// More fills above `fill`/`gradient`, bottom to top: ["#FFFFFF33", { "type": "linear", … }].
    pub fills: Vec<Value>,
    /// Components: properties instances can set: [{ "name": "Label", "type": "text" | "boolean" |
    /// "instance-swap", "default": "Button" }].
    pub properties: Vec<PropSpec>,
    /// Layers inside a component: which property they show, by name:
    /// { "visible": "Show icon", "text": "Label", "swap": "Icon" }.
    pub property: Option<PropRefSpec>,
    /// Text: "bullet" or "numbered" lines.
    pub list: Option<String>,
    /// Text: OpenType features, e.g. { "tnum": 1, "liga": 0 }.
    pub features: BTreeMap<String, u32>,
    /// Lines and arrows: the points of the polyline, relative to the parent ([[x, y], …]).
    pub points: Option<Vec<[f32; 2]>>,
    /// Vectors: close the polyline into a filled shape (a diamond, a triangle…).
    pub closed: Option<bool>,
    /// Lines and arrows: "none" | "line-arrow" | "triangle-arrow" | "reversed-triangle" | "circle" | "diamond".
    pub start_point: Option<EndPoint>,
    pub end_point: Option<EndPoint>,
    /// How the layer follows its frame when the frame resizes:
    /// { "horizontal": "left|right|left-right|center|scale", "vertical": "top|bottom|top-bottom|center|scale" }.
    pub constraints: Option<SizingSpec>,
    pub name: Option<String>,
    /// Position relative to the parent frame.
    pub x: f32,
    pub y: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    /// "#RRGGBB", "#RRGGBBAA" or "none".
    pub fill: Option<String>,
    pub stroke: Option<StrokeSpec>,
    pub radius: Option<f32>,
    pub corners: Option<[f32; 4]>,
    /// 0–1.
    pub opacity: Option<f32>,
    /// Degrees, counter-clockwise.
    pub rotation: Option<f32>,
    pub clip: Option<bool>,
    pub visible: Option<bool>,
    pub absolute: Option<bool>,
    pub layout: Option<LayoutSpec>,
    pub sizing: Option<SizingSpec>,
    /// Text content (text layers).
    pub text: Option<String>,
    pub font_family: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<u16>,
    pub italic: Option<bool>,
    /// A number (px), "150%" or "auto".
    pub line_height: Option<Value>,
    /// Percent of the font size.
    pub letter_spacing: Option<f32>,
    /// "left", "center", "right" or "justify".
    pub text_align: Option<String>,
    /// "top", "middle" or "bottom".
    pub vertical_align: Option<String>,
    /// "width" (grow sideways, default without a width), "height" (wrap inside the width) or "none".
    pub auto_resize: Option<String>,
    pub children: Vec<NodeSpec>,
}

pub fn parse_line_height(v: &Value) -> Option<LineHeight> {
    match v {
        Value::Number(n) => n.as_f64().map(|v| LineHeight::Px(v as f32)),
        Value::String(s) if s.eq_ignore_ascii_case("auto") => Some(LineHeight::Auto),
        Value::String(s) if s.ends_with('%') => s.trim_end_matches('%').trim().parse().ok().map(LineHeight::Percent),
        Value::String(s) => s.trim_end_matches("px").trim().parse().ok().map(LineHeight::Px),
        _ => None,
    }
}

impl NodeSpec {
    fn kind(&self) -> Kind {
        match self.kind.to_ascii_lowercase().as_str() {
            "rectangle" | "rect" | "image" => Kind::Rectangle,
            "ellipse" | "circle" => Kind::Ellipse,
            "text" => Kind::Text,
            "group" => Kind::Group,
            "line" | "arrow" | "vector" => Kind::Vector,
            "sticky" => Kind::Frame,
            _ => Kind::Frame,
        }
    }

    fn text_style(&self) -> Text {
        let mut t = Text::new(self.text.clone().unwrap_or_default(), self.font_size.unwrap_or(16.0), self.font_weight.unwrap_or(400));
        if let Some(f) = &self.font_family {
            t.family = f.clone();
        }
        t.italic = self.italic.unwrap_or(false);
        if let Some(lh) = self.line_height.as_ref().and_then(parse_line_height) {
            t.line_height = lh;
        }
        t.letter_spacing = self.letter_spacing.unwrap_or(0.0);
        t.list = match self.list.as_deref() {
            Some("bullet") | Some("bulleted") | Some("disc") => crate::text::ListStyle::Bullet,
            Some("numbered") | Some("ordered") | Some("decimal") => crate::text::ListStyle::Numbered,
            _ => crate::text::ListStyle::None,
        };
        t.features = self.features.clone();
        t.align = match self.text_align.as_deref() {
            Some("center") => TextAlign::Center,
            Some("right") => TextAlign::Right,
            Some("justify") => TextAlign::Justify,
            _ => TextAlign::Left,
        };
        t.vertical = match self.vertical_align.as_deref() {
            Some("middle") | Some("center") => VerticalAlign::Middle,
            Some("bottom") => VerticalAlign::Bottom,
            _ => VerticalAlign::Top,
        };
        t.resize = match (self.auto_resize.as_deref(), self.width, self.height) {
            (Some("height"), _, _) => TextResize::AutoHeight,
            (Some("none"), _, _) => TextResize::Fixed,
            (Some("width"), _, _) => TextResize::AutoWidth,
            (None, Some(_), Some(_)) => TextResize::Fixed,
            (None, Some(_), None) => TextResize::AutoHeight,
            _ => TextResize::AutoWidth,
        };
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn specs_build_nested_layers_relative_to_their_frame() {
        let mut scene = Scene::default();
        let spec: Vec<NodeSpec> = serde_json::from_str(
            r##"[{"type":"frame","name":"Card","x":100,"y":50,"width":320,"height":200,"fill":"#FFFFFF","radius":12,
                 "layout":{"direction":"vertical","gap":8,"padding":[16,20]},
                 "children":[{"type":"text","text":"Title","fontSize":20,"fontWeight":700,"fill":"#111827"},
                             {"type":"rectangle","name":"Bar","width":40,"height":4,"fill":"#0D99FF80",
                              "sizing":{"horizontal":"fill"}}]}]"##,
        )
        .unwrap();
        let ids = scene.build_specs(&spec, None, None);
        scene.relayout(&crate::KeepSize);
        let card = scene.node(ids[0]).unwrap();
        assert_eq!((card.bounds.x, card.bounds.w, card.radius), (100.0, 320.0, 12.0));
        assert_eq!(card.layout.as_ref().unwrap().padding, [16.0, 20.0, 16.0, 20.0]);
        let bar = scene.node(card.children[1]).unwrap();
        assert_eq!(bar.fill, Color(0x800d99ff));
        assert_eq!(bar.bounds.w, 280.0, "fill width inside the padding");
        let xml = scene.metadata_xml(None);
        assert!(xml.contains("<frame id=\"0\" name=\"Card\" x=\"100\" y=\"50\" width=\"320\" height=\"200\" auto-layout=\"vertical\">"));
        assert!(xml.contains("characters=\"Title\""));
        assert!(xml.contains("<rectangle id=\"2\" name=\"Bar\" x=\"20\""));
        let tokens = scene.design_tokens(Some(ids[0]));
        assert_eq!(tokens["colors"]["#FFFFFF"], 1);
        assert!(tokens["typography"].as_object().unwrap().keys().any(|k| k.contains("Inter Bold 20px")));
    }

    #[test]
    fn colors_parse_in_short_and_alpha_forms() {
        assert_eq!(parse_color("#fff"), Some(Color(0xffffffff)));
        assert_eq!(parse_color("0D99FF"), Some(Color(0xff0d99ff)));
        assert_eq!(parse_color("#11223344"), Some(Color(0x44112233)));
        assert_eq!(parse_color("nope"), None);
    }
}
