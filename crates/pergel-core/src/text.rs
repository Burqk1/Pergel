//! Text layer properties, mirroring Figma's typography panel. Layout and measuring live in the
//! renderer (Skia's paragraph engine); the core only stores what the user chose.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    #[default]
    Top,
    Middle,
    Bottom,
}

/// Figma's "Resizing": grow sideways, grow downwards within a fixed width, or keep the box.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextResize {
    #[default]
    AutoWidth,
    AutoHeight,
    Fixed,
}

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "unit", content = "value", rename_all = "lowercase")]
pub enum LineHeight {
    /// The font's own line spacing.
    #[default]
    Auto,
    Px(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Decoration {
    #[default]
    None,
    Underline,
    Strikethrough,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TextCase {
    #[default]
    Original,
    Upper,
    Lower,
    Title,
}

pub const DEFAULT_FAMILY: &str = "Inter";

/// A styled range of a text layer ("mixed" styles): character indices `start..end` of the
/// content with the properties that differ from the layer's own.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TextRun {
    pub start: usize,
    pub end: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub italic: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<f32>,
    /// Packed ARGB.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decoration: Option<Decoration>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub letter_spacing: Option<f32>,
    /// A link (Figma's "Create link").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub link: Option<String>,
}

impl TextRun {
    pub fn is_plain(&self) -> bool {
        self.family.is_none()
            && self.weight.is_none()
            && self.italic.is_none()
            && self.size.is_none()
            && self.color.is_none()
            && self.decoration.is_none()
            && self.letter_spacing.is_none()
            && self.link.is_none()
    }

    /// Copies the set properties of `other` onto this run.
    pub fn merge(&mut self, other: &TextRun) {
        if other.family.is_some() {
            self.family = other.family.clone();
        }
        if other.weight.is_some() {
            self.weight = other.weight;
        }
        if other.italic.is_some() {
            self.italic = other.italic;
        }
        if other.size.is_some() {
            self.size = other.size;
        }
        if other.color.is_some() {
            self.color = other.color;
        }
        if other.decoration.is_some() {
            self.decoration = other.decoration;
        }
        if other.letter_spacing.is_some() {
            self.letter_spacing = other.letter_spacing;
        }
        if other.link.is_some() {
            self.link = other.link.clone();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Text {
    pub content: String,
    pub family: String,
    pub weight: u16,
    pub italic: bool,
    pub size: f32,
    pub line_height: LineHeight,
    /// Percent of the font size, as Figma shows it.
    pub letter_spacing: f32,
    pub align: TextAlign,
    pub vertical: VerticalAlign,
    pub resize: TextResize,
    pub decoration: Decoration,
    pub case: TextCase,
    /// The layer is named after its content until the user renames it.
    pub auto_name: bool,
    /// Ranges with their own style, sorted and not overlapping.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub runs: Vec<TextRun>,
    /// Type on a path: the glyphs follow this outline (stretched with the layer's box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<crate::paint::VectorPath>,
    /// Bulleted or numbered lines.
    #[serde(default, skip_serializing_if = "ListStyle::is_none")]
    pub list: ListStyle,
    /// OpenType features: "tnum" → 1 (tabular numbers), "liga" → 0 (no ligatures)…
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub features: std::collections::BTreeMap<String, u32>,
}

/// Figma's list options for a text layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ListStyle {
    #[default]
    None,
    Bullet,
    Numbered,
}

impl ListStyle {
    pub fn is_none(&self) -> bool {
        *self == ListStyle::None
    }
}

impl Default for Text {
    fn default() -> Self {
        Self {
            content: String::new(),
            family: DEFAULT_FAMILY.into(),
            weight: 400,
            italic: false,
            size: 12.0,
            line_height: LineHeight::Auto,
            letter_spacing: 0.0,
            align: TextAlign::Left,
            vertical: VerticalAlign::Top,
            resize: TextResize::AutoWidth,
            decoration: Decoration::None,
            case: TextCase::Original,
            auto_name: true,
            runs: Vec::new(),
            path: None,
            list: ListStyle::None,
            features: Default::default(),
        }
    }
}

impl Text {
    pub fn new(content: impl Into<String>, size: f32, weight: u16) -> Self {
        Self { content: content.into(), size, weight, ..Self::default() }
    }

    /// Replaces the characters, moving styled ranges with the text around them.
    pub fn set_content(&mut self, new: String) {
        if self.runs.is_empty() {
            self.content = new;
            return;
        }
        let old: Vec<char> = self.content.chars().collect();
        let fresh: Vec<char> = new.chars().collect();
        let prefix = old.iter().zip(&fresh).take_while(|(a, b)| a == b).count();
        let max_suffix = old.len().min(fresh.len()) - prefix;
        let suffix = old.iter().rev().zip(fresh.iter().rev()).take(max_suffix).take_while(|(a, b)| a == b).count();
        let (old_end, new_end) = (old.len() - suffix, fresh.len() - suffix);
        let map = |i: usize| -> usize {
            if i <= prefix {
                i
            } else if i >= old_end {
                i - old_end + new_end
            } else {
                // Inside the replaced span: clamp to its new extent.
                (prefix + (i - prefix)).min(new_end)
            }
        };
        for r in &mut self.runs {
            // Typing right after a styled range extends it, as in Figma.
            let grows = r.end == prefix && old_end == prefix && new_end > prefix;
            r.start = map(r.start);
            r.end = if grows { new_end } else { map(r.end) };
        }
        self.runs.retain(|r| r.end > r.start && !r.is_plain());
        self.content = new;
    }

    /// Applies `style` to characters `start..end`, splitting and merging runs.
    pub fn style_range(&mut self, start: usize, end: usize, style: &TextRun) {
        let len = self.content.chars().count();
        let (start, end) = (start.min(len), end.min(len));
        if start >= end {
            return;
        }
        // Expand runs to one per character boundary in the range, then rebuild.
        let mut cuts: Vec<usize> = vec![0, start, end, len];
        for r in &self.runs {
            cuts.push(r.start.min(len));
            cuts.push(r.end.min(len));
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut out: Vec<TextRun> = Vec::new();
        for w in cuts.windows(2) {
            let (a, b) = (w[0], w[1]);
            if a >= b {
                continue;
            }
            let mut piece = self.runs.iter().find(|r| r.start <= a && r.end >= b).cloned().unwrap_or_default();
            piece.start = a;
            piece.end = b;
            if a >= start && b <= end {
                piece.merge(style);
            }
            // A property equal to the layer's own is not an override.
            if piece.weight == Some(self.weight) {
                piece.weight = None;
            }
            if piece.italic == Some(self.italic) {
                piece.italic = None;
            }
            if piece.size == Some(self.size) {
                piece.size = None;
            }
            if piece.decoration == Some(self.decoration) {
                piece.decoration = None;
            }
            if piece.family.as_deref() == Some(self.family.as_str()) {
                piece.family = None;
            }
            if piece.is_plain() {
                continue;
            }
            match out.last_mut() {
                Some(prev) if prev.end == a && TextRun { start: 0, end: 0, ..prev.clone() } == TextRun { start: 0, end: 0, ..piece.clone() } => prev.end = b,
                _ => out.push(piece),
            }
        }
        self.runs = out;
    }

    /// The content split into (text, run) pieces in order; `None` is the layer's own style.
    pub fn pieces(&self) -> Vec<(String, Option<&TextRun>)> {
        let chars: Vec<char> = self.display().chars().collect();
        let mut out = Vec::new();
        let mut i = 0;
        let mut runs: Vec<&TextRun> = self.runs.iter().filter(|r| r.end > r.start).collect();
        runs.sort_by_key(|r| r.start);
        for r in runs {
            let (s, e) = (r.start.min(chars.len()), r.end.min(chars.len()));
            if s > i {
                out.push((chars[i..s].iter().collect(), None));
            }
            if e > s.max(i) {
                out.push((chars[s.max(i)..e].iter().collect(), Some(r)));
            }
            i = i.max(e);
        }
        if i < chars.len() {
            out.push((chars[i..].iter().collect(), None));
        }
        out
    }

    /// The marker drawn before line `i` of a list ("•" or "3."), with the gap after it.
    pub fn marker(&self, i: usize) -> Option<String> {
        match self.list {
            ListStyle::None => None,
            ListStyle::Bullet => Some("•\u{2002}".into()),
            ListStyle::Numbered => Some(format!("{}.\u{2002}", i + 1)),
        }
    }

    /// `pieces()` with the list markers put in front of every line.
    pub fn list_pieces(&self) -> Vec<(String, Option<&TextRun>)> {
        let mut out: Vec<(String, Option<&TextRun>)> = Vec::new();
        let mut line = 0;
        if let Some(m) = self.marker(0) {
            out.push((m, None));
        }
        for (piece, run) in self.pieces() {
            let mut rest = piece.as_str();
            while let Some(i) = rest.find('\n') {
                out.push((rest[..=i].to_string(), run));
                line += 1;
                if let Some(m) = self.marker(line) {
                    out.push((m, None));
                }
                rest = &rest[i + 1..];
            }
            if !rest.is_empty() {
                out.push((rest.to_string(), run));
            }
        }
        out
    }

    /// CSS `font-feature-settings` for the features ("'tnum' 1, 'liga' 0").
    pub fn feature_css(&self) -> Option<String> {
        (!self.features.is_empty()).then(|| self.features.iter().map(|(k, v)| format!("'{k}' {v}")).collect::<Vec<_>>().join(", "))
    }

    /// The characters as drawn, after the case transform.
    pub fn display(&self) -> String {
        match self.case {
            TextCase::Original => self.content.clone(),
            TextCase::Upper => self.content.to_uppercase(),
            TextCase::Lower => self.content.to_lowercase(),
            TextCase::Title => {
                let mut out = String::with_capacity(self.content.len());
                let mut start = true;
                for c in self.content.chars() {
                    if start && c.is_alphabetic() {
                        out.extend(c.to_uppercase());
                    } else {
                        out.push(c);
                    }
                    start = c.is_whitespace();
                }
                out
            }
        }
    }

    /// Layer name derived from the content, like Figma: the first line, shortened.
    pub fn layer_name(&self) -> String {
        let first = self.content.lines().find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        let mut name: String = first.chars().take(60).collect();
        if first.chars().count() > 60 {
            name.push('…');
        }
        if name.is_empty() { "Text".into() } else { name }
    }

    /// Line height in pixels, given the font's natural line spacing for `Auto`.
    pub fn line_px(&self, natural: f32) -> f32 {
        match self.line_height {
            LineHeight::Auto => natural,
            LineHeight::Px(px) => px.max(0.0),
            LineHeight::Percent(p) => self.size * p.max(0.0) / 100.0,
        }
    }

    pub fn letter_spacing_px(&self) -> f32 {
        self.size * self.letter_spacing / 100.0
    }

    pub fn weight_name(&self) -> String {
        let base = match self.weight {
            0..=149 => "Thin",
            150..=249 => "Extra Light",
            250..=349 => "Light",
            350..=449 => "Regular",
            450..=549 => "Medium",
            550..=649 => "Semi Bold",
            650..=749 => "Bold",
            750..=849 => "Extra Bold",
            _ => "Black",
        };
        match (self.italic, base) {
            (false, b) => b.into(),
            (true, "Regular") => "Italic".into(),
            (true, b) => format!("{b} Italic"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_transforms_and_names() {
        let mut t = Text::new("hello  big world\nsecond", 16.0, 400);
        t.case = TextCase::Title;
        assert_eq!(t.display(), "Hello  Big World\nSecond");
        t.case = TextCase::Upper;
        assert_eq!(t.display(), "HELLO  BIG WORLD\nSECOND");
        assert_eq!(t.layer_name(), "hello  big world");
        assert_eq!(Text::default().layer_name(), "Text");
        t.italic = true;
        t.weight = 700;
        assert_eq!(t.weight_name(), "Bold Italic");
    }

    #[test]
    fn line_height_units() {
        let mut t = Text::new("a", 20.0, 400);
        assert_eq!(t.line_px(24.2), 24.2);
        t.line_height = LineHeight::Percent(150.0);
        assert_eq!(t.line_px(24.2), 30.0);
        t.line_height = LineHeight::Px(18.0);
        assert_eq!(t.line_px(24.2), 18.0);
        t.letter_spacing = -2.0;
        assert_eq!(t.letter_spacing_px(), -0.4);
    }

    #[test]
    fn styled_ranges_follow_edits() {
        let mut t = Text::new("Hello brave world", 16.0, 400);
        t.style_range(6, 11, &TextRun { weight: Some(700), ..Default::default() });
        assert_eq!(t.runs.len(), 1);
        assert_eq!((t.runs[0].start, t.runs[0].end), (6, 11));
        let p = t.pieces();
        assert_eq!(p.iter().map(|(s, r)| (s.as_str(), r.is_some())).collect::<Vec<_>>(), vec![("Hello ", false), ("brave", true), (" world", false)]);
        // Typing before the range shifts it.
        t.set_content("Oh, Hello brave world".into());
        assert_eq!((t.runs[0].start, t.runs[0].end), (10, 15));
        // Overlapping styles split the run.
        t.style_range(12, 21, &TextRun { italic: Some(true), ..Default::default() });
        assert_eq!(t.runs.len(), 3, "{:?}", t.runs);
        // Making it the layer's own weight removes the override.
        t.style_range(0, 21, &TextRun { weight: Some(400), italic: Some(false), ..Default::default() });
        assert!(t.runs.is_empty(), "{:?}", t.runs);
    }

    #[test]
    fn serializes_for_the_ui() {
        let json = serde_json::to_string(&Text::default()).unwrap();
        assert!(json.contains("\"lineHeight\":{\"unit\":\"auto\"}"));
        assert!(json.contains("\"resize\":\"auto-width\""));
        let t: Text = serde_json::from_str("{\"content\":\"x\",\"lineHeight\":{\"unit\":\"px\",\"value\":20}}").unwrap();
        assert_eq!((t.size, t.line_height), (12.0, LineHeight::Px(20.0)));
    }
}
