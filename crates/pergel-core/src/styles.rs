//! Figma's local styles: named text styles, colour styles and effect styles. Layers link to a
//! style by id; editing the style restyles every linked layer.

use serde::{Deserialize, Serialize};

use crate::geom::Color;
use crate::paint::Paint;
use crate::scene::{NodeId, Scene};
use crate::style::Effect;
use crate::text::{Decoration, LineHeight, TextCase};

/// The typography a text style fixes (everything but the characters and box behaviour).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypeStyle {
    pub family: String,
    pub weight: u16,
    #[serde(default)]
    pub italic: bool,
    pub size: f32,
    #[serde(default)]
    pub line_height: LineHeight,
    #[serde(default)]
    pub letter_spacing: f32,
    #[serde(default)]
    pub decoration: Decoration,
    #[serde(default)]
    pub case: TextCase,
}

impl TypeStyle {
    pub fn of(t: &crate::Text) -> Self {
        Self {
            family: t.family.clone(),
            weight: t.weight,
            italic: t.italic,
            size: t.size,
            line_height: t.line_height,
            letter_spacing: t.letter_spacing,
            decoration: t.decoration,
            case: t.case,
        }
    }

    pub fn apply(&self, t: &mut crate::Text) {
        t.family = self.family.clone();
        t.weight = self.weight;
        t.italic = self.italic;
        t.size = self.size;
        t.line_height = self.line_height;
        t.letter_spacing = self.letter_spacing;
        t.decoration = self.decoration;
        t.case = self.case;
    }

    /// "Inter Bold 32/40" style summary.
    pub fn summary(&self) -> String {
        let t = crate::Text { family: self.family.clone(), weight: self.weight, italic: self.italic, ..Default::default() };
        let lh = match self.line_height {
            LineHeight::Auto => "auto".to_string(),
            LineHeight::Px(v) => crate::paint::fmt(v),
            LineHeight::Percent(p) => format!("{}%", crate::paint::fmt(p)),
        };
        format!("{} {} {}/{}", self.family, t.weight_name(), crate::paint::fmt(self.size), lh)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum StyleValue {
    Text(TypeStyle),
    /// A colour style: a solid colour or a gradient/image paint.
    Color { color: Color, #[serde(default, skip_serializing_if = "Option::is_none")] paint: Option<Paint> },
    Effect { effects: Vec<Effect> },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Style {
    pub id: String,
    /// Slash groups, as in Figma: "Heading/H1".
    pub name: String,
    #[serde(flatten)]
    pub value: StyleValue,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Styles {
    #[serde(default)]
    pub list: Vec<Style>,
}

/// Which styles a layer uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StyleLinks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fill: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stroke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect: Option<String>,
}

impl StyleLinks {
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.fill.is_none() && self.stroke.is_none() && self.effect.is_none()
    }
}

impl Styles {
    pub fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Style> {
        self.list.iter().find(|s| s.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&Style> {
        self.list.iter().find(|s| s.name == name)
    }

    pub fn new_id(&self) -> String {
        (1..).map(|n| format!("s{n}")).find(|id| self.get(id).is_none()).unwrap_or_default()
    }

    /// Adds or replaces a style by name; returns its id.
    pub fn set(&mut self, name: &str, value: StyleValue) -> String {
        if let Some(s) = self.list.iter_mut().find(|s| s.name == name) {
            s.value = value;
            return s.id.clone();
        }
        let id = self.new_id();
        self.list.push(Style { id: id.clone(), name: name.to_string(), value, description: String::new() });
        id
    }
}

impl Scene {
    /// Restyles every layer linked to a style; returns the layers that changed.
    pub fn apply_styles(&mut self, styles: &Styles) -> Vec<NodeId> {
        let linked: Vec<(NodeId, StyleLinks)> = self.nodes().filter_map(|n| n.styles.as_deref().map(|s| (n.id, s.clone()))).collect();
        let mut changed = Vec::new();
        for (id, links) in linked {
            let Some(before) = self.node(id).cloned() else { continue };
            let mut after = before.clone();
            if let Some(StyleValue::Text(ts)) = links.text.as_deref().and_then(|s| styles.get(s)).map(|s| &s.value)
                && let Some(t) = after.text.as_mut()
            {
                ts.apply(t);
            }
            if let Some(StyleValue::Color { color, paint }) = links.fill.as_deref().and_then(|s| styles.get(s)).map(|s| &s.value) {
                after.fill = *color;
                after.fill_on = true;
                after.paint = paint.clone().map(Box::new);
            }
            if let Some(StyleValue::Color { color, .. }) = links.stroke.as_deref().and_then(|s| styles.get(s)).map(|s| &s.value) {
                let w = after.stroke.map_or(1.0, |s| s.1);
                after.stroke = Some((*color, w));
            }
            if let Some(StyleValue::Effect { effects }) = links.effect.as_deref().and_then(|s| styles.get(s)).map(|s| &s.value) {
                after.effects = effects.clone();
            }
            if after != before {
                self.update(id, |n| *n = after.clone());
                changed.push(id);
            }
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Node, Text};

    #[test]
    fn editing_a_style_restyles_linked_layers() {
        let mut styles = Styles::default();
        let h1 = styles.set("Heading/H1", StyleValue::Text(TypeStyle::of(&Text::new("", 32.0, 700))));
        let brand = styles.set("Brand/Primary", StyleValue::Color { color: Color(0xff7c3aed), paint: None });
        let mut scene = Scene::default();
        let id = scene.add(Node::text(Text::new("Title", 16.0, 400), 0.0, 0.0, Color(0xff000000)));
        scene.update(id, |n| n.styles = Some(Box::new(StyleLinks { text: Some(h1.clone()), fill: Some(brand.clone()), ..Default::default() })));
        assert_eq!(scene.apply_styles(&styles), vec![id]);
        let n = scene.node(id).unwrap();
        assert_eq!((n.text.as_ref().unwrap().size, n.fill), (32.0, Color(0xff7c3aed)));
        assert_eq!(TypeStyle::of(n.text.as_ref().unwrap()).summary(), "Inter Bold 32/auto");
        let mut ts = TypeStyle::of(n.text.as_ref().unwrap());
        ts.size = 40.0;
        styles.set("Heading/H1", StyleValue::Text(ts));
        scene.apply_styles(&styles);
        assert_eq!(scene.node(id).unwrap().text.as_ref().unwrap().size, 40.0);
    }
}
