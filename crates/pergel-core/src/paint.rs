//! Fills beyond a solid colour (gradients and images) and vector path data.

use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::geom::Color;

/// A layer's fill when it is not a plain colour. The node's `fill` colour stays as a
/// representative colour (used when zoomed far out and for "Selection colors").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Paint {
    Gradient(Gradient),
    Image(ImageFill),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GradientKind {
    #[default]
    Linear,
    Radial,
    Angular,
    Diamond,
}

impl GradientKind {
    pub fn label(self) -> &'static str {
        match self {
            GradientKind::Linear => "Linear",
            GradientKind::Radial => "Radial",
            GradientKind::Angular => "Angular",
            GradientKind::Diamond => "Diamond",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Stop {
    /// Position along the gradient, 0–1.
    pub at: f32,
    pub color: Color,
}

/// Handles are in the layer's unit square (0,0 top-left, 1,1 bottom-right), like Figma's
/// gradient handles. Linear: `from` → `to`. Radial/angular/diamond: centre `from`, `to` ends the
/// first axis and `width` is the second axis' length relative to the first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Gradient {
    pub kind: GradientKind,
    pub stops: Vec<Stop>,
    pub from: (f32, f32),
    pub to: (f32, f32),
    #[serde(default = "one")]
    pub width: f32,
}

fn one() -> f32 {
    1.0
}

impl Gradient {
    pub fn new(kind: GradientKind, a: Color, b: Color) -> Self {
        let (from, to) = match kind {
            GradientKind::Linear => ((0.5, 0.0), (0.5, 1.0)),
            _ => ((0.5, 0.5), (0.5, 1.0)),
        };
        Self { kind, stops: vec![Stop { at: 0.0, color: a }, Stop { at: 1.0, color: b }], from, to, width: 1.0 }
    }

    /// Stops sorted by position (the order Skia and CSS need).
    pub fn sorted_stops(&self) -> Vec<Stop> {
        let mut s = self.stops.clone();
        s.sort_by(|a, b| a.at.total_cmp(&b.at));
        s
    }

    /// The angle of a linear gradient in CSS terms (0deg = towards the top), for a box of `w`×`h`.
    pub fn css_angle(&self, w: f32, h: f32) -> f32 {
        let dx = (self.to.0 - self.from.0) * w;
        let dy = (self.to.1 - self.from.1) * h;
        (dx.atan2(-dy).to_degrees().rem_euclid(360.0) * 100.0).round() / 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImageMode {
    /// Cover the layer, cropping the overflow (Figma's default "Fill").
    #[default]
    Fill,
    /// Fit inside the layer, letterboxed.
    Fit,
    /// Stretch to the layer exactly.
    Crop,
    /// Repeat at `scale`.
    Tile,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImageFill {
    /// Key into the document's image store.
    pub hash: String,
    pub mode: ImageMode,
    /// The image's own size in pixels.
    pub width: u32,
    pub height: u32,
    #[serde(default = "one")]
    pub scale: f32,
    #[serde(default = "one")]
    pub opacity: f32,
    /// Exposure, contrast, saturation… (Figma's image adjustments).
    #[serde(default, skip_serializing_if = "ImageAdjust::is_none")]
    pub adjust: ImageAdjust,
    /// Mirrored horizontally, vertically (the layer was flipped).
    #[serde(default, skip_serializing_if = "unflipped")]
    pub flip: [bool; 2],
}

fn unflipped(flip: &[bool; 2]) -> bool {
    !flip[0] && !flip[1]
}

/// An extra fill painted above a layer's own fill (Figma's fill list).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FillLayer {
    pub color: crate::geom::Color,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<Paint>,
    #[serde(default = "visible_default")]
    pub visible: bool,
}

fn visible_default() -> bool {
    true
}

/// Figma's image adjustments, each −1…1; 0 leaves the image as it is.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageAdjust {
    pub exposure: f32,
    pub contrast: f32,
    pub saturation: f32,
    pub temperature: f32,
    pub tint: f32,
    pub highlights: f32,
    pub shadows: f32,
}

type Matrix5 = [[f32; 5]; 5];

fn mul5(a: &Matrix5, b: &Matrix5) -> Matrix5 {
    let mut r = [[0.0; 5]; 5];
    for (i, row) in r.iter_mut().enumerate() {
        for (j, v) in row.iter_mut().enumerate() {
            *v = (0..5).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

/// Per-channel scale and offset (offsets in 0–1).
fn affine(scale: [f32; 3], offset: [f32; 3]) -> Matrix5 {
    let mut m = [[0.0; 5]; 5];
    for i in 0..3 {
        m[i][i] = scale[i];
        m[i][4] = offset[i];
    }
    m[3][3] = 1.0;
    m[4][4] = 1.0;
    m
}

impl ImageAdjust {
    pub fn is_none(&self) -> bool {
        *self == Self::default()
    }

    /// All adjustments as one row-major 4×5 colour matrix (offsets in 0–1), applied in Figma's
    /// order: exposure, temperature and tint, shadows and highlights, contrast, saturation.
    pub fn matrix(&self) -> Option<[f32; 20]> {
        if self.is_none() {
            return None;
        }
        let e = 2f32.powf(self.exposure);
        let (t, g) = (self.temperature * 0.2, self.tint * 0.2);
        let (sh, hi) = (self.shadows * 0.25, self.highlights * 0.25);
        let c = (1.0 + self.contrast).max(0.0);
        let s = (1.0 + self.saturation).max(0.0);
        let (lr, lg, lb) = (0.2126 * (1.0 - s), 0.7152 * (1.0 - s), 0.0722 * (1.0 - s));
        let saturation: Matrix5 = [
            [lr + s, lg, lb, 0.0, 0.0],
            [lr, lg + s, lb, 0.0, 0.0],
            [lr, lg, lb + s, 0.0, 0.0],
            [0.0, 0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 1.0],
        ];
        let steps = [
            affine([e; 3], [0.0; 3]),
            affine([1.0 + t, 1.0 - g * 0.5, 1.0 - t], [0.0, 0.0, 0.0]),
            // Shadows lift (or sink) the dark end, highlights stretch the bright end.
            affine([1.0 - sh + hi; 3], [sh; 3]),
            affine([c; 3], [0.5 * (1.0 - c); 3]),
            saturation,
        ];
        let mut m = affine([1.0; 3], [0.0; 3]);
        for step in &steps {
            m = mul5(step, &m);
        }
        let mut out = [0.0; 20];
        for i in 0..4 {
            out[i * 5..i * 5 + 5].copy_from_slice(&m[i]);
        }
        Some(out)
    }

    /// A CSS `filter` close to the adjustments, for generated code.
    pub fn css_filter(&self) -> Option<String> {
        if self.is_none() {
            return None;
        }
        let r = |v: f32| (v * 100.0).round() / 100.0;
        let mut parts = Vec::new();
        let bright = 2f32.powf(self.exposure) * (1.0 + (self.highlights + self.shadows) * 0.12);
        if (bright - 1.0).abs() > 0.005 {
            parts.push(format!("brightness({})", r(bright)));
        }
        if self.contrast != 0.0 {
            parts.push(format!("contrast({})", r((1.0 + self.contrast).max(0.0))));
        }
        if self.saturation != 0.0 {
            parts.push(format!("saturate({})", r((1.0 + self.saturation).max(0.0))));
        }
        if self.temperature > 0.0 {
            parts.push(format!("sepia({})", r(self.temperature * 0.3)));
        } else if self.temperature < 0.0 {
            parts.push(format!("hue-rotate({}deg)", r(self.temperature * -15.0)));
        }
        if self.tint != 0.0 {
            parts.push(format!("hue-rotate({}deg)", r(self.tint * 12.0)));
        }
        (!parts.is_empty()).then(|| parts.join(" "))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Cap {
    #[default]
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Join {
    #[default]
    Miter,
    Round,
    Bevel,
}

/// A vector layer's outline: SVG path data (absolute M/L/Q/C/Z commands) in the layer's own
/// coordinates for a box of `size`; it stretches with the layer when resized.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VectorPath {
    pub d: String,
    pub size: (f32, f32),
    #[serde(default)]
    pub even_odd: bool,
    #[serde(default)]
    pub cap: Cap,
    #[serde(default)]
    pub join: Join,
    /// Start and end points of open paths (Figma's arrowheads).
    #[serde(default, skip_serializing_if = "EndPoint::is_none")]
    pub start: EndPoint,
    #[serde(default, skip_serializing_if = "EndPoint::is_none")]
    pub end: EndPoint,
}

/// Figma's stroke start / end point on an open path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EndPoint {
    #[default]
    None,
    LineArrow,
    TriangleArrow,
    ReversedTriangle,
    Circle,
    Diamond,
}

impl EndPoint {
    pub fn is_none(&self) -> bool {
        *self == EndPoint::None
    }

    pub fn label(self) -> &'static str {
        match self {
            EndPoint::None => "none",
            EndPoint::LineArrow => "line-arrow",
            EndPoint::TriangleArrow => "triangle-arrow",
            EndPoint::ReversedTriangle => "reversed-triangle",
            EndPoint::Circle => "circle",
            EndPoint::Diamond => "diamond",
        }
    }
}

/// One marker at `tip`, pointing along `dir`, for a stroke `width`: path data and whether it is
/// filled with the stroke colour (otherwise stroked like the line).
fn marker(kind: EndPoint, tip: (f32, f32), dir: (f32, f32), width: f32) -> Option<(String, bool)> {
    let len = (dir.0 * dir.0 + dir.1 * dir.1).sqrt();
    if kind == EndPoint::None || len < 1e-4 {
        return None;
    }
    let (ux, uy) = (dir.0 / len, dir.1 / len);
    let (nx, ny) = (-uy, ux);
    let s = (width * 3.0).max(6.0);
    let pt = |along: f32, side: f32| format!("{} {}", fmt(tip.0 + ux * along + nx * side), fmt(tip.1 + uy * along + ny * side));
    Some(match kind {
        EndPoint::None => return None,
        EndPoint::LineArrow => (format!("M{}L{}L{}", pt(-s, s * 0.6), pt(0.0, 0.0), pt(-s, -s * 0.6)), false),
        EndPoint::TriangleArrow => (format!("M{}L{}L{}Z", pt(s * 0.4, 0.0), pt(-s * 0.8, s * 0.6), pt(-s * 0.8, -s * 0.6)), true),
        EndPoint::ReversedTriangle => (format!("M{}L{}L{}Z", pt(s * 0.2, s * 0.6), pt(s * 0.2, -s * 0.6), pt(-s, 0.0)), true),
        EndPoint::Diamond => (format!("M{}L{}L{}L{}Z", pt(s * 0.6, 0.0), pt(0.0, s * 0.6), pt(-s * 0.6, 0.0), pt(0.0, -s * 0.6)), true),
        EndPoint::Circle => {
            let r = s * 0.5;
            let (l, rt) = (format!("{} {}", fmt(tip.0 - r), fmt(tip.1)), format!("{} {}", fmt(tip.0 + r), fmt(tip.1)));
            (format!("M{l}A{} {} 0 1 0 {rt}A{} {} 0 1 0 {l}Z", fmt(r), fmt(r), fmt(r), fmt(r)), true)
        }
    })
}

impl VectorPath {
    /// Scale factors from the path's own space to a box of `w`×`h`.
    pub fn scale_to(&self, w: f32, h: f32) -> (f32, f32) {
        let f = |box_len: f32, own: f32| if own.abs() < 1e-3 { 1.0 } else { box_len / own };
        (f(w, self.size.0), f(h, self.size.1))
    }

    /// Path data mapped into the box at (`x`, `y`) of `w`×`h` (for SVG export and code).
    pub fn placed(&self, x: f32, y: f32, w: f32, h: f32) -> String {
        let (sx, sy) = self.scale_to(w, h);
        map_path(&self.d, |px, py| (x + px * sx, y + py * sy))
    }

    /// The start / end point markers for the path placed in the box `x, y, w, h` with a stroke
    /// `width`: path data, and whether each is filled with the stroke colour or stroked.
    pub fn markers(&self, x: f32, y: f32, w: f32, h: f32, width: f32) -> Vec<(String, bool)> {
        if self.start.is_none() && self.end.is_none() {
            return Vec::new();
        }
        let paths = crate::vector_edit::parse_path(&self.placed(x, y, w, h));
        let open: Vec<_> = paths.iter().filter(|p| !p.closed && p.anchors.len() >= 2).collect();
        let away = |a: (f32, f32), b: Option<(f32, f32)>| b.filter(|b| (b.0 - a.0).abs() + (b.1 - a.1).abs() > 1e-3);
        let mut out = Vec::new();
        if let Some(first) = open.first() {
            let (a, next) = (&first.anchors[0], &first.anchors[1]);
            let toward = away(a.p, a.cout).or(away(a.p, next.cin)).unwrap_or(next.p);
            out.extend(marker(self.start, a.p, (a.p.0 - toward.0, a.p.1 - toward.1), width));
        }
        if let Some(last) = open.last() {
            let n = last.anchors.len();
            let (a, prev) = (&last.anchors[n - 1], &last.anchors[n - 2]);
            let from = away(a.p, a.cin).or(away(a.p, prev.cout)).unwrap_or(prev.p);
            out.extend(marker(self.end, a.p, (a.p.0 - from.0, a.p.1 - from.1), width));
        }
        out
    }

    /// Whether the outline has an open sub-path (start / end points apply).
    pub fn is_open(&self) -> bool {
        crate::vector_edit::parse_path(&self.d).iter().any(|p| !p.closed && p.anchors.len() >= 2)
    }
}

/// Figma's vector shape tools.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ShapeTool {
    /// A straight line; `down` when it runs from the top-left to the bottom-right.
    Line { down: bool },
    Arrow { down: bool },
    /// A regular polygon (Figma's default is a triangle).
    Polygon(u8),
    /// A star with `points` tips and the inner radius as a fraction of the outer one.
    Star { points: u8, ratio: f32 },
}

impl ShapeTool {
    pub fn label(self) -> &'static str {
        match self {
            ShapeTool::Line { .. } => "Line",
            ShapeTool::Arrow { .. } => "Arrow",
            ShapeTool::Polygon(_) => "Polygon",
            ShapeTool::Star { .. } => "Star",
        }
    }

    /// Whether the shape is drawn by its stroke (lines) rather than its fill.
    pub fn is_line(self) -> bool {
        matches!(self, ShapeTool::Line { .. } | ShapeTool::Arrow { .. })
    }

    /// The outline for a `w`×`h` box.
    pub fn path(self, w: f32, h: f32) -> VectorPath {
        let p = |x: f32, y: f32| format!("{} {}", fmt(x), fmt(y));
        let down = match self {
            ShapeTool::Line { down } | ShapeTool::Arrow { down } => down,
            _ => true,
        };
        let (a, b) = if down { ((0.0, 0.0), (w, h)) } else { ((0.0, h), (w, 0.0)) };
        let d = match self {
            ShapeTool::Line { .. } => format!("M{}L{}", p(a.0, a.1), p(b.0, b.1)),
            // An arrow is a line whose end point is a line arrowhead, as in Figma.
            ShapeTool::Arrow { .. } => format!("M{}L{}", p(a.0, a.1), p(b.0, b.1)),
            ShapeTool::Polygon(n) => {
                let n = n.max(3) as usize;
                let pts: Vec<String> = (0..n)
                    .map(|i| {
                        let t = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                        p(w / 2.0 + w / 2.0 * t.cos(), h / 2.0 + h / 2.0 * t.sin())
                    })
                    .collect();
                format!("M{}Z", pts.join("L"))
            }
            ShapeTool::Star { points, ratio } => {
                let n = points.max(3) as usize * 2;
                let pts: Vec<String> = (0..n)
                    .map(|i| {
                        let r = if i % 2 == 0 { 1.0 } else { ratio };
                        let t = std::f32::consts::TAU * i as f32 / n as f32 - std::f32::consts::FRAC_PI_2;
                        p(w / 2.0 + w / 2.0 * r * t.cos(), h / 2.0 + h / 2.0 * r * t.sin())
                    })
                    .collect();
                format!("M{}Z", pts.join("L"))
            }
        };
        let line = self.is_line();
        let end = if matches!(self, ShapeTool::Arrow { .. }) { EndPoint::LineArrow } else { EndPoint::None };
        VectorPath { d, size: (w, h), even_odd: false, cap: if line { Cap::Round } else { Cap::Butt }, join: if line { Join::Round } else { Join::Miter }, start: EndPoint::None, end }
    }
}

/// Rewrites every coordinate pair of absolute path data through `f`.
pub fn map_path(d: &str, f: impl Fn(f32, f32) -> (f32, f32)) -> String {
    let mut out = String::with_capacity(d.len());
    let mut nums: Vec<f32> = Vec::new();
    let mut cmd: Option<char> = None;
    let flush = |out: &mut String, cmd: Option<char>, nums: &mut Vec<f32>| {
        let Some(c) = cmd else { return };
        out.push(c);
        for pair in nums.chunks(2) {
            if let [x, y] = pair {
                let (x, y) = f(*x, *y);
                out.push_str(&format!("{} {} ", fmt(x), fmt(y)));
            }
        }
        if out.ends_with(' ') {
            out.pop();
        }
        nums.clear();
    };
    let mut token = String::new();
    for ch in d.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            if !token.is_empty() {
                nums.extend(token.parse::<f32>().ok());
                token.clear();
            }
            flush(&mut out, cmd, &mut nums);
            cmd = Some(ch);
        } else if ch == ',' || ch.is_whitespace() || (ch == '-' && !token.is_empty() && !token.ends_with(['e', 'E'])) {
            if !token.is_empty() {
                nums.extend(token.parse::<f32>().ok());
                token.clear();
            }
            if ch == '-' {
                token.push(ch);
            }
        } else {
            token.push(ch);
        }
    }
    flush(&mut out, cmd, &mut nums);
    out
}

/// Compact number formatting for path data and CSS.
pub fn fmt(v: f32) -> String {
    let r = (v * 100.0).round() / 100.0;
    if r == r.trunc() { format!("{}", r as i64) } else { format!("{r}") }
}

/// Encoded image files (PNG, JPEG, GIF, WebP) keyed by content hash. Shared by every layer
/// that shows the image; never part of undo (entries are immutable).
#[derive(Debug, Clone, Default)]
pub struct Images {
    map: HashMap<String, Arc<Vec<u8>>>,
}

impl Images {
    /// Stores `bytes` and returns their key and pixel size, or `None` if they are not an image.
    pub fn add(&mut self, bytes: Vec<u8>) -> Option<(String, u32, u32)> {
        let size = imagesize::blob_size(&bytes).ok()?;
        let hash = content_hash(&bytes);
        self.map.entry(hash.clone()).or_insert_with(|| Arc::new(bytes));
        Some((hash, size.width as u32, size.height as u32))
    }

    pub fn get(&self, hash: &str) -> Option<&Arc<Vec<u8>>> {
        self.map.get(hash)
    }

    pub fn insert(&mut self, hash: String, bytes: Arc<Vec<u8>>) {
        self.map.insert(hash, bytes);
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<Vec<u8>>)> {
        self.map.iter()
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

/// The file type of encoded image bytes, as an extension.
pub fn image_extension(bytes: &[u8]) -> &'static str {
    match bytes {
        [0x89, b'P', b'N', b'G', ..] => "png",
        [0xff, 0xd8, ..] => "jpg",
        [b'G', b'I', b'F', ..] => "gif",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "webp",
        _ => "bin",
    }
}

/// 128-bit FNV-1a, hex encoded: stable across platforms and good enough to key image content.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in bytes {
        h ^= *b as u128;
        h = h.wrapping_mul(0x0000000001000000000000000000013B);
    }
    format!("{h:032x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_adjustments_compose_into_one_matrix() {
        assert_eq!(ImageAdjust::default().matrix(), None);
        let m = ImageAdjust { exposure: 1.0, ..Default::default() }.matrix().unwrap();
        assert_eq!((m[0], m[6], m[12], m[18]), (2.0, 2.0, 2.0, 1.0), "one stop brighter");
        let grey = ImageAdjust { saturation: -1.0, ..Default::default() }.matrix().unwrap();
        // Every channel becomes the luminance.
        assert!((grey[0] - 0.2126).abs() < 1e-4 && (grey[6] - 0.7152).abs() < 1e-4 && (grey[10] - 0.2126).abs() < 1e-4);
        let f = ImageAdjust { contrast: 0.5, saturation: 0.2, ..Default::default() }.css_filter().unwrap();
        assert_eq!(f, "contrast(1.5) saturate(1.2)");
    }

    #[test]
    fn arrows_are_lines_with_an_arrowhead_at_the_end() {
        let v = ShapeTool::Arrow { down: true }.path(100.0, 0.0);
        assert_eq!((v.d.as_str(), v.start, v.end), ("M0 0L100 0", EndPoint::None, EndPoint::LineArrow));
        let heads = v.markers(0.0, 0.0, 100.0, 0.0, 2.0);
        // A chevron whose tip is the line's end, pointing right.
        assert_eq!(heads, vec![("M94 3.6L100 0L94 -3.6".to_string(), false)]);
        let mut both = v.clone();
        both.start = EndPoint::Circle;
        both.end = EndPoint::TriangleArrow;
        let heads = both.markers(0.0, 0.0, 100.0, 0.0, 2.0);
        assert_eq!(heads.len(), 2);
        assert!(heads.iter().all(|h| h.1), "circle and triangle are filled");
        assert!(heads[1].0.starts_with("M102.4 0"), "the triangle's tip covers the round cap: {}", heads[1].0);
    }

    #[test]
    fn path_data_maps_through_a_box() {
        let v = VectorPath { d: "M0 0L10 0L10 5Z".into(), size: (10.0, 5.0), even_odd: false, cap: Cap::Butt, join: Join::Miter, start: Default::default(), end: Default::default() };
        assert_eq!(v.placed(100.0, 50.0, 20.0, 20.0), "M100 50L120 50L120 70Z");
        assert_eq!(map_path("M1-2C3,4 5,6 7e1,8", |x, y| (x, y)), "M1 -2C3 4 5 6 70 8");
    }

    #[test]
    fn gradient_angle_matches_css() {
        let g = Gradient::new(GradientKind::Linear, Color(0xff000000), Color(0xffffffff));
        assert_eq!(g.css_angle(100.0, 100.0), 180.0);
        let g = Gradient { from: (0.0, 0.5), to: (1.0, 0.5), ..g };
        assert_eq!(g.css_angle(100.0, 100.0), 90.0);
    }
}
