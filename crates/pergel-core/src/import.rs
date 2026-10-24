//! SVG import: an SVG document becomes a frame of vector layers (like Figma's SVG paste/drop),
//! with gradients, strokes, opacity and embedded raster images kept.

use std::sync::Arc;

use usvg::tiny_skia_path::{PathSegment, Point, Transform};

use crate::clipboard::Tree;
use crate::geom::{Color, Rect};
use crate::layout::Sizing;
use crate::paint::{Cap, Gradient, GradientKind, ImageFill, ImageMode, Images, Join, Paint, Stop, VectorPath, content_hash, fmt};
use crate::scene::Kind;
use crate::style::{Blend, StrokeAlign};

pub struct SvgImport {
    pub tree: Tree,
    /// How many vector and image layers were made.
    pub layers: usize,
}

/// Parses `svg` into a layer tree whose root frame sits at (0, 0) and is called `name`. Raster
/// images inside it are added to `images`.
pub fn import_svg(svg: &str, name: &str, images: &mut Images) -> Result<SvgImport, String> {
    let opt = usvg::Options::default();
    let doc = usvg::Tree::from_str(svg, &opt).map_err(|e| format!("Not a valid SVG: {e}"))?;
    let size = doc.size();
    let mut root = blank(Kind::Frame, name, Rect::new(0.0, 0.0, size.width(), size.height()));
    root.fill_on = false;
    root.fill = Color(0x00ff_ffff);
    let mut count = 0;
    convert_children(doc.root(), &mut root.children, images, &mut count);
    if count == 0 {
        return Err("The SVG has nothing Pergel can draw".into());
    }
    Ok(SvgImport { tree: root, layers: count })
}

pub(crate) fn blank(kind: Kind, name: &str, bounds: Rect) -> Tree {
    Tree {
        name: name.to_string(),
        kind,
        bounds,
        rotation: 0.0,
        radius: 0.0,
        corners: None,
        fill_on: true,
        stroke_align: StrokeAlign::Center,
        opacity: 1.0,
        blend: if kind.is_container() { Blend::PassThrough } else { Blend::Normal },
        effects: Vec::new(),
        exports: Vec::new(),
        lock_ratio: false,
        layout: None,
        sizing: [Sizing::Fixed; 2],
        absolute: false,
        main: None,
        is_main: false,
        fill: Color(0xff00_0000),
        stroke: None,
        visible: true,
        locked: false,
        clip: false,
        children: Vec::new(),
        text: None,
        paint: None,
        vector: None,
        mask: false,
        bindings: None,
        constraints: [crate::layout::Constraint::Min; 2],
        styles: None,
        variant_set: false,
        reactions: Vec::new(),
        code: None,
        grids: Vec::new(),
        dash: None,
        note: None,
        ready: false,
        section: false,
        scroll: Default::default(),
        sides: None,
        slice: false,
        props: Vec::new(),
        prop_refs: None,
        prop_values: Default::default(),
        extra_fills: Vec::new(),
        comments: Vec::new(),
        uid: None,
        library: None,
        saved_id: None,
    }
}

fn convert_children(group: &usvg::Group, out: &mut Vec<Tree>, images: &mut Images, count: &mut usize) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => {
                let mut kids = Vec::new();
                convert_children(g, &mut kids, images, count);
                if kids.is_empty() {
                    continue;
                }
                let opacity = g.opacity().get();
                // usvg already dropped groups that only carried a transform; keep named ones and
                // ones with their own opacity, flatten the rest.
                if g.id().is_empty() && kids.len() == 1 {
                    let mut kid = kids.pop().unwrap_or_else(|| blank(Kind::Group, "Group", Rect::default()));
                    kid.opacity *= opacity;
                    out.push(kid);
                    continue;
                }
                let Some(bounds) = Rect::union_all(kids.iter().map(|k| k.bounds)) else { continue };
                let mut t = blank(Kind::Group, if g.id().is_empty() { "Group" } else { g.id() }, bounds);
                t.opacity = opacity;
                t.children = kids;
                out.push(t);
            }
            usvg::Node::Path(p) => {
                if !p.is_visible() {
                    continue;
                }
                if let Some(t) = convert_path(p) {
                    *count += 1;
                    out.push(t);
                }
            }
            usvg::Node::Image(img) => {
                if !img.is_visible() {
                    continue;
                }
                let bytes = match img.kind() {
                    usvg::ImageKind::PNG(b) | usvg::ImageKind::JPEG(b) | usvg::ImageKind::GIF(b) | usvg::ImageKind::WEBP(b) => b.clone(),
                    usvg::ImageKind::SVG(_) => continue,
                };
                let bb = img.abs_bounding_box();
                let Some((hash, w, h)) = add_shared(images, bytes) else { continue };
                let mut t = blank(Kind::Rectangle, if img.id().is_empty() { "Image" } else { img.id() }, Rect::new(bb.x(), bb.y(), bb.width(), bb.height()));
                t.paint = Some(Paint::Image(ImageFill { hash, mode: ImageMode::Crop, width: w, height: h, scale: 1.0, opacity: 1.0, adjust: Default::default(), flip: [false, false] }));
                t.fill = Color(0xffd9_d9d9);
                *count += 1;
                out.push(t);
            }
            usvg::Node::Text(_) => {}
        }
    }
}

fn add_shared(images: &mut Images, bytes: Arc<Vec<u8>>) -> Option<(String, u32, u32)> {
    let size = imagesize::blob_size(&bytes).ok()?;
    let hash = content_hash(&bytes);
    if images.get(&hash).is_none() {
        images.insert(hash.clone(), bytes);
    }
    Some((hash, size.width as u32, size.height as u32))
}

fn convert_path(p: &usvg::Path) -> Option<Tree> {
    let ts = p.abs_transform();
    let path = (*p.data()).clone().transform(ts)?;
    let b = path.bounds();
    let bounds = Rect::new(b.x(), b.y(), b.width(), b.height());
    let mut d = String::new();
    let rel = |pt: Point| format!("{} {}", fmt(pt.x - bounds.x), fmt(pt.y - bounds.y));
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(a) => d.push_str(&format!("M{}", rel(a))),
            PathSegment::LineTo(a) => d.push_str(&format!("L{}", rel(a))),
            PathSegment::QuadTo(a, b) => d.push_str(&format!("Q{} {}", rel(a), rel(b))),
            PathSegment::CubicTo(a, b, c) => d.push_str(&format!("C{} {} {}", rel(a), rel(b), rel(c))),
            PathSegment::Close => d.push('Z'),
        }
    }
    let mut t = blank(Kind::Vector, if p.id().is_empty() { "Vector" } else { p.id() }, bounds);
    let mut vector = VectorPath { d, size: (bounds.w, bounds.h), even_odd: false, cap: Cap::Butt, join: Join::Miter, start: Default::default(), end: Default::default() };

    match p.fill() {
        Some(fill) => {
            vector.even_odd = fill.rule() == usvg::FillRule::EvenOdd;
            let alpha = fill.opacity().get();
            let (color, paint) = convert_paint(fill.paint(), alpha, ts, bounds);
            t.fill = color;
            t.paint = paint;
        }
        None => t.fill_on = false,
    }
    if let Some(stroke) = p.stroke() {
        let (color, _) = convert_paint(stroke.paint(), stroke.opacity().get(), ts, bounds);
        let (sx, sy) = ts.get_scale();
        let width = stroke.width().get() * (sx * sy).abs().sqrt();
        t.stroke = Some((color, width));
        vector.cap = match stroke.linecap() {
            usvg::LineCap::Butt => Cap::Butt,
            usvg::LineCap::Round => Cap::Round,
            usvg::LineCap::Square => Cap::Square,
        };
        vector.join = match stroke.linejoin() {
            usvg::LineJoin::Round => Join::Round,
            usvg::LineJoin::Bevel => Join::Bevel,
            _ => Join::Miter,
        };
    }
    t.vector = Some(vector);
    Some(t)
}

fn argb(c: usvg::Color, alpha: f32) -> Color {
    Color::rgba(c.red, c.green, c.blue, (alpha.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// A solid colour (for a gradient: its first stop) and, for gradients, the gradient itself with
/// handles normalised to the layer's box.
fn convert_paint(paint: &usvg::Paint, alpha: f32, ts: Transform, bounds: Rect) -> (Color, Option<Paint>) {
    let stops = |base: &[usvg::Stop]| -> Vec<Stop> {
        base.iter().map(|s| Stop { at: s.offset().get(), color: argb(s.color(), s.opacity().get() * alpha) }).collect()
    };
    let unit = |gt: Transform, x: f32, y: f32| {
        let mut pt = Point::from_xy(x, y);
        ts.pre_concat(gt).map_point(&mut pt);
        let nx = if bounds.w > 0.0 { (pt.x - bounds.x) / bounds.w } else { 0.5 };
        let ny = if bounds.h > 0.0 { (pt.y - bounds.y) / bounds.h } else { 0.5 };
        (nx, ny)
    };
    match paint {
        usvg::Paint::Color(c) => (argb(*c, alpha), None),
        usvg::Paint::LinearGradient(g) => {
            let s = stops(g.stops());
            let first = s.first().map_or(Color(0xff00_0000), |s| s.color);
            let gt = g.transform();
            let grad = Gradient { kind: GradientKind::Linear, stops: s, from: unit(gt, g.x1(), g.y1()), to: unit(gt, g.x2(), g.y2()), width: 1.0 };
            (first, Some(Paint::Gradient(grad)))
        }
        usvg::Paint::RadialGradient(g) => {
            let s = stops(g.stops());
            let first = s.first().map_or(Color(0xff00_0000), |s| s.color);
            let gt = g.transform();
            let r = g.r().get();
            let from = unit(gt, g.cx(), g.cy());
            let to = unit(gt, g.cx(), g.cy() + r);
            let side = unit(gt, g.cx() + r, g.cy());
            // Second axis relative to the first, in the box's pixel space.
            let len = |a: (f32, f32), b: (f32, f32)| (((b.0 - a.0) * bounds.w).powi(2) + ((b.1 - a.1) * bounds.h).powi(2)).sqrt();
            let main = len(from, to);
            let width = if main > 0.0 { len(from, side) / main } else { 1.0 };
            (first, Some(Paint::Gradient(Gradient { kind: GradientKind::Radial, stops: s, from, to, width })))
        }
        usvg::Paint::Pattern(_) => (Color::rgba(0, 0, 0, (alpha * 255.0) as u8), None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icons_import_as_a_frame_of_vectors() {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="#111827" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path id="arrow" d="M5 12h14"/><path d="m12 5 7 7-7 7"/>
            <circle cx="12" cy="12" r="4" fill="#ff0000" stroke="none"/>
            <rect x="0" y="0" width="24" height="24" fill="url(#g)" stroke="none" opacity="0.5"/>
            <defs><linearGradient id="g" x1="0" y1="0" x2="24" y2="0" gradientUnits="userSpaceOnUse"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
        </svg>"##;
        let mut images = Images::default();
        let out = import_svg(svg, "arrow-right", &mut images).unwrap();
        assert_eq!(out.tree.name, "arrow-right");
        assert_eq!((out.tree.bounds.w, out.tree.bounds.h), (24.0, 24.0));
        assert_eq!(out.layers, 4);
        let line = &out.tree.children[0];
        assert_eq!(line.kind, Kind::Vector);
        assert_eq!(line.name, "arrow");
        assert_eq!(line.bounds, Rect::new(5.0, 12.0, 14.0, 0.0));
        assert!(!line.fill_on);
        assert_eq!(line.stroke, Some((Color(0xff11_1827), 2.0)));
        assert_eq!(line.vector.as_ref().unwrap().cap, Cap::Round);
        let circle = &out.tree.children[2];
        assert_eq!(circle.fill, Color(0xffff_0000));
        assert_eq!(circle.bounds, Rect::new(8.0, 8.0, 8.0, 8.0));
        let Some(Paint::Gradient(g)) = &out.tree.children[3].paint else { panic!("gradient") };
        assert_eq!((g.from, g.to), ((0.0, 0.0), (1.0, 0.0)));
        assert!(import_svg("<nope", "x", &mut images).is_err());
    }
}
