//! Text layout on Skia's paragraph engine. Fonts are registered from bytes (the bundled Inter,
//! later system and Google fonts), so the same code runs in WebAssembly and natively.

use std::collections::HashMap;

use pergel_core::{Decoration, LineHeight, Node, NodeId, Text, TextAlign, TextResize, VerticalAlign};
use skia_safe::font_arguments::VariationPosition;
use skia_safe::font_arguments::variation_position::Coordinate;
use skia_safe::font_style::{Slant, Weight, Width};
use skia_safe::textlayout::{
    FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, TextAlign as SkAlign, TextDecoration, TextStyle,
    TypefaceFontProvider,
};
use skia_safe::{Canvas, Color, Data, FontArguments, FontMgr, FontStyle};

/// Wide enough to never wrap: auto-width text is measured on one line per paragraph.
const UNBOUNDED: f32 = 1.0e6;

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct FontFamily {
    pub name: String,
    /// Lowest and highest weight available (a range for variable fonts).
    pub weights: (u16, u16),
    pub upright: bool,
    pub italic: bool,
}

struct Cached {
    text: Text,
    color: u32,
    width: f32,
    paragraph: Paragraph,
}

pub struct Typesetter {
    loader: FontMgr,
    provider: TypefaceFontProvider,
    collection: FontCollection,
    families: Vec<FontFamily>,
    cache: HashMap<NodeId, Cached>,
}

impl Default for Typesetter {
    fn default() -> Self {
        Self::new()
    }
}

impl Typesetter {
    pub fn new() -> Self {
        let provider = TypefaceFontProvider::new();
        let mut collection = FontCollection::new();
        collection.set_default_font_manager(Some(FontMgr::from(provider.clone())), pergel_core::DEFAULT_FAMILY);
        Self {
            loader: FontMgr::custom_empty().unwrap_or_else(FontMgr::new),
            provider,
            collection,
            families: Vec::new(),
            cache: HashMap::new(),
        }
    }

    pub fn families(&self) -> &[FontFamily] {
        &self.families
    }

    pub fn has_family(&self, name: &str) -> bool {
        self.families.iter().any(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// Registers a font file under `family` (its own family name when `None`). Returns the family.
    pub fn add_font(&mut self, bytes: &[u8], family: Option<&str>) -> Option<String> {
        let typeface = self.loader.new_from_data(Data::new_copy(bytes), None)?;
        let name = family.map(str::to_string).unwrap_or_else(|| typeface.family_name());
        let style = typeface.font_style();
        let weights = typeface
            .variation_design_parameters()
            .and_then(|axes| axes.into_iter().find(|a| a.tag == Coordinate::wght))
            .map(|a| (a.min as u16, a.max as u16))
            .unwrap_or((*style.weight() as u16, *style.weight() as u16));
        let italic = style.slant() != Slant::Upright;
        self.provider.register_typeface(typeface, Some(name.as_str()));
        match self.families.iter_mut().find(|f| f.name == name) {
            Some(f) => {
                f.weights = (f.weights.0.min(weights.0), f.weights.1.max(weights.1));
                f.italic |= italic;
                f.upright |= !italic;
            }
            None => self.families.push(FontFamily { name: name.clone(), weights, upright: !italic, italic }),
        }
        self.families.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        self.collection.clear_caches();
        self.cache.clear();
        Some(name)
    }

    fn build(&self, text: &Text, color: u32) -> Paragraph {
        let mut style = ParagraphStyle::new();
        style.set_text_align(match text.align {
            TextAlign::Left => SkAlign::Left,
            TextAlign::Center => SkAlign::Center,
            TextAlign::Right => SkAlign::Right,
            TextAlign::Justify => SkAlign::Justify,
        });
        let mut ts = TextStyle::new();
        ts.set_font_families(&[text.family.as_str(), pergel_core::DEFAULT_FAMILY]);
        ts.set_font_size(text.size.max(0.1));
        let slant = if text.italic { Slant::Italic } else { Slant::Upright };
        ts.set_font_style(FontStyle::new(Weight::from(text.weight as i32), Width::NORMAL, slant));
        let coordinates = [Coordinate { axis: Coordinate::wght, value: text.weight as f32 }];
        let args = FontArguments::new().set_variation_design_position(VariationPosition { coordinates: &coordinates });
        ts.set_font_arguments(&args);
        ts.set_color(Color::new(color));
        ts.set_letter_spacing(text.letter_spacing_px());
        if text.line_height != LineHeight::Auto {
            ts.set_height(text.line_px(text.size) / text.size.max(0.1));
            ts.set_height_override(true);
            ts.set_half_leading(true);
        }
        ts.set_decoration_type(match text.decoration {
            Decoration::None => TextDecoration::NO_DECORATION,
            Decoration::Underline => TextDecoration::UNDERLINE,
            Decoration::Strikethrough => TextDecoration::LINE_THROUGH,
        });
        for (feature, value) in &text.features {
            ts.add_font_feature(feature, *value as i32);
        }
        style.set_text_style(&ts);
        let mut builder = ParagraphBuilder::new(&style, &self.collection);
        builder.push_style(&ts);
        let display = text.display();
        if (text.runs.is_empty() && text.list == pergel_core::ListStyle::None) || display.is_empty() {
            // An empty layer still has one line, so the caret and the box have a height.
            builder.add_text(if display.is_empty() { " " } else { display.as_str() });
            return builder.build();
        }
        // Mixed styles: each styled range gets its own text style.
        let alpha = color >> 24;
        for (piece, run) in text.list_pieces() {
            match run {
                None => {
                    builder.add_text(&piece);
                }
                Some(r) => {
                    let mut s = ts.clone();
                    let family = r.family.as_deref().unwrap_or(text.family.as_str());
                    s.set_font_families(&[family, pergel_core::DEFAULT_FAMILY]);
                    let size = r.size.unwrap_or(text.size).max(0.1);
                    s.set_font_size(size);
                    let weight = r.weight.unwrap_or(text.weight);
                    let slant = if r.italic.unwrap_or(text.italic) { Slant::Italic } else { Slant::Upright };
                    s.set_font_style(FontStyle::new(Weight::from(weight as i32), Width::NORMAL, slant));
                    let coordinates = [Coordinate { axis: Coordinate::wght, value: weight as f32 }];
                    let args = FontArguments::new().set_variation_design_position(VariationPosition { coordinates: &coordinates });
                    s.set_font_arguments(&args);
                    if let Some(c) = r.color {
                        // The run's colour, faded like the rest of the layer.
                        let a = ((c >> 24) * alpha / 255) << 24;
                        s.set_color(Color::new(a | (c & 0x00ff_ffff)));
                    }
                    if let Some(ls) = r.letter_spacing {
                        s.set_letter_spacing(size * ls / 100.0);
                    }
                    if let Some(d) = r.decoration {
                        s.set_decoration_type(match d {
                            Decoration::None => TextDecoration::NO_DECORATION,
                            Decoration::Underline => TextDecoration::UNDERLINE,
                            Decoration::Strikethrough => TextDecoration::LINE_THROUGH,
                        });
                    } else if r.link.is_some() {
                        s.set_decoration_type(TextDecoration::UNDERLINE);
                    }
                    builder.push_style(&s);
                    builder.add_text(&piece);
                    builder.pop();
                }
            }
        }
        builder.build()
    }

    /// The size the layer should have: `(width, height)` for auto width, the height for auto
    /// height, or `None` for fixed boxes.
    pub fn fit(&self, text: &Text, width: f32) -> Option<(f32, f32)> {
        let mut p = self.build(text, 0);
        match text.resize {
            TextResize::AutoWidth => {
                p.layout(UNBOUNDED);
                let w = if text.content.is_empty() { 0.0 } else { p.max_intrinsic_width() };
                Some((w.ceil().max(1.0), p.height()))
            }
            TextResize::AutoHeight => {
                p.layout(width.max(1.0));
                Some((width, p.height()))
            }
            TextResize::Fixed => None,
        }
    }

    /// Line height in pixels and the laid-out text height inside `width`, for the editor overlay.
    pub fn metrics(&self, text: &Text, width: f32) -> (f32, f32) {
        let mut line = self.build(&Text { content: "A".into(), resize: TextResize::AutoWidth, ..text.clone() }, 0);
        line.layout(UNBOUNDED);
        let mut body = self.build(text, 0);
        body.layout(if text.resize == TextResize::AutoWidth { UNBOUNDED } else { width.max(1.0) });
        (line.height(), body.height())
    }

    /// Paints the node's text in its own frame with `color` (fill and layer opacity applied).
    pub fn paint(&mut self, canvas: &Canvas, node: &Node, color: u32) {
        let Some(text) = node.text.as_deref() else { return };
        let b = node.bounds;
        // Auto-width boxes are exactly as wide as the text; a hair more keeps lines from wrapping.
        let width = if text.resize == TextResize::AutoWidth { b.w + 0.5 } else { b.w.max(1.0) };
        let fresh = self.cache.get(&node.id).is_some_and(|c| c.color == color && c.width == width && c.text == *text);
        if !fresh {
            let mut paragraph = self.build(text, color);
            paragraph.layout(width);
            self.cache.insert(node.id, Cached { text: text.clone(), color, width, paragraph });
        }
        let Some(cached) = self.cache.get(&node.id) else { return };
        let p = &cached.paragraph;
        let dy = match text.vertical {
            VerticalAlign::Top => 0.0,
            VerticalAlign::Middle => (b.h - p.height()) / 2.0,
            VerticalAlign::Bottom => b.h - p.height(),
        };
        if text.content.is_empty() {
            return;
        }
        p.paint(canvas, (b.x, b.y + dy));
    }

    /// Type on a path: the characters one by one along `path` (canvas coordinates), aligned by
    /// the text alignment, sitting on it by their baseline.
    pub fn paint_on_path(&mut self, canvas: &Canvas, node: &Node, color: u32, path: &skia_safe::Path) {
        let Some(text) = node.text.as_deref() else { return };
        let content = text.display().replace('\n', " ");
        if content.is_empty() {
            return;
        }
        let slant = if text.italic { Slant::Italic } else { Slant::Upright };
        let coordinates = [Coordinate { axis: Coordinate::wght, value: text.weight as f32 }];
        let args = FontArguments::new().set_variation_design_position(VariationPosition { coordinates: &coordinates });
        let style = FontStyle::new(Weight::from(text.weight as i32), Width::NORMAL, slant);
        let faces = self.collection.find_typefaces(&[text.family.as_str(), pergel_core::DEFAULT_FAMILY], style);
        let Some(face) = faces.into_iter().next() else { return };
        // Variable fonts: the weight as a design position.
        let face = face.clone_with_arguments(&args).unwrap_or(face);
        let font = skia_safe::Font::new(face, text.size.max(0.1));
        let glyphs = font.text_to_glyphs_vec(content.as_str());
        let mut widths = vec![0.0; glyphs.len()];
        font.get_widths(&glyphs, &mut widths);
        let Some(contour) = skia_safe::ContourMeasureIter::new(path, false, None).next() else { return };
        let len = contour.length();
        let spacing = text.letter_spacing_px();
        let total: f32 = widths.iter().sum::<f32>() + spacing * glyphs.len().saturating_sub(1) as f32;
        let mut at = match text.align {
            TextAlign::Center => (len - total) / 2.0,
            TextAlign::Right => len - total,
            _ => 0.0,
        };
        let (mut shown, mut xforms) = (Vec::new(), Vec::new());
        for (g, w) in glyphs.iter().zip(&widths) {
            let mid = at + w / 2.0;
            if (0.0..=len).contains(&mid)
                && let Some((p, t)) = contour.pos_tan(mid)
            {
                // Each glyph turns with the path's tangent at its centre.
                shown.push(*g);
                xforms.push(skia_safe::RSXform::new(t.x, t.y, (p.x - t.x * w / 2.0, p.y - t.y * w / 2.0)));
            }
            at += w + spacing;
        }
        if shown.is_empty() {
            return;
        }
        let Some(blob) = skia_safe::TextBlob::from_rsxform(&shown[..], &xforms, &font) else { return };
        let mut paint = skia_safe::Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::new(color));
        canvas.draw_text_blob(&blob, (0.0, 0.0), &paint);
    }

    /// Text size for auto layout; NaN while no font is loaded so layers keep their size.
    pub fn measure(&self, text: &Text, width: Option<f32>) -> (f32, f32) {
        if self.families.is_empty() {
            return (f32::NAN, f32::NAN);
        }
        let mut p = self.build(text, 0);
        p.layout(width.unwrap_or(UNBOUNDED).max(1.0));
        let w = match width {
            Some(w) => w,
            None if text.content.is_empty() => 1.0,
            None => p.max_intrinsic_width().ceil().max(1.0),
        };
        (w, p.height())
    }

    /// Drops cached layouts of layers that no longer exist (call occasionally).
    pub fn retain(&mut self, alive: impl Fn(NodeId) -> bool) {
        self.cache.retain(|id, _| alive(*id));
    }
}

impl pergel_core::Measure for Typesetter {
    fn text(&self, text: &Text, width: Option<f32>) -> (f32, f32) {
        self.measure(text, width)
    }
}
