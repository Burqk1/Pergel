mod text;

pub use text::{FontFamily, Typesetter};

use std::cell::RefCell;
use std::collections::HashMap;

use pergel_core::{Blend, ClipShape, Effect, GradientKind, ImageFill, ImageMode, Kind, Node, NodeId, Scene, Viewport};
use skia_safe::vertices::{Builder, BuilderFlags, VertexMode};
use skia_safe::canvas::SaveLayerRec;
use skia_safe::{
    BlendMode, BlurStyle, Canvas, ClipOp, Color, MaskFilter, Paint, PaintStyle, Path, PathBuilder, PathFillType, Point,
    Image, Matrix, PaintCap, PaintJoin, RRect, Rect, SamplingOptions, Shader, TileMode, Vector, Vertices, gradient,
    image_filters,
};

pub const BACKGROUND: Color = Color::from_argb(255, 30, 30, 30);
pub const SELECTION: Color = Color::from_argb(255, 13, 153, 255);
/// Components, instances and their layers are outlined in purple, as in Figma.
pub const COMPONENT: Color = Color::from_argb(255, 151, 71, 255);

const BATCH_PX: f32 = 3.0;
pub const HANDLE_PX: f32 = 7.0;
/// Text smaller than this on screen is drawn as a faint bar, like Figma's greeking.
const GREEK_PX: f32 = 3.0;

/// Text renders as a lighter block when zoomed far out.
fn greek(argb: u32) -> u32 {
    with_alpha(argb, 1.0 / 3.0)
}

/// Scales a colour's alpha by `factor`.
fn with_alpha(argb: u32, factor: f32) -> u32 {
    let a = ((argb >> 24) as f32 * factor).round().clamp(0.0, 255.0) as u32;
    (a << 24) | (argb & 0x00ff_ffff)
}

pub fn blend_mode(blend: Blend) -> BlendMode {
    match blend {
        Blend::PassThrough | Blend::Normal => BlendMode::SrcOver,
        Blend::Darken => BlendMode::Darken,
        Blend::Multiply => BlendMode::Multiply,
        Blend::ColorBurn => BlendMode::ColorBurn,
        Blend::Lighten => BlendMode::Lighten,
        Blend::Screen => BlendMode::Screen,
        Blend::ColorDodge => BlendMode::ColorDodge,
        Blend::Overlay => BlendMode::Overlay,
        Blend::SoftLight => BlendMode::SoftLight,
        Blend::HardLight => BlendMode::HardLight,
        Blend::Difference => BlendMode::Difference,
        Blend::Exclusion => BlendMode::Exclusion,
        Blend::Hue => BlendMode::Hue,
        Blend::Saturation => BlendMode::Saturation,
        Blend::Color => BlendMode::Color,
        Blend::Luminosity => BlendMode::Luminosity,
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FrameStats {
    pub drawn: u32,
    pub batched: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Overlay<'a> {
    pub selection: &'a [NodeId],
    pub hover: Option<NodeId>,
    pub marquee: Option<pergel_core::Rect>,
    /// The text layer being edited; its text is drawn by the editor overlay instead.
    pub editing: Option<NodeId>,
    /// Hide the selection box and handles (while dragging, like Figma).
    pub hide_selection: bool,
    /// Vector edit mode / pen tool: the outline being edited, in world coordinates.
    pub vector: Option<VectorOverlay<'a>>,
    /// Prototype connections (world coordinates), drawn when the Prototype tab is open.
    pub connections: &'a [((f32, f32), (f32, f32))],
    /// Smart guides while dragging (world coordinates).
    pub guides: &'a [((f32, f32), (f32, f32))],
    /// Show frames' layout guides (Shift+G).
    pub layout_grids: bool,
    /// Ruler guides: (vertical, position in world units).
    pub page_guides: &'a [(bool, f32)],
    /// Annotated layers' top-right corners (world), drawn as note pins.
    pub notes: &'a [(f32, f32)],
    /// Slices on screen (world boxes), drawn as dashed outlines.
    pub slices: &'a [pergel_core::Rect],
}

#[derive(Debug, Clone)]
pub struct VectorOverlay<'a> {
    pub paths: &'a [pergel_core::SubPath],
    /// Selected anchors as (sub-path, anchor).
    pub selected: &'a [(usize, usize)],
    /// The pen's rubber band from the last anchor to the pointer.
    pub rubber: Option<((f32, f32), (f32, f32))>,
}

#[derive(Default)]
pub struct Renderer {
    overview: Option<(u64, Vertices)>,
    batch: Vec<(Rect, u32)>,
    pub text: Typesetter,
    /// Decoded image fills by content hash (`None` when the bytes could not be decoded).
    images: HashMap<String, Option<Image>>,
}

impl Renderer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn draw(&mut self, canvas: &Canvas, scene: &Scene, view: &Viewport, dpr: f32, overlay: &Overlay) -> FrameStats {
        canvas.clear(BACKGROUND);
        canvas.save();
        let scale = view.zoom * dpr;
        canvas.scale((scale, scale));
        canvas.translate((-view.x, -view.y));

        // An empty page has no extent; it must not take the overview path (no mesh to build).
        let stats = if !scene.is_empty() && scene.max_extent() * scale < BATCH_PX {
            self.draw_overview(canvas, scene)
        } else {
            self.draw_detailed(canvas, scene, view, scale, overlay.editing)
        };

        canvas.restore();
        draw_overlay(canvas, scene, view, dpr, overlay);
        stats
    }

    /// Renders `area` of the scene into RGBA pixels at most `max` px on the long side. The result
    /// starts with the width and height as little-endian u32s.
    pub fn thumbnail(&mut self, scene: &Scene, area: pergel_core::Rect, max: f32) -> Option<Vec<u8>> {
        let scale = (max / area.w.max(area.h).max(1.0)).min(4.0);
        let (w, h) = ((area.w * scale).ceil().max(1.0) as i32, (area.h * scale).ceil().max(1.0) as i32);
        let mut surface = skia_safe::surfaces::raster_n32_premul((w, h))?;
        let mut view = Viewport::new(w as f32, h as f32);
        view.x = area.x;
        view.y = area.y;
        view.zoom = scale;
        {
            let canvas = surface.canvas();
            canvas.clear(Color::TRANSPARENT);
            canvas.save();
            canvas.scale((scale, scale));
            canvas.translate((-area.x, -area.y));
            self.draw_detailed(canvas, scene, &view, scale, None);
            canvas.restore();
        }
        let info = skia_safe::ImageInfo::new((w, h), skia_safe::ColorType::RGBA8888, skia_safe::AlphaType::Unpremul, None);
        let mut out = vec![0u8; 8 + (w * h * 4) as usize];
        out[..4].copy_from_slice(&(w as u32).to_le_bytes());
        out[4..8].copy_from_slice(&(h as u32).to_le_bytes());
        surface.read_pixels(&info, &mut out[8..], (w * 4) as usize, (0, 0)).then_some(out)
    }

    /// A one-page PDF of `area` (vector shapes and real text).
    pub fn pdf(&mut self, scene: &Scene, area: pergel_core::Rect) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        {
            let doc = skia_safe::pdf::new_document(&mut out, None);
            let mut page = doc.begin_page((area.w.max(1.0), area.h.max(1.0)), None);
            {
                let canvas = page.canvas();
                canvas.translate((-area.x, -area.y));
                let mut view = Viewport::new(area.w.max(1.0), area.h.max(1.0));
                view.x = area.x;
                view.y = area.y;
                self.draw_detailed(canvas, scene, &view, 1.0, None);
            }
            page.end_page().close();
        }
        (!out.is_empty()).then_some(out)
    }

    /// One PDF with a page per area (slide decks: one frame per page).
    pub fn pdf_pages(&mut self, scene: &Scene, areas: &[pergel_core::Rect]) -> Option<Vec<u8>> {
        let mut out: Vec<u8> = Vec::new();
        {
            let mut doc = skia_safe::pdf::new_document(&mut out, None);
            for area in areas {
                let mut page = doc.begin_page((area.w.max(1.0), area.h.max(1.0)), None);
                {
                    let canvas = page.canvas();
                    canvas.translate((-area.x, -area.y));
                    let mut view = Viewport::new(area.w.max(1.0), area.h.max(1.0));
                    view.x = area.x;
                    view.y = area.y;
                    self.draw_detailed(canvas, scene, &view, 1.0, None);
                }
                doc = page.end_page();
            }
            doc.close();
        }
        (!out.is_empty()).then_some(out)
    }

    fn draw_overview(&mut self, canvas: &Canvas, scene: &Scene) -> FrameStats {
        if self.overview.as_ref().is_none_or(|(generation, _)| *generation != scene.generation()) {
            let quads: Vec<(Rect, u32)> = scene
                .nodes()
                .filter(|n| n.kind != Kind::Group && !n.mask && !n.slice && n.fill_on && n.fill.argb() >> 24 != 0)
                .filter(|n| scene.reach(n.id).is_some())
                .map(|n| {
                    let argb = with_alpha(n.fill.argb(), scene.opacity(n.id));
                    (to_sk(&n.aabb()), if n.kind == Kind::Text { greek(argb) } else { argb })
                })
                .collect();
            if quads.is_empty() {
                self.overview = None;
                return FrameStats::default();
            }
            self.overview = Some((scene.generation(), mesh(&quads)));
        }
        if let Some((_, vertices)) = &self.overview {
            canvas.draw_vertices(vertices, BlendMode::Dst, &Paint::default());
        }
        let n = scene.len() as u32;
        FrameStats { drawn: n, batched: n }
    }

    fn draw_detailed(
        &mut self,
        canvas: &Canvas,
        scene: &Scene,
        view: &Viewport,
        scale: f32,
        editing: Option<NodeId>,
    ) -> FrameStats {
        let mut stats = FrameStats::default();
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        let mut stroke = Paint::default();
        stroke.set_anti_alias(true);
        stroke.set_style(PaintStyle::Stroke);
        self.batch.clear();

        for node in scene.visible(&view.world_area()) {
            if node.kind == Kind::Group || node.mask || node.slice {
                continue;
            }
            let Some(clips) = scene.reach(node.id) else { continue };
            // Layers fully inside their clipping frames need no clip, which keeps them batchable.
            let clips: Vec<ClipShape> = clips.into_iter().filter(|c| !c.encloses(node)).collect();
            if editing == Some(node.id) {
                continue;
            }
            let opacity = scene.opacity(node.id);
            if opacity <= 0.0 {
                continue;
            }
            stats.drawn += 1;
            let has_effects = node.effects.iter().any(|e| e.visible());
            let tiny = !has_effects && match node.kind {
                Kind::Text => node.text.as_ref().map_or(0.0, |t| t.size) * scale < GREEK_PX,
                _ => node.bounds.w.max(node.bounds.h) * scale < BATCH_PX,
            };
            if tiny && clips.is_empty() && matches!(node.blend, Blend::Normal | Blend::PassThrough) {
                if node.fill_on && node.fill.argb() >> 24 != 0 {
                    let argb = with_alpha(node.fill.argb(), opacity);
                    self.batch.push((to_sk(&node.aabb()), if node.kind == Kind::Text { greek(argb) } else { argb }));
                }
                stats.batched += 1;
                continue;
            }
            self.flush(canvas);
            canvas.save();
            for c in &clips {
                match c.mask.and_then(|m| scene.node(m)) {
                    Some(m) => {
                        canvas.clip_path(&world_path(m), ClipOp::Intersect, true);
                    }
                    None => apply_clip(canvas, c),
                }
            }
            if node.rotation != 0.0 {
                let (cx, cy) = node.bounds.center();
                canvas.rotate(-node.rotation, Some(Point::new(cx, cy)));
            }
            let rect = to_sk(&node.bounds);
            let blend = blend_mode(node.blend);
            let layer_blur = node.effects.iter().find_map(|e| match *e {
                Effect::LayerBlur { radius, visible: true } if radius > 0.0 => Some(radius),
                _ => None,
            });
            if let Some(radius) = layer_blur {
                let mut p = Paint::default();
                p.set_image_filter(image_filters::blur((radius / 2.0, radius / 2.0), None, None, None));
                canvas.save_layer(&SaveLayerRec::default().paint(&p));
            }
            self.draw_behind(canvas, node, rect, opacity);
            if node.kind == Kind::Text {
                let on_path = node.text.as_deref().and_then(|t| t.path.as_ref()).and_then(|p| Path::from_svg(p.placed(rect.left, rect.top, rect.width(), rect.height())));
                if node.fill_on {
                    match on_path {
                        Some(p) => self.text.paint_on_path(canvas, node, with_alpha(node.fill.argb(), opacity), &p),
                        None => self.text.paint(canvas, node, with_alpha(node.fill.argb(), opacity)),
                    }
                }
            } else {
                match node.paint.as_deref() {
                    _ if !node.fill_on => {}
                    Some(pergel_core::Paint::Image(img)) => self.draw_image(canvas, scene, node, rect, img, opacity, blend),
                    Some(pergel_core::Paint::Gradient(g)) => {
                        if let Some(shader) = gradient_shader(g, rect) {
                            fill.set_shader(shader);
                            fill.set_color(Color::BLACK);
                            fill.set_alpha_f(opacity);
                            fill.set_blend_mode(blend);
                            draw_shape(canvas, node, rect, 0.0, &fill);
                            fill.set_shader(None);
                        }
                    }
                    None if node.fill.argb() >> 24 != 0 => {
                        fill.set_color(Color::new(with_alpha(node.fill.argb(), opacity)));
                        fill.set_blend_mode(blend);
                        draw_shape(canvas, node, rect, 0.0, &fill);
                    }
                    None => {}
                }
                // More fills, bottom to top.
                for extra in node.extra_fills.iter().filter(|f| f.visible && node.fill_on) {
                    match &extra.paint {
                        Some(pergel_core::Paint::Image(img)) => self.draw_image(canvas, scene, node, rect, img, opacity, blend),
                        Some(pergel_core::Paint::Gradient(g)) => {
                            if let Some(shader) = gradient_shader(g, rect) {
                                fill.set_shader(shader);
                                fill.set_color(Color::BLACK);
                                fill.set_alpha_f(opacity);
                                fill.set_blend_mode(blend);
                                draw_shape(canvas, node, rect, 0.0, &fill);
                                fill.set_shader(None);
                            }
                        }
                        None => {
                            fill.set_color(Color::new(with_alpha(extra.color.argb(), opacity)));
                            fill.set_blend_mode(blend);
                            draw_shape(canvas, node, rect, 0.0, &fill);
                        }
                    }
                }
                if let Some((color, width)) = node.stroke
                    && width > 0.0
                {
                    stroke.set_color(Color::new(with_alpha(color.argb(), opacity)));
                    stroke.set_stroke_width(width);
                    stroke.set_blend_mode(blend);
                    stroke.set_path_effect(node.dash.and_then(|[a, b]| skia_safe::PathEffect::dash(&[a.max(0.1), b.max(0.1)], 0.0)));
                    if let Some(v) = node.vector.as_deref() {
                        stroke.set_stroke_cap(match v.cap {
                            pergel_core::Cap::Butt => PaintCap::Butt,
                            pergel_core::Cap::Round => PaintCap::Round,
                            pergel_core::Cap::Square => PaintCap::Square,
                        });
                        stroke.set_stroke_join(match v.join {
                            pergel_core::Join::Miter => PaintJoin::Miter,
                            pergel_core::Join::Round => PaintJoin::Round,
                            pergel_core::Join::Bevel => PaintJoin::Bevel,
                        });
                        draw_shape(canvas, node, rect, 0.0, &stroke);
                        let markers = v.markers(rect.left, rect.top, rect.width(), rect.height(), width);
                        if !markers.is_empty() {
                            stroke.set_path_effect(None);
                            let mut head = Paint::default();
                            head.set_anti_alias(true);
                            head.set_color(Color::new(with_alpha(color.argb(), opacity)));
                            head.set_blend_mode(blend);
                            for (d, filled) in markers {
                                if let Some(path) = Path::from_svg(&d) {
                                    canvas.draw_path(&path, if filled { &head } else { &stroke });
                                }
                            }
                        }
                        stroke.set_stroke_cap(PaintCap::Butt);
                        stroke.set_stroke_join(PaintJoin::Miter);
                    } else if let Some(ring) = node.sides.and_then(|s| sides_ring(node, rect, s)) {
                        let mut p = Paint::default();
                        p.set_anti_alias(true);
                        p.set_color(Color::new(with_alpha(color.argb(), opacity)));
                        p.set_blend_mode(blend);
                        canvas.draw_path(&ring, &p);
                    } else {
                        draw_shape(canvas, node, rect, node.stroke_align.inset(width), &stroke);
                    }
                }
                draw_inner_shadows(canvas, node, rect, opacity);
            }
            if layer_blur.is_some() {
                canvas.restore();
            }
            canvas.restore();
        }
        self.flush(canvas);
        stats
    }

    /// Background blur and drop shadows, drawn before the layer itself.
    fn draw_behind(&mut self, canvas: &Canvas, node: &Node, rect: Rect, opacity: f32) {
        for e in &node.effects {
            match *e {
                Effect::BackgroundBlur { radius, visible: true } if radius > 0.0 => {
                    canvas.save();
                    canvas.clip_path(&shape_path(node, rect, 0.0), ClipOp::Intersect, true);
                    if let Some(filter) = image_filters::blur((radius / 2.0, radius / 2.0), skia_safe::TileMode::Clamp, None, None) {
                        canvas.save_layer(&SaveLayerRec::default().backdrop(&filter));
                        canvas.restore();
                    }
                    canvas.restore();
                }
                Effect::DropShadow { color, x, y, blur, spread, visible: true } => {
                    let argb = with_alpha(color.argb(), opacity);
                    if node.kind == Kind::Text {
                        // Text shadows: the glyphs again, offset and blurred, in the shadow colour.
                        let mut p = Paint::default();
                        if blur > 0.0 {
                            p.set_image_filter(image_filters::blur((blur / 2.0, blur / 2.0), None, None, None));
                        }
                        canvas.save_layer(&SaveLayerRec::default().paint(&p));
                        canvas.translate((x, y));
                        self.text.paint(canvas, node, argb);
                        canvas.restore();
                        continue;
                    }
                    let mut p = Paint::default();
                    p.set_anti_alias(true);
                    p.set_color(Color::new(argb));
                    if blur > 0.0 {
                        p.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, blur / 2.0, false));
                    }
                    canvas.draw_path(&shape_path(node, rect.with_offset((x, y)), -spread), &p);
                }
                _ => {}
            }
        }
    }

    /// An image fill clipped to the layer's shape and placed by its scale mode.
    #[allow(clippy::too_many_arguments)]
    fn draw_image(&mut self, canvas: &Canvas, scene: &Scene, node: &Node, rect: Rect, img: &ImageFill, opacity: f32, blend: BlendMode) {
        let image = self
            .images
            .entry(img.hash.clone())
            .or_insert_with(|| scene.images.get(&img.hash).and_then(|b| Image::from_encoded(skia_safe::Data::new_copy(b))))
            .clone();
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_alpha_f(opacity * img.opacity);
        p.set_blend_mode(blend);
        if let Some(m) = img.adjust.matrix() {
            p.set_color_filter(skia_safe::color_filters::matrix_row_major(&m, None));
        }
        let Some(image) = image else {
            // Missing or undecodable: a light placeholder, as Figma shows for broken images.
            p.set_color(Color::from_argb((opacity * 255.0) as u8, 0xd9, 0xd9, 0xd9));
            canvas.draw_path(&shape_path(node, rect, 0.0), &p);
            return;
        };
        let (iw, ih) = (image.width() as f32, image.height() as f32);
        canvas.save();
        canvas.clip_path(&shape_path(node, rect, 0.0), ClipOp::Intersect, true);
        let sampling = SamplingOptions::new(skia_safe::FilterMode::Linear, skia_safe::MipmapMode::Linear);
        match img.mode {
            ImageMode::Tile => {
                let s = img.scale.max(0.01);
                let m = Matrix::new_all(s, 0.0, rect.left, 0.0, s, rect.top, 0.0, 0.0, 1.0);
                if let Some(shader) = image.to_shader((TileMode::Repeat, TileMode::Repeat), sampling, &m) {
                    p.set_shader(shader);
                    canvas.draw_rect(rect, &p);
                }
            }
            mode => {
                let dst = match mode {
                    ImageMode::Crop => rect,
                    _ => {
                        let scale = if mode == ImageMode::Fill {
                            (rect.width() / iw).max(rect.height() / ih)
                        } else {
                            (rect.width() / iw).min(rect.height() / ih)
                        };
                        let (w, h) = (iw * scale, ih * scale);
                        Rect::from_xywh(rect.center_x() - w / 2.0, rect.center_y() - h / 2.0, w, h)
                    }
                };
                canvas.draw_image_rect_with_sampling_options(&image, None, dst, sampling, &p);
            }
        }
        canvas.restore();
    }

    fn flush(&mut self, canvas: &Canvas) {
        if !self.batch.is_empty() {
            canvas.draw_vertices(&mesh(&self.batch), BlendMode::Dst, &Paint::default());
            self.batch.clear();
        }
    }
}

fn apply_clip(canvas: &Canvas, c: &ClipShape) {
    let (cx, cy) = c.rect.center();
    let pivot = Some(Point::new(cx, cy));
    if c.rotation != 0.0 {
        canvas.rotate(-c.rotation, pivot);
    }
    let r = to_sk(&c.rect);
    if c.radii == [0.0; 4] {
        canvas.clip_rect(r, ClipOp::Intersect, true);
    } else {
        canvas.clip_rrect(rounded(r, c.radii), ClipOp::Intersect, true);
    }
    if c.rotation != 0.0 {
        canvas.rotate(c.rotation, pivot);
    }
}

fn to_sk(r: &pergel_core::Rect) -> Rect {
    Rect::from_xywh(r.x, r.y, r.w, r.h)
}

fn rounded(r: Rect, radii: [f32; 4]) -> RRect {
    let v = radii.map(|x| Vector::new(x.max(0.0), x.max(0.0)));
    RRect::new_rect_radii(r, &v)
}

/// Individual strokes (top, right, bottom, left) as a filled ring: the area between the outer
/// edge and the inner edge, placed by the stroke alignment.
pub fn sides_ring(node: &Node, rect: Rect, [t, r, b, l]: [f32; 4]) -> Option<Path> {
    let k = match node.stroke_align {
        pergel_core::StrokeAlign::Inside => 0.0,
        pergel_core::StrokeAlign::Center => 0.5,
        pergel_core::StrokeAlign::Outside => 1.0,
    };
    let grow = |r0: Rect, f: f32| Rect::new(r0.left - l * f, r0.top - t * f, r0.right + r * f, r0.bottom + b * f);
    let outer_r = grow(rect, k);
    let inner_r = grow(rect, k - 1.0);
    let radii = node.radii();
    let shape = |r0: Rect, d: f32| {
        let mut pb = PathBuilder::new();
        if radii == [0.0; 4] {
            pb.add_rect(r0, None, None);
        } else {
            pb.add_rrect(rounded(r0, radii.map(|x| (x + d).max(0.0))), None, None);
        }
        pb.detach()
    };
    let avg = (t + r + b + l) / 4.0;
    let outer = shape(outer_r, avg * k);
    if inner_r.width() <= 0.0 || inner_r.height() <= 0.0 {
        return Some(outer);
    }
    let inner = shape(inner_r, avg * (k - 1.0));
    outer.op(&inner, skia_safe::PathOp::Difference)
}

/// The node's outline as a path, `inset` > 0 shrinking it and < 0 growing it.
fn shape_path(node: &Node, rect: Rect, inset: f32) -> Path {
    let r = Rect::new(rect.left + inset, rect.top + inset, (rect.right - inset).max(rect.left + inset), (rect.bottom - inset).max(rect.top + inset));
    if node.kind == Kind::Vector {
        return vector_path(node, rect);
    }
    let mut b = PathBuilder::new();
    match node.kind {
        Kind::Ellipse => {
            b.add_oval(r, None, None);
        }
        _ => {
            let radii = node.radii().map(|x| (x - inset).max(0.0));
            if radii == [0.0; 4] {
                b.add_rect(r, None, None);
            } else {
                b.add_rrect(rounded(r, radii), None, None);
            }
        }
    }
    b.detach()
}

/// A layer's outline in world coordinates (its rotation applied), for boolean operations.
pub fn world_path(node: &Node) -> Path {
    let p = shape_path(node, to_sk(&node.bounds), 0.0);
    rotated(node, p)
}

fn rotated(node: &Node, p: Path) -> Path {
    if node.rotation == 0.0 {
        return p;
    }
    let (cx, cy) = node.bounds.center();
    p.make_transform(&Matrix::rotate_deg_pivot(-node.rotation, Point::new(cx, cy)))
}

/// The area a layer's stroke covers, as a fillable outline in world coordinates ("Outline stroke").
pub fn stroke_outline(node: &Node) -> Option<Path> {
    let (_, width) = node.stroke.filter(|s| s.1 > 0.0)?;
    let shape = shape_path(node, to_sk(&node.bounds), 0.0);
    let mut paint = Paint::default();
    paint.set_style(PaintStyle::Stroke);
    if let Some(v) = node.vector.as_deref() {
        paint.set_stroke_cap(match v.cap {
            pergel_core::Cap::Butt => PaintCap::Butt,
            pergel_core::Cap::Round => PaintCap::Round,
            pergel_core::Cap::Square => PaintCap::Square,
        });
        paint.set_stroke_join(match v.join {
            pergel_core::Join::Miter => PaintJoin::Miter,
            pergel_core::Join::Round => PaintJoin::Round,
            pergel_core::Join::Bevel => PaintJoin::Bevel,
        });
    }
    let align = if node.kind == Kind::Vector { pergel_core::StrokeAlign::Center } else { node.stroke_align };
    paint.set_stroke_width(if align == pergel_core::StrokeAlign::Center { width } else { width * 2.0 });
    let mut b = PathBuilder::new();
    if !skia_safe::path_utils::fill_path_with_paint(&shape, &paint, &mut b, None, None) {
        return None;
    }
    let mut ring = b.detach();
    if let Some(v) = node.vector.as_deref() {
        let r = to_sk(&node.bounds);
        for (d, filled) in v.markers(r.left, r.top, r.width(), r.height(), width) {
            let Some(path) = Path::from_svg(&d) else { continue };
            let area = if filled {
                Some(path)
            } else {
                let mut mb = PathBuilder::new();
                skia_safe::path_utils::fill_path_with_paint(&path, &paint, &mut mb, None, None).then(|| mb.detach())
            };
            if let Some(u) = area.and_then(|a| ring.op(&a, skia_safe::PathOp::Union)) {
                ring = u;
            }
        }
    }
    let out = match align {
        pergel_core::StrokeAlign::Inside => ring.op(&shape, skia_safe::PathOp::Intersect)?,
        pergel_core::StrokeAlign::Outside => ring.op(&shape, skia_safe::PathOp::Difference)?,
        pergel_core::StrokeAlign::Center => ring,
    };
    Some(rotated(node, out))
}

thread_local! {
    /// Parsed vector outlines by path data; vectors are drawn every frame.
    static PATHS: RefCell<HashMap<String, Path>> = RefCell::new(HashMap::new());
}

/// A vector layer's outline stretched into `rect`.
fn vector_path(node: &Node, rect: Rect) -> Path {
    let Some(v) = node.vector.as_deref() else { return Path::new() };
    let path = PATHS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.len() > 8192 {
            cache.clear();
        }
        cache.entry(v.d.clone()).or_insert_with(|| Path::from_svg(&v.d).unwrap_or_default()).clone()
    });
    let (sx, sy) = v.scale_to(rect.width(), rect.height());
    let mut path = path.make_transform(&Matrix::new_all(sx, 0.0, rect.left, 0.0, sy, rect.top, 0.0, 0.0, 1.0));
    if v.even_odd {
        path.set_fill_type(PathFillType::EvenOdd);
    }
    path
}

/// A gradient fill's shader; handles are in the layer's unit square.
fn gradient_shader(g: &pergel_core::Gradient, rect: Rect) -> Option<Shader> {
    let stops = g.sorted_stops();
    if stops.is_empty() {
        return None;
    }
    let colors: Vec<skia_safe::Color4f> = stops.iter().map(|s| skia_safe::Color4f::from(Color::new(s.color.argb()))).collect();
    let pos: Vec<f32> = stops.iter().map(|s| s.at.clamp(0.0, 1.0)).collect();
    let grad = gradient::Gradient::new(gradient::Colors::new(&colors, Some(&pos), TileMode::Clamp, None), gradient::Interpolation::default());
    let unit = Matrix::new_all(rect.width(), 0.0, rect.left, 0.0, rect.height(), rect.top, 0.0, 0.0, 1.0);
    let (fx, fy) = g.from;
    let (ax, ay) = (g.to.0 - fx, g.to.1 - fy);
    match g.kind {
        GradientKind::Linear => {
            gradient::shaders::linear_gradient(((fx, fy), g.to), &grad, &unit)
        }
        kind => {
            // The unit circle mapped onto the gradient's ellipse: first axis to `to`, second
            // perpendicular to it, scaled by `width`.
            let (bx, by) = (-ay * g.width, ax * g.width);
            let ellipse = Matrix::new_all(ax, bx, fx, ay, by, fy, 0.0, 0.0, 1.0);
            let m = Matrix::concat(&unit, &ellipse);
            if kind == GradientKind::Angular {
                gradient::shaders::sweep_gradient((0.0, 0.0), (0.0, 360.0), &grad, &m)
            } else {
                gradient::shaders::radial_gradient(((0.0, 0.0), 1.0), &grad, &m)
            }
        }
    }
}

/// Inner shadows: a blurred ring around the shape's edge, kept inside the shape.
fn draw_inner_shadows(canvas: &Canvas, node: &Node, rect: Rect, opacity: f32) {
    for e in &node.effects {
        let Effect::InnerShadow { color, x, y, blur, spread, visible: true } = *e else { continue };
        canvas.save();
        let shape = shape_path(node, rect, 0.0);
        canvas.clip_path(&shape, ClipOp::Intersect, true);
        let pad = blur * 3.0 + x.abs().max(y.abs()) + spread.abs() + 4.0;
        let mut b = PathBuilder::new();
        b.add_rect(rect.with_outset((pad, pad)), None, None);
        b.add_path(&shape_path(node, rect.with_offset((x, y)), spread), None);
        b.set_fill_type(PathFillType::EvenOdd);
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_color(Color::new(with_alpha(color.argb(), opacity)));
        if blur > 0.0 {
            p.set_mask_filter(MaskFilter::blur(BlurStyle::Normal, blur / 2.0, false));
        }
        canvas.draw_path(&b.detach(), &p);
        canvas.restore();
    }
}

/// Draws the node's shape in its own (unrotated) frame; `inset` > 0 shrinks it, < 0 grows it.
fn draw_shape(canvas: &Canvas, node: &Node, rect: Rect, inset: f32, paint: &Paint) {
    let r = Rect::new(
        rect.left + inset,
        rect.top + inset,
        (rect.right - inset).max(rect.left + inset),
        (rect.bottom - inset).max(rect.top + inset),
    );
    match node.kind {
        Kind::Vector => {
            canvas.draw_path(&vector_path(node, rect), paint);
        }
        Kind::Ellipse => {
            canvas.draw_oval(r, paint);
        }
        _ => {
            let radii = node.radii();
            if radii == [0.0; 4] {
                canvas.draw_rect(r, paint);
            } else {
                canvas.draw_rrect(rounded(r, radii.map(|x| (x - inset).max(0.0))), paint);
            }
        }
    }
}

/// Maps a world point to device pixels.
fn device(view: &Viewport, dpr: f32, x: f32, y: f32) -> Point {
    let (sx, sy) = view.to_screen(x, y);
    Point::new(sx * dpr, sy * dpr)
}

fn crisp(r: Rect, dpr: f32) -> Rect {
    let half = 0.5 * dpr;
    Rect::new(r.left.round() + half, r.top.round() + half, r.right.round() - half, r.bottom.round() - half)
}

/// Outlines a (possibly rotated) box or ellipse given in world coordinates.
fn outline(canvas: &Canvas, view: &Viewport, dpr: f32, rect: &pergel_core::Rect, rotation: f32, oval: bool, paint: &Paint) {
    let tl = device(view, dpr, rect.x, rect.y);
    let r = Rect::from_xywh(tl.x, tl.y, rect.w * view.zoom * dpr, rect.h * view.zoom * dpr);
    if rotation == 0.0 {
        let r = if oval { r } else { crisp(r, dpr) };
        if oval {
            canvas.draw_oval(r, paint);
        } else {
            canvas.draw_rect(r, paint);
        }
        return;
    }
    canvas.save();
    canvas.rotate(-rotation, Some(r.center()));
    if oval {
        canvas.draw_oval(r, paint);
    } else {
        canvas.draw_rect(r, paint);
    }
    canvas.restore();
}

fn draw_overlay(canvas: &Canvas, scene: &Scene, view: &Viewport, dpr: f32, overlay: &Overlay) {
    let mut line = Paint::default();
    line.set_anti_alias(true);
    line.set_style(PaintStyle::Stroke);
    line.set_color(SELECTION);
    line.set_stroke_width(dpr);

    if let Some(node) = overlay.hover.filter(|id| !overlay.selection.contains(id)).and_then(|id| scene.node(id)) {
        line.set_color(if scene.in_component(node.id) { COMPONENT } else { SELECTION });
        line.set_stroke_width(if node.kind == Kind::Frame { dpr } else { 2.0 * dpr });
        let (rect, rotation) = if node.kind == Kind::Group { (node.aabb(), 0.0) } else { (node.bounds, node.rotation) };
        outline(canvas, view, dpr, &rect, rotation, node.kind == Kind::Ellipse, &line);
        line.set_stroke_width(dpr);
    }

    if let Some(m) = overlay.marquee {
        let r = Rect::from_xywh(m.x * dpr, m.y * dpr, m.w * dpr, m.h * dpr);
        let mut fill = Paint::default();
        fill.set_color(Color::from_argb(26, 13, 153, 255));
        canvas.draw_rect(r, &fill);
        canvas.draw_rect(crisp(r, dpr), &line);
    }

    if !overlay.page_guides.is_empty() {
        let mut p = Paint::default();
        p.set_color(Color::from_argb(255, 242, 72, 34));
        p.set_stroke_width(dpr);
        let (w, h) = (view.width * dpr, view.height * dpr);
        for (vertical, at) in overlay.page_guides {
            if *vertical {
                let x = device(view, dpr, *at, 0.0).x.round() + 0.5;
                canvas.draw_line((x, 0.0), (x, h), &p);
            } else {
                let y = device(view, dpr, 0.0, *at).y.round() + 0.5;
                canvas.draw_line((0.0, y), (w, y), &p);
            }
        }
    }
    if !overlay.slices.is_empty() {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_style(PaintStyle::Stroke);
        p.set_stroke_width(dpr);
        p.set_color(Color::from_argb(160, 140, 140, 140));
        p.set_path_effect(skia_safe::PathEffect::dash(&[4.0 * dpr, 3.0 * dpr], 0.0));
        for r in overlay.slices {
            let a = device(view, dpr, r.x, r.y);
            let b = device(view, dpr, r.right(), r.bottom());
            canvas.draw_rect(Rect::new(a.x.round() + 0.5, a.y.round() + 0.5, b.x.round() - 0.5, b.y.round() - 0.5), &p);
        }
    }
    for (x, y) in overlay.notes {
        // A note pin: an orange dot with a white ring at the layer's top-right corner.
        let p = device(view, dpr, *x, *y);
        let mut ring = Paint::default();
        ring.set_anti_alias(true);
        ring.set_color(Color::WHITE);
        canvas.draw_circle(p, 7.0 * dpr, &ring);
        ring.set_color(Color::from_argb(255, 255, 140, 0));
        canvas.draw_circle(p, 5.0 * dpr, &ring);
    }
    if overlay.layout_grids {
        draw_layout_grids(canvas, scene, view, dpr);
    }
    draw_connections(canvas, view, dpr, overlay.connections);
    if !overlay.guides.is_empty() {
        let mut red = Paint::default();
        red.set_anti_alias(true);
        red.set_style(PaintStyle::Stroke);
        red.set_color(Color::from_argb(255, 242, 72, 34));
        red.set_stroke_width(dpr);
        for (a, b) in overlay.guides {
            let (a, b) = (device(view, dpr, a.0, a.1), device(view, dpr, b.0, b.1));
            canvas.draw_line(a, b, &red);
            for p in [a, b] {
                // Small crosses at the snapped ends, as Figma draws them.
                let s = 3.0 * dpr;
                canvas.draw_line((p.x - s, p.y - s), (p.x + s, p.y + s), &red);
                canvas.draw_line((p.x - s, p.y + s), (p.x + s, p.y - s), &red);
            }
        }
    }
    if let Some(v) = &overlay.vector {
        draw_vector_edit(canvas, view, dpr, v);
        return;
    }
    if overlay.hide_selection {
        return;
    }
    let selected: Vec<&Node> = overlay.selection.iter().filter_map(|id| scene.node(*id)).collect();
    line.set_color(if selected.iter().any(|n| scene.in_component(n.id)) { COMPONENT } else { SELECTION });
    if selected.len() > 1 {
        for node in &selected {
            let (rect, rotation) = if node.kind == Kind::Group { (node.aabb(), 0.0) } else { (node.bounds, node.rotation) };
            outline(canvas, view, dpr, &rect, rotation, false, &line);
        }
    }
    let Some((rect, rotation)) = scene.selection_box(overlay.selection) else { return };
    let (rect, rotation) = match selected.as_slice() {
        [one] if one.kind == Kind::Group => (one.aabb(), 0.0),
        _ => (rect, rotation),
    };
    outline(canvas, view, dpr, &rect, rotation, false, &line);

    let mut handle_fill = Paint::default();
    handle_fill.set_color(Color::WHITE);
    let size = HANDLE_PX * dpr;
    for (x, y) in rect.corners(rotation) {
        let p = device(view, dpr, x, y);
        let h = Rect::from_xywh((p.x - size / 2.0).round(), (p.y - size / 2.0).round(), size, size);
        canvas.save();
        if rotation != 0.0 {
            canvas.rotate(-rotation, Some(p));
        }
        canvas.draw_rect(h, &handle_fill);
        canvas.draw_rect(if rotation == 0.0 { crisp(h, dpr) } else { h }, &line);
        canvas.restore();
    }
}

/// Frames' column, row and square-grid guides, tinted as in Figma.
fn draw_layout_grids(canvas: &Canvas, scene: &Scene, view: &Viewport, dpr: f32) {
    let area = view.world_area();
    for n in scene.visible(&area) {
        if n.grids.is_empty() || n.kind != Kind::Frame || n.rotation != 0.0 || scene.reach(n.id).is_none() {
            continue;
        }
        let b = n.bounds;
        let tl = device(view, dpr, b.x, b.y);
        let k = view.zoom * dpr;
        canvas.save();
        canvas.clip_rect(Rect::from_xywh(tl.x, tl.y, b.w * k, b.h * k), ClipOp::Intersect, true);
        for g in n.grids.iter().filter(|g| g.visible) {
            let mut p = Paint::default();
            p.set_color(Color::new(g.color.argb()));
            match g.kind {
                pergel_core::GridKind::Columns => {
                    for (s, w) in g.bands(b.w) {
                        canvas.draw_rect(Rect::from_xywh(tl.x + s * k, tl.y, w * k, b.h * k), &p);
                    }
                }
                pergel_core::GridKind::Rows => {
                    for (s, h) in g.bands(b.h) {
                        canvas.draw_rect(Rect::from_xywh(tl.x, tl.y + s * k, b.w * k, h * k), &p);
                    }
                }
                pergel_core::GridKind::Grid => {
                    let step = g.size.max(1.0) * k;
                    if step < 4.0 {
                        continue;
                    }
                    p.set_style(PaintStyle::Stroke);
                    p.set_stroke_width(dpr);
                    let mut x = tl.x;
                    while x <= tl.x + b.w * k {
                        canvas.draw_line((x, tl.y), (x, tl.y + b.h * k), &p);
                        x += step;
                    }
                    let mut y = tl.y;
                    while y <= tl.y + b.h * k {
                        canvas.draw_line((tl.x, y), (tl.x + b.w * k, y), &p);
                        y += step;
                    }
                }
            }
        }
        canvas.restore();
    }
}

/// Prototype noodles: a curve from each hotspot to its destination with an arrowhead.
fn draw_connections(canvas: &Canvas, view: &Viewport, dpr: f32, links: &[((f32, f32), (f32, f32))]) {
    if links.is_empty() {
        return;
    }
    let mut line = Paint::default();
    line.set_anti_alias(true);
    line.set_style(PaintStyle::Stroke);
    line.set_color(SELECTION);
    line.set_stroke_width(2.0 * dpr);
    let mut dot = Paint::default();
    dot.set_anti_alias(true);
    dot.set_color(SELECTION);
    for (a, b) in links {
        let (a, b) = (device(view, dpr, a.0, a.1), device(view, dpr, b.0, b.1));
        let bend = ((b.x - a.x).abs() * 0.5).max(40.0 * dpr);
        let mut p = PathBuilder::new();
        p.move_to(a);
        p.cubic_to((a.x + bend, a.y), (b.x - bend, b.y), b);
        canvas.draw_path(&p.detach(), &line);
        canvas.draw_circle(a, 4.0 * dpr, &dot);
        let s = 7.0 * dpr;
        let mut head = PathBuilder::new();
        head.move_to(b);
        head.line_to((b.x - s, b.y - s * 0.6));
        head.line_to((b.x - s, b.y + s * 0.6));
        head.close();
        canvas.draw_path(&head.detach(), &dot);
    }
}

/// Vector edit mode: the outline, its anchors (selected ones filled) and their handles.
fn draw_vector_edit(canvas: &Canvas, view: &Viewport, dpr: f32, v: &VectorOverlay) {
    let mut line = Paint::default();
    line.set_anti_alias(true);
    line.set_style(PaintStyle::Stroke);
    line.set_color(SELECTION);
    line.set_stroke_width(dpr);
    let mut paths = v.paths.to_vec();
    pergel_core::map_points(&mut paths, |(x, y)| {
        let p = device(view, dpr, x, y);
        (p.x, p.y)
    });
    if let Some(path) = Path::from_svg(pergel_core::to_path(&paths)) {
        canvas.draw_path(&path, &line);
    }
    if let Some((a, b)) = v.rubber {
        let (a, b) = (device(view, dpr, a.0, a.1), device(view, dpr, b.0, b.1));
        canvas.draw_line(a, b, &line);
    }
    let mut white = Paint::default();
    white.set_anti_alias(true);
    white.set_color(Color::WHITE);
    let mut blue = white.clone();
    blue.set_color(SELECTION);
    let size = 7.0 * dpr;
    let handle_r = 3.5 * dpr;
    for (pi, p) in paths.iter().enumerate() {
        for (ai, a) in p.anchors.iter().enumerate() {
            let selected = v.selected.contains(&(pi, ai));
            if selected {
                for h in [a.cin, a.cout].into_iter().flatten() {
                    canvas.draw_line(Point::new(a.p.0, a.p.1), Point::new(h.0, h.1), &line);
                    canvas.draw_circle(Point::new(h.0, h.1), handle_r, &white);
                    canvas.draw_circle(Point::new(h.0, h.1), handle_r, &line);
                }
            }
            let r = Rect::from_xywh(a.p.0 - size / 2.0, a.p.1 - size / 2.0, size, size);
            canvas.draw_rect(r, if selected { &blue } else { &white });
            canvas.draw_rect(r, &line);
        }
    }
}

/// Skia cannot build an empty vertex buffer, so callers must pass at least one quad.
fn mesh(quads: &[(Rect, u32)]) -> Vertices {
    debug_assert!(!quads.is_empty());
    let mut builder = Builder::new(VertexMode::Triangles, quads.len() * 6, 0, BuilderFlags::HAS_COLORS);
    for (i, (r, _)) in quads.iter().enumerate() {
        let (tl, tr, bl, br) = (
            Point::new(r.left, r.top),
            Point::new(r.right, r.top),
            Point::new(r.left, r.bottom),
            Point::new(r.right, r.bottom),
        );
        builder.positions()[i * 6..i * 6 + 6].copy_from_slice(&[tl, tr, bl, tr, br, bl]);
    }
    if let Some(colors) = builder.colors() {
        for (i, (_, argb)) in quads.iter().enumerate() {
            colors[i * 6..i * 6 + 6].fill(Color::new(*argb));
        }
    }
    builder.detach()
}
