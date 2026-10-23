//! SVG export of a layer subtree ("Copy as SVG", Export → SVG). Coordinates stay in world
//! space with the viewBox around the layer; rotations become transforms, clipping frames
//! become clip paths.

use std::fmt::Write;

use base64::Engine;

use crate::geom::{Color, Rect};
use crate::paint::{Cap, GradientKind, ImageMode, Join, Paint};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::style::Blend;
use crate::text::{LineHeight, TextAlign, TextCase};

fn paint(c: Color) -> String {
    format!("#{:06x}", c.argb() & 0xffffff)
}

fn alpha(c: Color) -> f32 {
    (c.argb() >> 24) as f32 / 255.0
}

fn n(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r.fract() == 0.0 { format!("{}", r as i64) } else { format!("{r}") }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn blend_css(b: Blend) -> Option<&'static str> {
    Some(match b {
        Blend::PassThrough | Blend::Normal => return None,
        Blend::Darken => "darken",
        Blend::Multiply => "multiply",
        Blend::ColorBurn => "color-burn",
        Blend::Lighten => "lighten",
        Blend::Screen => "screen",
        Blend::ColorDodge => "color-dodge",
        Blend::Overlay => "overlay",
        Blend::SoftLight => "soft-light",
        Blend::HardLight => "hard-light",
        Blend::Difference => "difference",
        Blend::Exclusion => "exclusion",
        Blend::Hue => "hue",
        Blend::Saturation => "saturation",
        Blend::Color => "color",
        Blend::Luminosity => "luminosity",
    })
}

struct Svg<'a> {
    scene: &'a Scene,
    defs: String,
    body: String,
    clips: usize,
}

impl Scene {
    /// How far an exported layer's artwork reaches past its box on every side: strokes centred
    /// on open paths and their arrowheads (frames that clip don't overflow).
    pub fn svg_overflow(&self, id: NodeId) -> f32 {
        let Some(root) = self.node(id) else { return 0.0 };
        if root.kind == Kind::Frame && root.clip {
            return 0.0;
        }
        self.subtree(id)
            .into_iter()
            .filter_map(|i| self.node(i))
            .filter(|n| n.visible && (n.kind == Kind::Vector || n.kind == Kind::Text))
            .filter_map(|n| {
                // Type on a path rises a line height above (and below) its outline.
                if let Some(t) = n.text.as_deref() {
                    return t.path.as_ref().map(|_| t.size * 1.2);
                }
                let (_, w) = n.stroke.filter(|s| s.1 > 0.0)?;
                let v = n.vector.as_deref()?;
                let heads = !(v.start.is_none() && v.end.is_none());
                Some(w / 2.0 + if heads { (w * 3.0).max(6.0) * 0.8 } else { 0.0 })
            })
            .fold(0.0f32, f32::max)
            .ceil()
    }

    pub fn to_svg(&self, id: NodeId) -> Option<String> {
        let root = self.node(id)?;
        let mut area = root.aabb();
        let pad = self.svg_overflow(id);
        if pad > 0.0 {
            area = Rect::new(area.x - pad, area.y - pad, area.w + 2.0 * pad, area.h + 2.0 * pad);
        }
        let mut svg = Svg { scene: self, defs: String::new(), body: String::new(), clips: 0 };
        svg.node(id, 1);
        let mut out = String::new();
        let _ = writeln!(
            out,
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"{} {} {} {}\" fill=\"none\">",
            n(area.w),
            n(area.h),
            n(area.x),
            n(area.y),
            n(area.w),
            n(area.h)
        );
        if !svg.defs.is_empty() {
            let _ = write!(out, "  <defs>\n{}  </defs>\n", svg.defs);
        }
        out.push_str(&svg.body);
        out.push_str("</svg>\n");
        Some(out)
    }
}

impl Svg<'_> {
    fn attrs(&self, node: &Node) -> String {
        let mut a = String::new();
        if node.opacity < 1.0 {
            let _ = write!(a, " opacity=\"{}\"", n(node.opacity));
        }
        if node.rotation != 0.0 {
            let (cx, cy) = node.bounds.center();
            let _ = write!(a, " transform=\"rotate({} {} {})\"", n(-node.rotation), n(cx), n(cy));
        }
        if let Some(b) = blend_css(node.blend) {
            let _ = write!(a, " style=\"mix-blend-mode:{b}\"");
        }
        a
    }

    fn fill_attrs(&mut self, node: &Node) -> String {
        if let Some(Paint::Gradient(g)) = node.paint.as_deref() {
            self.clips += 1;
            let id = format!("paint{}", self.clips);
            let b = node.bounds;
            let at = |p: (f32, f32)| (b.x + p.0 * b.w, b.y + p.1 * b.h);
            let (fx, fy) = at(g.from);
            let (tx, ty) = at(g.to);
            match g.kind {
                GradientKind::Linear => {
                    let _ = writeln!(
                        self.defs,
                        "    <linearGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\">",
                        n(fx),
                        n(fy),
                        n(tx),
                        n(ty)
                    );
                }
                _ => {
                    let r = ((tx - fx).powi(2) + (ty - fy).powi(2)).sqrt();
                    let angle = (ty - fy).atan2(tx - fx).to_degrees() - 90.0;
                    let _ = writeln!(
                        self.defs,
                        "    <radialGradient id=\"{id}\" gradientUnits=\"userSpaceOnUse\" cx=\"0\" cy=\"0\" r=\"1\" gradientTransform=\"translate({} {}) rotate({}) scale({} {})\">",
                        n(fx),
                        n(fy),
                        n(angle),
                        n(r * g.width),
                        n(r)
                    );
                }
            }
            for stop in g.sorted_stops() {
                let a = alpha(stop.color);
                let op = if a < 1.0 { format!(" stop-opacity=\"{}\"", n(a)) } else { String::new() };
                let _ = writeln!(self.defs, "      <stop offset=\"{}\" stop-color=\"{}\"{op} />", n(stop.at), paint(stop.color));
            }
            let close = if g.kind == GradientKind::Linear { "linearGradient" } else { "radialGradient" };
            let _ = writeln!(self.defs, "    </{close}>");
            return format!(" fill=\"url(#{id})\"");
        }
        if !node.fill_on || node.fill.argb() >> 24 == 0 {
            return " fill=\"none\"".into();
        }
        let a = alpha(node.fill);
        if a < 1.0 {
            format!(" fill=\"{}\" fill-opacity=\"{}\"", paint(node.fill), n(a))
        } else {
            format!(" fill=\"{}\"", paint(node.fill))
        }
    }

    fn shape(&mut self, node: &Node, pad: &str, inset: f32, paint_attrs: &str) {
        let b = node.bounds;
        let (x, y, w, h) = (b.x + inset, b.y + inset, (b.w - 2.0 * inset).max(0.0), (b.h - 2.0 * inset).max(0.0));
        match node.kind {
            Kind::Vector => {
                let Some(v) = node.vector.as_deref() else { return };
                let mut extra = String::new();
                if v.even_odd {
                    extra.push_str(" fill-rule=\"evenodd\"");
                }
                if paint_attrs.contains("stroke=") {
                    match v.cap {
                        Cap::Round => extra.push_str(" stroke-linecap=\"round\""),
                        Cap::Square => extra.push_str(" stroke-linecap=\"square\""),
                        Cap::Butt => {}
                    }
                    match v.join {
                        Join::Round => extra.push_str(" stroke-linejoin=\"round\""),
                        Join::Bevel => extra.push_str(" stroke-linejoin=\"bevel\""),
                        Join::Miter => {}
                    }
                }
                let _ = writeln!(self.body, "{pad}<path d=\"{}\"{extra}{paint_attrs} />", v.placed(b.x, b.y, b.w, b.h));
                if let Some((color, width)) = node.stroke.filter(|_| paint_attrs.contains("stroke=")) {
                    for (d, filled) in v.markers(b.x, b.y, b.w, b.h, width) {
                        if filled {
                            let _ = writeln!(self.body, "{pad}<path d=\"{d}\" fill=\"{}\" />", paint(color));
                        } else {
                            let _ = writeln!(
                                self.body,
                                "{pad}<path d=\"{d}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{extra} />",
                                paint(color),
                                n(width)
                            );
                        }
                    }
                }
            }
            Kind::Ellipse => {
                let _ = writeln!(
                    self.body,
                    "{pad}<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"{paint_attrs} />",
                    n(x + w / 2.0),
                    n(y + h / 2.0),
                    n(w / 2.0),
                    n(h / 2.0)
                );
            }
            _ => {
                let r = node.radii().map(|v| (v - inset).max(0.0));
                if r.iter().all(|v| *v == r[0]) {
                    let rx = if r[0] > 0.0 { format!(" rx=\"{}\"", n(r[0])) } else { String::new() };
                    let _ = writeln!(
                        self.body,
                        "{pad}<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{rx}{paint_attrs} />",
                        n(x),
                        n(y),
                        n(w),
                        n(h)
                    );
                } else {
                    let [tl, tr, br, bl] = r;
                    let d = format!(
                        "M{} {}H{}A{tr} {tr} 0 0 1 {} {}V{}A{br} {br} 0 0 1 {} {}H{}A{bl} {bl} 0 0 1 {} {}V{}A{tl} {tl} 0 0 1 {} {}Z",
                        n(x + tl),
                        n(y),
                        n(x + w - tr),
                        n(x + w),
                        n(y + tr),
                        n(y + h - br),
                        n(x + w - br),
                        n(y + h),
                        n(x + bl),
                        n(x),
                        n(y + h - bl),
                        n(y + tl),
                        n(x + tl),
                        n(y),
                        tr = n(tr),
                        br = n(br),
                        bl = n(bl),
                        tl = n(tl),
                    );
                    let _ = writeln!(self.body, "{pad}<path d=\"{d}\"{paint_attrs} />");
                }
            }
        }
    }

    fn node(&mut self, id: NodeId, depth: usize) {
        let Some(node) = self.scene.node(id).cloned() else { return };
        if !node.visible || node.slice {
            return;
        }
        let pad = "  ".repeat(depth);
        let attrs = self.attrs(&node);
        match node.kind {
            Kind::Text => self.text(&node, &pad, &attrs),
            Kind::Group => {
                let _ = writeln!(self.body, "{pad}<g{attrs}>");
                self.children(&node.children, depth + 1);
                let _ = writeln!(self.body, "{pad}</g>");
            }
            _ => {
                let _ = writeln!(self.body, "{pad}<g{attrs}>");
                let inner = format!("{pad}  ");
                let has_paint = node.paint.is_some();
                if node.fill_on && (has_paint || node.fill.argb() >> 24 != 0) {
                    if let Some(Paint::Image(img)) = node.paint.as_deref() {
                        self.image(&node, img, &inner);
                    } else {
                        let attrs = self.fill_attrs(&node);
                        self.shape(&node, &inner, 0.0, &attrs);
                    }
                }
                for f in node.extra_fills.iter().filter(|f| f.visible && node.fill_on) {
                    let mut layer = node.clone();
                    layer.fill = f.color;
                    layer.paint = f.paint.clone().map(Box::new);
                    match f.paint.as_ref() {
                        Some(Paint::Image(img)) => self.image(&layer, img, &inner),
                        _ => {
                            let attrs = self.fill_attrs(&layer);
                            self.shape(&layer, &inner, 0.0, &attrs);
                        }
                    }
                }
                if !node.children.is_empty() {
                    let clip = node.kind == Kind::Frame && node.clip;
                    if clip {
                        self.clips += 1;
                        let id = format!("clip{}", self.clips);
                        let saved = std::mem::take(&mut self.body);
                        let _ = writeln!(self.defs, "    <clipPath id=\"{id}\">");
                        self.shape(&node, "      ", 0.0, "");
                        let clip_body = std::mem::replace(&mut self.body, saved);
                        self.defs.push_str(&clip_body);
                        let _ = writeln!(self.defs, "    </clipPath>");
                        let _ = writeln!(self.body, "{inner}<g clip-path=\"url(#{id})\">");
                    }
                    self.children(&node.children, depth + if clip { 2 } else { 1 });
                    if clip {
                        let _ = writeln!(self.body, "{inner}</g>");
                    }
                }
                if let (Some([t, r, b, l]), Some((color, _))) = (node.sides, node.stroke) {
                    // Individual strokes: the ring between the outer and inner edges, filled.
                    let k = match node.stroke_align {
                        crate::style::StrokeAlign::Inside => 0.0,
                        crate::style::StrokeAlign::Center => 0.5,
                        crate::style::StrokeAlign::Outside => 1.0,
                    };
                    let bx = node.bounds;
                    let edge = |f: f32| (bx.x - l * f, bx.y - t * f, bx.right() + r * f, bx.bottom() + b * f);
                    let rect = |(x0, y0, x1, y1): (f32, f32, f32, f32)| format!("M{} {}H{}V{}H{}Z", n(x0), n(y0), n(x1), n(y1), n(x0));
                    let (outer, inside) = (edge(k), edge(k - 1.0));
                    let hole = if inside.2 > inside.0 && inside.3 > inside.1 { rect(inside) } else { String::new() };
                    let a = alpha(color);
                    let opacity = if a < 1.0 { format!(" fill-opacity=\"{}\"", n(a)) } else { String::new() };
                    let _ = writeln!(self.body, "{inner}<path d=\"{}{hole}\" fill-rule=\"evenodd\" fill=\"{}\"{opacity} />", rect(outer), paint(color));
                } else if let Some((color, width)) = node.stroke.filter(|s| s.1 > 0.0) {
                    let a = alpha(color);
                    let opacity = if a < 1.0 { format!(" stroke-opacity=\"{}\"", n(a)) } else { String::new() };
                    let dash = node.dash.map_or(String::new(), |[a, b]| format!(" stroke-dasharray=\"{} {}\"", n(a), n(b)));
                    let paint_attrs = format!(" stroke=\"{}\" stroke-width=\"{}\"{opacity}{dash}", paint(color), n(width));
                    let inset = if node.kind == Kind::Vector { 0.0 } else { node.stroke_align.inset(width) };
                    self.shape(&node, &inner, inset, &paint_attrs);
                }
                let _ = writeln!(self.body, "{pad}</g>");
            }
        }
    }

    /// Children in order; a mask layer becomes a clip path around the layers above it.
    fn children(&mut self, kids: &[NodeId], depth: usize) {
        let pad = "  ".repeat(depth);
        let mut open = 0;
        for c in kids {
            match self.scene.node(*c).filter(|n| n.mask && n.visible).cloned() {
                Some(m) => {
                    self.clips += 1;
                    let id = format!("mask{}", self.clips);
                    let saved = std::mem::take(&mut self.body);
                    let _ = writeln!(self.defs, "    <clipPath id=\"{id}\">");
                    self.shape(&m, "      ", 0.0, "");
                    let clip_body = std::mem::replace(&mut self.body, saved);
                    self.defs.push_str(&clip_body);
                    let _ = writeln!(self.defs, "    </clipPath>");
                    let _ = writeln!(self.body, "{pad}<g clip-path=\"url(#{id})\">");
                    open += 1;
                }
                None => self.node(*c, depth + open),
            }
        }
        for _ in 0..open {
            let _ = writeln!(self.body, "{pad}</g>");
        }
    }

    /// An image fill: the picture clipped to the layer's shape, sized by the fill mode.
    fn image(&mut self, node: &Node, img: &crate::ImageFill, pad: &str) {
        let Some(bytes) = self.scene.images.get(&img.hash) else { return };
        self.clips += 1;
        let id = format!("clip{}", self.clips);
        let saved = std::mem::take(&mut self.body);
        let _ = writeln!(self.defs, "    <clipPath id=\"{id}\">");
        self.shape(node, "      ", 0.0, "");
        let clip_body = std::mem::replace(&mut self.body, saved);
        self.defs.push_str(&clip_body);
        let _ = writeln!(self.defs, "    </clipPath>");
        let mime = match crate::image_extension(bytes) {
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/png",
        };
        let data = base64::engine::general_purpose::STANDARD.encode(bytes.as_slice());
        let b = node.bounds;
        let aspect = match img.mode {
            ImageMode::Fill => "xMidYMid slice",
            ImageMode::Fit => "xMidYMid meet",
            ImageMode::Crop | ImageMode::Tile => "none",
        };
        let mut op = if img.opacity < 1.0 { format!(" opacity=\"{}\"", n(img.opacity)) } else { String::new() };
        if let Some(m) = img.adjust.matrix() {
            self.clips += 1;
            let fid = format!("adjust{}", self.clips);
            let values: Vec<String> = m.iter().map(|v| n(*v)).collect();
            let _ = writeln!(
                self.defs,
                "    <filter id=\"{fid}\" color-interpolation-filters=\"sRGB\"><feColorMatrix type=\"matrix\" values=\"{}\" /></filter>",
                values.join(" ")
            );
            let _ = write!(op, " filter=\"url(#{fid})\"");
        }
        let _ = writeln!(
            self.body,
            "{pad}<image clip-path=\"url(#{id})\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"{aspect}\"{op} href=\"data:{mime};base64,{data}\" />",
            n(b.x),
            n(b.y),
            n(b.w),
            n(b.h)
        );
    }

    fn text(&mut self, node: &Node, pad: &str, attrs: &str) {
        let Some(t) = node.text.as_deref() else { return };
        if !node.fill_on {
            return;
        }
        let b = node.bounds;
        let line = match t.line_height {
            LineHeight::Auto => t.size * 1.21,
            other => crate::Text { line_height: other, ..t.clone() }.line_px(t.size),
        };
        let (anchor, x) = match t.align {
            TextAlign::Center => ("middle", b.x + b.w / 2.0),
            TextAlign::Right => ("end", b.x + b.w),
            _ => ("start", b.x),
        };
        let family = esc(&t.family);
        let style = if t.italic { " font-style=\"italic\"" } else { "" };
        let spacing = if t.letter_spacing != 0.0 { format!(" letter-spacing=\"{}\"", n(t.letter_spacing_px())) } else { String::new() };
        let decoration = match t.decoration {
            crate::Decoration::None => "",
            crate::Decoration::Underline => " text-decoration=\"underline\"",
            crate::Decoration::Strikethrough => " text-decoration=\"line-through\"",
        };
        let content = match t.case {
            TextCase::Original => t.content.clone(),
            _ => t.display(),
        };
        let a = alpha(node.fill);
        let fill_opacity = if a < 1.0 { format!(" fill-opacity=\"{}\"", n(a)) } else { String::new() };
        if let Some(p) = &t.path {
            // Type on a path: <textPath> along the outline.
            self.clips += 1;
            let id = format!("textpath{}", self.clips);
            let _ = writeln!(self.defs, "    <path id=\"{id}\" d=\"{}\" />", p.placed(b.x, b.y, b.w, b.h));
            let (offset, anchor) = match t.align {
                TextAlign::Center => ("50%", "middle"),
                TextAlign::Right => ("100%", "end"),
                _ => ("0", "start"),
            };
            let _ = writeln!(
                self.body,
                "{pad}<text{attrs} font-family=\"{family}, sans-serif\" font-size=\"{}\" font-weight=\"{}\"{style}{spacing}{decoration} fill=\"{}\"{fill_opacity}><textPath href=\"#{id}\" startOffset=\"{offset}\" text-anchor=\"{anchor}\">{}</textPath></text>",
                n(t.size),
                t.weight,
                paint(node.fill),
                esc(&content.replace('\n', " "))
            );
            return;
        }
        let _ = writeln!(
            self.body,
            "{pad}<text{attrs} font-family=\"{family}, sans-serif\" font-size=\"{}\" font-weight=\"{}\"{style}{spacing}{decoration}{} fill=\"{}\"{fill_opacity} text-anchor=\"{anchor}\">",
            n(t.size),
            t.weight,
            t.feature_css().map_or(String::new(), |f| format!(" style=\"font-feature-settings: {}\"", esc(&f))),
            paint(node.fill)
        );
        // Baseline of the first line: half the leading plus the ascent (about 0.8 of the size).
        let first = b.y + (line - t.size) / 2.0 + t.size * 0.8;
        for (i, l) in content.split('\n').enumerate() {
            let l = format!("{}{l}", t.marker(i).unwrap_or_default());
            let _ = writeln!(self.body, "{pad}  <tspan x=\"{}\" y=\"{}\">{}</tspan>", n(x), n(first + i as f32 * line), esc(&l));
        }
        let _ = writeln!(self.body, "{pad}</text>");
    }
}

impl Scene {
    /// Frame or group "Resize to fit": the box shrinks or grows around its children, which
    /// stay where they are.
    pub fn resize_to_fit(&mut self, id: NodeId) -> bool {
        let Some(node) = self.node(id) else { return false };
        if node.kind != Kind::Frame || node.children.is_empty() {
            return false;
        }
        let rotation = node.rotation;
        let Some(union) = crate::Rect::union_all(node.children.iter().filter_map(|c| self.node(*c)).map(|c| c.aabb()))
        else {
            return false;
        };
        self.set_geometry(id, union, rotation);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};
    use crate::{Rect, Text};

    #[test]
    fn arrow_files_include_the_stroke_and_head() {
        let mut scene = Scene::default();
        let mut n = Node::new(Kind::Vector, "Arrow", Rect::new(0.0, 0.0, 100.0, 0.0), Color(0));
        n.fill_on = false;
        n.stroke = Some((Color::rgba(0, 0, 0, 255), 2.0));
        n.vector = Some(Box::new(crate::ShapeTool::Arrow { down: true }.path(100.0, 0.0)));
        let id = scene.add(n);
        assert_eq!(scene.svg_overflow(id), 6.0);
        let svg = scene.to_svg_at_origin(id).unwrap();
        assert!(svg.contains("viewBox=\"-6 -6 112 12\""), "{svg}");
    }

    #[test]
    fn svg_has_shapes_clips_and_text() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Card", 10.0, 10.0, 200.0, 100.0));
        scene.update(f, |n| {
            n.fill = Color::rgba(255, 255, 255, 255);
            n.radius = 8.0;
        });
        scene.insert(rect("Box", 20.0, 20.0, 40.0, 40.0).stroke(Color::rgba(0, 0, 0, 255), 2.0), Some(f), None);
        scene.insert(Node::text(Text::new("Hi & bye", 16.0, 700), 70.0, 20.0, Color::rgba(17, 24, 39, 255)), Some(f), None);
        let svg = scene.to_svg(f).unwrap();
        assert!(svg.contains("viewBox=\"10 10 200 100\""));
        assert!(svg.contains("<clipPath id=\"clip1\">"));
        assert!(svg.contains("rx=\"8\" fill=\"#ffffff\""));
        assert!(svg.contains("stroke=\"#000000\" stroke-width=\"2\""));
        assert!(svg.contains("font-weight=\"700\""));
        assert!(svg.contains("Hi &amp; bye"));
    }

    #[test]
    fn resize_to_fit_hugs_children_without_moving_them() {
        let mut scene = Scene::default();
        let f = scene.add(frame("F", 0.0, 0.0, 500.0, 500.0));
        let a = scene.insert(rect("A", 50.0, 60.0, 10.0, 10.0), Some(f), None);
        scene.insert(rect("B", 100.0, 100.0, 20.0, 20.0), Some(f), None);
        assert!(scene.resize_to_fit(f));
        assert_eq!(scene.node(f).unwrap().bounds, Rect::new(50.0, 60.0, 70.0, 60.0));
        assert_eq!(scene.node(a).unwrap().bounds.x, 50.0);
    }
}
