//! Figma's local variables: named values (colour, number, string, boolean) in collections with
//! modes (e.g. Light / Dark), bound to layer properties. Changing a variable or the active mode
//! updates every bound layer; design to code turns bindings into CSS custom properties.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::geom::Color;
use crate::scene::{NodeId, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VarKind {
    Color,
    Number,
    String,
    Boolean,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "lowercase")]
pub enum VarValue {
    Color(Color),
    Number(f32),
    String(String),
    Boolean(bool),
    /// Another variable's id.
    Alias(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mode {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub modes: Vec<Mode>,
    /// The mode the document shows.
    pub active: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    pub id: String,
    /// Slash-separated groups, as in Figma: "color/brand/primary".
    pub name: String,
    pub collection: String,
    #[serde(rename = "type")]
    pub kind: VarKind,
    /// Value per mode id.
    pub values: BTreeMap<String, VarValue>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Variables {
    #[serde(default)]
    pub collections: Vec<Collection>,
    #[serde(default)]
    pub variables: Vec<Variable>,
}

/// Layer properties a variable can drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BindProp {
    Fill,
    Stroke,
    StrokeWidth,
    Radius,
    Opacity,
    Gap,
    Padding,
    Width,
    Height,
    FontSize,
    Text,
    Visible,
}

impl BindProp {
    pub fn kind(self) -> VarKind {
        match self {
            BindProp::Fill | BindProp::Stroke => VarKind::Color,
            BindProp::Text => VarKind::String,
            BindProp::Visible => VarKind::Boolean,
            _ => VarKind::Number,
        }
    }
}

/// Property → variable id.
pub type Bindings = BTreeMap<BindProp, String>;

impl Variables {
    pub fn is_empty(&self) -> bool {
        self.variables.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.id == id)
    }

    pub fn by_name(&self, name: &str) -> Option<&Variable> {
        self.variables.iter().find(|v| v.name == name)
    }

    /// The value in the collection's active mode, following aliases.
    pub fn resolve(&self, id: &str) -> Option<VarValue> {
        let mut cur = self.get(id)?;
        for _ in 0..16 {
            let col = self.collections.iter().find(|c| c.id == cur.collection)?;
            let value = cur.values.get(&col.active).or_else(|| col.modes.first().and_then(|m| cur.values.get(&m.id)))?;
            match value {
                VarValue::Alias(next) => cur = self.get(next)?,
                v => return Some(v.clone()),
            }
        }
        None
    }

    /// A fresh id that no collection, mode or variable uses.
    pub fn new_id(&self, prefix: &str) -> String {
        let used = |s: &str| {
            self.collections.iter().any(|c| c.id == s || c.modes.iter().any(|m| m.id == s)) || self.variables.iter().any(|v| v.id == s)
        };
        (1..).map(|n| format!("{prefix}{n}")).find(|s| !used(s)).unwrap_or_default()
    }

    /// The collection named `name`, created (with one mode, "Mode 1") if missing.
    pub fn collection(&mut self, name: &str) -> String {
        if let Some(c) = self.collections.iter().find(|c| c.name == name) {
            return c.id.clone();
        }
        let id = self.new_id("c");
        let mode = format!("{id}m1");
        self.collections.push(Collection { id: id.clone(), name: name.into(), modes: vec![Mode { id: mode.clone(), name: "Mode 1".into() }], active: mode });
        id
    }

    /// Creates or updates a variable by name in `collection` with the same value in every mode
    /// (or `per_mode` values by mode name). Returns its id.
    pub fn set(&mut self, collection: &str, name: &str, value: VarValue, per_mode: &[(String, VarValue)]) -> String {
        let cid = self.collection(collection);
        let fresh = !self.variables.iter().any(|v| v.collection == cid);
        let col = self.collections.iter_mut().find(|c| c.id == cid).expect("just made");
        if fresh && !per_mode.is_empty() && col.modes.len() == 1 && col.modes[0].name == "Mode 1" {
            // A new collection takes the modes it is first given.
            let first = per_mode[0].0.clone();
            col.modes[0].name = first;
        }
        for (mode_name, _) in per_mode {
            if !col.modes.iter().any(|m| &m.name == mode_name) {
                let id = format!("{cid}m{}", col.modes.len() + 1);
                col.modes.push(Mode { id, name: mode_name.clone() });
            }
        }
        let modes = col.modes.clone();
        let kind = match &value {
            VarValue::Color(_) => VarKind::Color,
            VarValue::Number(_) => VarKind::Number,
            VarValue::String(_) => VarKind::String,
            VarValue::Boolean(_) => VarKind::Boolean,
            VarValue::Alias(a) => self.get(a).map_or(VarKind::String, |v| v.kind),
        };
        let values: BTreeMap<String, VarValue> =
            modes.iter().map(|m| (m.id.clone(), per_mode.iter().find(|(n, _)| *n == m.name).map_or_else(|| value.clone(), |(_, v)| v.clone()))).collect();
        if let Some(v) = self.variables.iter_mut().find(|v| v.name == name && v.collection == cid) {
            v.values = values;
            v.kind = kind;
            return v.id.clone();
        }
        let id = self.new_id("v");
        self.variables.push(Variable { id: id.clone(), name: name.into(), collection: cid, kind, values, description: String::new() });
        id
    }

    /// The CSS custom property for a variable: "color/brand/primary" → "--color-brand-primary".
    pub fn css_name(name: &str) -> String {
        let mut out = String::from("--");
        for c in name.chars() {
            if c.is_ascii_alphanumeric() {
                out.push(c.to_ascii_lowercase());
            } else if !out.ends_with('-') {
                out.push('-');
            }
        }
        out.trim_end_matches('-').to_string()
    }

    /// A value as CSS / design-token text.
    pub fn display(value: &VarValue) -> String {
        match value {
            VarValue::Color(c) => {
                let a = c.argb() >> 24;
                if a == 255 { format!("#{:06X}", c.argb() & 0xffffff) } else { format!("#{:06X}{a:02X}", c.argb() & 0xffffff) }
            }
            VarValue::Number(n) => crate::paint::fmt(*n),
            VarValue::String(s) => s.clone(),
            VarValue::Boolean(b) => b.to_string(),
            VarValue::Alias(a) => format!("{{{a}}}"),
        }
    }
}

impl Scene {
    /// Writes every bound property of the layers from the variables' current values. Returns the
    /// layers that changed (recorded for undo when inside a step).
    pub fn apply_variables(&mut self, vars: &Variables) -> Vec<NodeId> {
        let bound: Vec<(NodeId, Bindings)> =
            self.nodes().filter_map(|n| n.bindings.as_deref().map(|b| (n.id, b.clone()))).collect();
        let mut changed = Vec::new();
        for (id, bindings) in bound {
            let Some(before) = self.node(id).cloned() else { continue };
            let mut after = before.clone();
            for (prop, var) in &bindings {
                let Some(value) = vars.resolve(var) else { continue };
                apply(&mut after, *prop, &value);
            }
            if after != before {
                let geometry = after.bounds != before.bounds;
                let bounds = after.bounds;
                self.update(id, |n| {
                    let keep = n.bounds;
                    *n = Node { bounds: keep, ..after.clone() };
                });
                if geometry {
                    self.set_bounds(id, bounds);
                }
                changed.push(id);
            }
        }
        changed
    }
}

use crate::scene::Node;

fn apply(n: &mut Node, prop: BindProp, value: &VarValue) {
    let num = match value {
        VarValue::Number(v) => Some(*v),
        _ => None,
    };
    match (prop, value) {
        (BindProp::Fill, VarValue::Color(c)) => {
            n.fill = *c;
            n.fill_on = true;
            n.paint = None;
        }
        (BindProp::Stroke, VarValue::Color(c)) => {
            let w = n.stroke.map_or(1.0, |s| s.1);
            n.stroke = Some((*c, w));
        }
        (BindProp::Text, VarValue::String(s)) => {
            if let Some(t) = n.text.as_mut() {
                t.content = s.clone();
            }
        }
        (BindProp::Visible, VarValue::Boolean(b)) => n.visible = *b,
        _ => {
            let Some(v) = num else { return };
            match prop {
                BindProp::StrokeWidth => {
                    if let Some(s) = n.stroke.as_mut() {
                        s.1 = v.max(0.0);
                    }
                }
                BindProp::Radius => {
                    n.radius = v.max(0.0);
                    n.corners = None;
                }
                BindProp::Opacity => n.opacity = if v > 1.0 { v / 100.0 } else { v }.clamp(0.0, 1.0),
                BindProp::Gap => {
                    if let Some(l) = n.layout.as_mut() {
                        l.gap = v;
                    }
                }
                BindProp::Padding => {
                    if let Some(l) = n.layout.as_mut() {
                        l.padding = [v; 4];
                    }
                }
                BindProp::Width => n.bounds.w = v.max(0.0),
                BindProp::Height => n.bounds.h = v.max(0.0),
                BindProp::FontSize => {
                    if let Some(t) = n.text.as_mut() {
                        t.size = v.max(1.0);
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::rect;

    #[test]
    fn modes_and_aliases_drive_bound_layers() {
        let mut vars = Variables::default();
        let modes = vec![("Light".to_string(), VarValue::Color(Color(0xffffffff))), ("Dark".to_string(), VarValue::Color(Color(0xff111827)))];
        let bg = vars.set("Theme", "color/background", VarValue::Color(Color(0xffffffff)), &modes);
        let radius = vars.set("Theme", "radius/md", VarValue::Number(8.0), &[]);
        let alias = vars.set("Theme", "color/card", VarValue::Alias(bg.clone()), &[]);
        let names: Vec<&str> = vars.collections[0].modes.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["Light", "Dark"]);
        let mut scene = Scene::default();
        let card = scene.add(rect("Card", 0.0, 0.0, 100.0, 100.0));
        scene.update(card, |n| {
            let mut b = Bindings::new();
            b.insert(BindProp::Fill, alias.clone());
            b.insert(BindProp::Radius, radius.clone());
            n.bindings = Some(Box::new(b));
        });
        let dark = vars.collections[0].modes.iter().find(|m| m.name == "Dark").unwrap().id.clone();
        vars.collections[0].active = dark;
        assert_eq!(scene.apply_variables(&vars), vec![card]);
        let n = scene.node(card).unwrap();
        assert_eq!((n.fill, n.radius), (Color(0xff111827), 8.0));
        assert_eq!(Variables::css_name("color/Brand Primary"), "--color-brand-primary");
        assert_eq!(Variables::display(&VarValue::Color(Color(0x80ff0000))), "#FF000080");
    }
}
