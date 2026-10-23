use serde::{Deserialize, Serialize};

use crate::geom::Color;

/// Figma's layer blend modes. `PassThrough` is the default for groups and frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Blend {
    PassThrough,
    #[default]
    Normal,
    Darken,
    Multiply,
    ColorBurn,
    Lighten,
    Screen,
    ColorDodge,
    Overlay,
    SoftLight,
    HardLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StrokeAlign {
    #[default]
    Inside,
    Center,
    Outside,
}

impl StrokeAlign {
    /// How far the stroke's centre line sits inside the shape's edge, for a stroke of `width`.
    pub fn inset(self, width: f32) -> f32 {
        match self {
            StrokeAlign::Inside => width / 2.0,
            StrokeAlign::Center => 0.0,
            StrokeAlign::Outside => -width / 2.0,
        }
    }
}

/// Figma's layer effects. Shadows draw from the layer's shape; blurs act on the layer itself or
/// on what is behind it.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Effect {
    DropShadow { color: Color, x: f32, y: f32, blur: f32, spread: f32, visible: bool },
    InnerShadow { color: Color, x: f32, y: f32, blur: f32, spread: f32, visible: bool },
    LayerBlur { radius: f32, visible: bool },
    BackgroundBlur { radius: f32, visible: bool },
}

impl Effect {
    pub fn visible(&self) -> bool {
        match *self {
            Effect::DropShadow { visible, .. }
            | Effect::InnerShadow { visible, .. }
            | Effect::LayerBlur { visible, .. }
            | Effect::BackgroundBlur { visible, .. } => visible,
        }
    }

    /// Figma's defaults for a new effect of this kind.
    pub fn default_of(kind: &str) -> Effect {
        let shadow = Color::rgba(0, 0, 0, 64);
        match kind {
            "inner-shadow" => Effect::InnerShadow { color: shadow, x: 0.0, y: 4.0, blur: 4.0, spread: 0.0, visible: true },
            "layer-blur" => Effect::LayerBlur { radius: 4.0, visible: true },
            "background-blur" => Effect::BackgroundBlur { radius: 4.0, visible: true },
            _ => Effect::DropShadow { color: shadow, x: 0.0, y: 4.0, blur: 4.0, spread: 0.0, visible: true },
        }
    }

    /// How far the effect paints outside the layer's box (for culling and thumbnails).
    pub fn outset(&self) -> f32 {
        match *self {
            Effect::DropShadow { x, y, blur, spread, visible: true, .. } => blur + spread.max(0.0) + x.abs().max(y.abs()),
            Effect::LayerBlur { radius, visible: true } => radius,
            _ => 0.0,
        }
    }
}

/// Figma's layout guides on frames: columns, rows or a square grid.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GridKind {
    #[default]
    Columns,
    Rows,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GridAlign {
    #[default]
    Stretch,
    Start,
    Center,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutGrid {
    pub kind: GridKind,
    /// Columns or rows (ignored for a square grid).
    pub count: u16,
    pub gutter: f32,
    pub margin: f32,
    /// Square grid cell, or column/row width when not stretched.
    pub size: f32,
    pub align: GridAlign,
    pub color: Color,
    pub visible: bool,
}

impl Default for LayoutGrid {
    fn default() -> Self {
        Self {
            kind: GridKind::Columns,
            count: 12,
            gutter: 20.0,
            margin: 0.0,
            size: 10.0,
            align: GridAlign::Stretch,
            color: Color::rgba(255, 0, 0, 26),
            visible: true,
        }
    }
}

impl LayoutGrid {
    /// The column (or row) bands inside a frame span of `len`: (start, size) along the axis.
    pub fn bands(&self, len: f32) -> Vec<(f32, f32)> {
        let n = self.count.max(1) as usize;
        let total_gutter = self.gutter * (n as f32 - 1.0);
        let (start, width) = match self.align {
            GridAlign::Stretch => (self.margin, ((len - 2.0 * self.margin - total_gutter) / n as f32).max(0.0)),
            _ => {
                let w = self.size.max(1.0);
                let span = w * n as f32 + total_gutter;
                let s = match self.align {
                    GridAlign::Start => self.margin,
                    GridAlign::End => len - self.margin - span,
                    _ => (len - span) / 2.0,
                };
                (s, w)
            }
        };
        (0..n).map(|i| (start + i as f32 * (width + self.gutter), width)).collect()
    }

    /// "12 columns, gutter 20, margin 80" for agents.
    pub fn summary(&self) -> String {
        match self.kind {
            GridKind::Grid => format!("{} px grid", crate::paint::fmt(self.size)),
            k => format!(
                "{} {}, gutter {}, margin {}{}",
                self.count,
                if k == GridKind::Columns { "columns" } else { "rows" },
                crate::paint::fmt(self.gutter),
                crate::paint::fmt(self.margin),
                if self.align == GridAlign::Stretch { String::new() } else { format!(", {} px wide", crate::paint::fmt(self.size)) }
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    #[default]
    Png,
    Jpg,
    Svg,
    Pdf,
}

/// One row of the Export section: "2x PNG", "512w JPG"…
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ExportSetting {
    /// Multiplier ("2x"), unless `width` or `height` fixes the size.
    pub scale: f32,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub format: ExportFormat,
    /// Added to the layer name in the file name.
    pub suffix: String,
}

impl Default for ExportSetting {
    fn default() -> Self {
        Self { scale: 1.0, width: None, height: None, format: ExportFormat::Png, suffix: String::new() }
    }
}
