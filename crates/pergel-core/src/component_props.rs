//! Component properties (Figma's boolean, text and instance-swap properties): a main component
//! (or its component set) defines them, layers inside it link to them, and each instance keeps
//! its own values. Values are applied whenever an instance syncs with its main.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::components::Component;
use crate::components::overrides::SWAP;
use crate::scene::{NodeId, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PropKind {
    Boolean,
    Text,
    InstanceSwap,
}

/// A property value: a boolean, a text, or the main component an instance-swap slot shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PropValue {
    Bool(bool),
    Swap(NodeId),
    Text(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComponentProp {
    pub id: String,
    pub name: String,
    pub kind: PropKind,
    pub default: PropValue,
}

/// What a layer inside a main component takes from its properties.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PropRefs {
    /// Boolean property shown as the layer's visibility.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<String>,
    /// Text property shown as the text layer's characters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Instance-swap property choosing the main component of a nested instance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub swap: Option<String>,
}

impl PropRefs {
    pub fn is_empty(&self) -> bool {
        self.visible.is_none() && self.text.is_none() && self.swap.is_none()
    }
}

/// A property name as a JSX prop: "Show icon" → "showIcon".
pub fn camel(name: &str) -> String {
    let mut out = String::new();
    for (i, w) in name.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).enumerate() {
        let mut c = w.chars();
        let first = c.next().unwrap_or_default();
        if i == 0 {
            out.extend(first.to_lowercase());
        } else {
            out.extend(first.to_uppercase());
        }
        out.push_str(c.as_str());
    }
    out
}

/// A short random id for a new property.
pub fn new_prop_id(existing: &[ComponentProp]) -> String {
    let mut n = existing.len() as u64 + 1;
    loop {
        let id = format!("p{n}");
        if !existing.iter().any(|p| p.id == id) {
            return id;
        }
        n += 1;
    }
}

impl Scene {
    /// The properties an instance of `main` has: the main's own, or its component set's.
    pub fn prop_defs(&self, main: NodeId) -> Vec<ComponentProp> {
        let own = self.node(main).map(|n| n.props.clone()).unwrap_or_default();
        if !own.is_empty() {
            return own;
        }
        self.variant_set_of(main).and_then(|s| self.node(s)).map(|s| s.props.clone()).unwrap_or_default()
    }

    /// Where a main component's (or set's) properties live: the set for a variant.
    pub fn props_owner(&self, main: NodeId) -> NodeId {
        match self.variant_set_of(main) {
            Some(set) if self.node(main).is_some_and(|n| n.props.is_empty()) => set,
            _ => main,
        }
    }

    /// The main component a layer belongs to (itself for a main).
    pub fn component_of(&self, id: NodeId) -> Option<NodeId> {
        let mut cur = Some(id);
        while let Some(i) = cur {
            let n = self.node(i)?;
            if n.component == Some(Component::Main) || n.variant_set {
                return Some(i);
            }
            if matches!(n.component, Some(Component::Instance(_))) && i != id {
                return None;
            }
            cur = n.parent;
        }
        None
    }

    /// An instance's value for property `id` (its default when the instance has none).
    pub fn prop_value(&self, inst: NodeId, def: &ComponentProp) -> PropValue {
        self.node(inst).and_then(|n| n.prop_values.get(&def.id).cloned()).unwrap_or_else(|| def.default.clone())
    }

    /// Instance-swap values: nested instances become the chosen component (before their
    /// structure is mirrored, so the swapped content isn't overwritten).
    pub(crate) fn apply_swaps(&mut self, inst: NodeId) {
        let Some(main) = self.main_of(inst) else { return };
        let defs = self.prop_defs(main);
        if !defs.iter().any(|d| d.kind == PropKind::InstanceSwap) {
            return;
        }
        for d in self.subtree(inst).into_iter().skip(1) {
            let Some(src) = self.node(d).and_then(|n| n.source) else { continue };
            let Some(pid) = self.node(src).and_then(|s| s.prop_refs.as_deref()).and_then(|r| r.swap.clone()) else { continue };
            let Some(def) = defs.iter().find(|p| p.id == pid) else { continue };
            let PropValue::Swap(target) = self.prop_value(inst, def) else { continue };
            let own = match self.node(src).and_then(|s| s.component) {
                Some(Component::Instance(m)) => Some(m),
                _ => None,
            };
            let swapped = self.node(d).is_some_and(|n| n.overrides & SWAP != 0);
            if Some(target) == own || !self.is_main(target) {
                if swapped {
                    self.unswap(d, src);
                }
            } else if self.main_of(d) != Some(target) {
                self.swap_nested(d, target, src);
            }
        }
    }

    /// Boolean and text values, after the instance mirrored its main.
    pub(crate) fn apply_values(&mut self, inst: NodeId) {
        let Some(main) = self.main_of(inst) else { return };
        let defs = self.prop_defs(main);
        if defs.is_empty() {
            return;
        }
        for d in self.subtree(inst) {
            let Some(src) = self.node(d).and_then(|n| n.source) else { continue };
            let Some(refs) = self.node(src).and_then(|s| s.prop_refs.as_deref()).cloned() else { continue };
            if let Some(def) = refs.visible.and_then(|p| defs.iter().find(|x| x.id == p))
                && let PropValue::Bool(v) = self.prop_value(inst, def)
                && self.node(d).is_some_and(|n| n.visible != v)
            {
                self.update(d, |n| n.visible = v);
            }
            if let Some(def) = refs.text.and_then(|p| defs.iter().find(|x| x.id == p))
                && let PropValue::Text(v) = self.prop_value(inst, def)
                && self.node(d).and_then(|n| n.text.as_deref()).is_some_and(|t| t.content != v)
            {
                self.update(d, |n| {
                    if let Some(t) = n.text.as_mut() {
                        t.set_content(v.clone());
                    }
                });
            }
        }
    }

    /// A nested instance shows `target` instead of its main's own component.
    fn swap_nested(&mut self, d: NodeId, target: NodeId, src: NodeId) {
        let Some(tree) = self.extract(target) else { return };
        let kids = self.node(d).map(|n| n.children.clone()).unwrap_or_default();
        for c in kids {
            self.remove(c);
        }
        for child in &tree.children {
            self.insert_tree(child, Some(d), None);
        }
        self.link_subtree(d, target);
        let size = (tree.bounds.w, tree.bounds.h);
        self.update(d, |n| {
            n.component = Some(Component::Instance(target));
            n.bounds.w = size.0;
            n.bounds.h = size.1;
        });
        self.touch(d);
        let n = &mut self.nodes[d as usize];
        n.source = Some(src);
        n.overrides = SWAP;
    }

    /// Back to the main's own nested component: the content is mirrored again on the next sync.
    fn unswap(&mut self, d: NodeId, src: NodeId) {
        let kids = self.node(d).map(|n| n.children.clone()).unwrap_or_default();
        for c in kids {
            self.remove(c);
        }
        let own = self.node(src).and_then(|s| s.component);
        self.update(d, |n| n.component = own);
        self.touch(d);
        self.nodes[d as usize].overrides &= !SWAP;
    }

    /// Links a layer inside a component to its properties by name; a text property without a
    /// default takes the layer's text, an instance-swap property its nested component.
    pub fn bind_props_by_name(&mut self, id: NodeId, visible: Option<&str>, text: Option<&str>, swap: Option<&str>, warnings: &mut Vec<String>) {
        let Some(owner) = self.node(id).and_then(|n| n.parent).and_then(|p| self.component_of(p)).map(|c| self.props_owner(c)) else {
            warnings.push("`property` only works on layers inside a component".into());
            return;
        };
        let mut defs = self.node(owner).map(|n| n.props.clone()).unwrap_or_default();
        let find = |defs: &[ComponentProp], name: &str, kind: PropKind, warnings: &mut Vec<String>| {
            let found = defs.iter().position(|d| d.name.eq_ignore_ascii_case(name) && d.kind == kind);
            if found.is_none() {
                warnings.push(format!("No {kind:?} property named {name:?} on the component"));
            }
            found
        };
        let mut refs = self.node(id).and_then(|n| n.prop_refs.as_deref()).cloned().unwrap_or_default();
        if let Some(i) = visible.and_then(|v| find(&defs, v, PropKind::Boolean, warnings)) {
            refs.visible = Some(defs[i].id.clone());
        }
        if let Some(i) = text.and_then(|v| find(&defs, v, PropKind::Text, warnings)) {
            refs.text = Some(defs[i].id.clone());
            if defs[i].default == PropValue::Text(String::new())
                && let Some(t) = self.node(id).and_then(|n| n.text.as_deref())
            {
                defs[i].default = PropValue::Text(t.content.clone());
            }
        }
        if let Some(i) = swap.and_then(|v| find(&defs, v, PropKind::InstanceSwap, warnings)) {
            refs.swap = Some(defs[i].id.clone());
            if defs[i].default == PropValue::Swap(0)
                && let Some(m) = self.main_of(id)
            {
                defs[i].default = PropValue::Swap(m);
            }
        }
        self.update(owner, |n| n.props = defs);
        self.update(id, |n| n.prop_refs = (!refs.is_empty()).then(|| Box::new(refs)));
    }

    /// Instance-property summary for agents: "Label=Buy; Show icon=false".
    pub fn prop_summary(&self, inst: NodeId) -> Option<String> {
        let main = self.main_of(inst)?;
        let defs = self.prop_defs(main);
        if defs.is_empty() {
            return None;
        }
        Some(
            defs.iter()
                .map(|d| {
                    let v = match self.prop_value(inst, d) {
                        PropValue::Bool(b) => b.to_string(),
                        PropValue::Text(t) => t,
                        PropValue::Swap(m) => self.node(m).map_or("?".into(), |n| n.name.clone()),
                    };
                    format!("{}={v}", d.name)
                })
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    /// Property definitions for agents: "Label (text) = Button; Show icon (boolean) = true".
    pub fn prop_defs_summary(&self, owner: NodeId) -> Option<String> {
        let defs = self.node(owner).map(|n| n.props.clone()).unwrap_or_default();
        if defs.is_empty() {
            return None;
        }
        Some(
            defs.iter()
                .map(|d| {
                    let kind = match d.kind {
                        PropKind::Boolean => "boolean",
                        PropKind::Text => "text",
                        PropKind::InstanceSwap => "instance-swap",
                    };
                    let v = match &d.default {
                        PropValue::Bool(b) => b.to_string(),
                        PropValue::Text(t) => t.clone(),
                        PropValue::Swap(m) => self.node(*m).map_or("?".into(), |n| n.name.clone()),
                    };
                    format!("{} ({kind}) = {v}", d.name)
                })
                .collect::<Vec<_>>()
                .join("; "),
        )
    }

    /// Sets instance values by property name (agents): booleans, texts, or component names/ids
    /// for swaps. Returns the names that matched nothing.
    pub fn set_prop_values_by_name(&mut self, inst: NodeId, values: &BTreeMap<String, serde_json::Value>) -> Vec<String> {
        let Some(main) = self.main_of(inst) else { return values.keys().cloned().collect() };
        let defs = self.prop_defs(main);
        let mut unknown = Vec::new();
        let mut set: Vec<(String, PropValue)> = Vec::new();
        for (name, v) in values {
            let Some(def) = defs.iter().find(|d| d.name.eq_ignore_ascii_case(name) || d.id == *name) else {
                unknown.push(name.clone());
                continue;
            };
            let value = match (def.kind, v) {
                (PropKind::Boolean, serde_json::Value::Bool(b)) => PropValue::Bool(*b),
                (PropKind::Boolean, serde_json::Value::String(s)) => PropValue::Bool(s == "true"),
                (PropKind::Text, v) => PropValue::Text(v.as_str().map_or_else(|| v.to_string(), str::to_string)),
                (PropKind::InstanceSwap, v) => {
                    let by_id = v.as_u64().map(|i| i as NodeId).or_else(|| v.as_str().and_then(|s| s.parse().ok()));
                    let by_name = v.as_str().and_then(|s| self.mains().into_iter().find(|m| self.node(*m).is_some_and(|n| n.name.eq_ignore_ascii_case(s))));
                    match by_id.filter(|i| self.is_main(*i)).or(by_name) {
                        Some(m) => PropValue::Swap(m),
                        None => {
                            unknown.push(format!("{name} (no component {v})"));
                            continue;
                        }
                    }
                }
                _ => {
                    unknown.push(name.clone());
                    continue;
                }
            };
            set.push((def.id.clone(), value));
        }
        if !set.is_empty() {
            self.update(inst, |n| {
                for (k, v) in set {
                    n.prop_values.insert(k, v);
                }
            });
            self.sync_instance(inst);
        }
        unknown
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};
    use crate::{Color, Node, Text};

    #[test]
    fn instances_show_their_own_property_values() {
        let mut scene = Scene::default();
        let button = scene.add(frame("Button", 0.0, 0.0, 120.0, 40.0));
        let label = scene.insert(Node::text(Text::new("Button", 14.0, 500), 10.0, 10.0, Color(0xff00_0000)), Some(button), None);
        let icon = scene.insert(rect("Icon", 90.0, 10.0, 20.0, 20.0), Some(button), None);
        scene.update(button, |n| {
            n.component = Some(Component::Main);
            n.props = vec![
                ComponentProp { id: "p1".into(), name: "Label".into(), kind: PropKind::Text, default: PropValue::Text("Button".into()) },
                ComponentProp { id: "p2".into(), name: "Show icon".into(), kind: PropKind::Boolean, default: PropValue::Bool(true) },
            ];
        });
        scene.update(label, |n| n.prop_refs = Some(Box::new(PropRefs { text: Some("p1".into()), ..Default::default() })));
        scene.update(icon, |n| n.prop_refs = Some(Box::new(PropRefs { visible: Some("p2".into()), ..Default::default() })));
        let inst = scene.create_instance(button, None, None, (300.0, 100.0)).unwrap();
        let mut values = BTreeMap::new();
        values.insert("Label".to_string(), serde_json::json!("Buy now"));
        values.insert("show icon".to_string(), serde_json::json!(false));
        assert!(scene.set_prop_values_by_name(inst, &values).is_empty());
        let kids = scene.node(inst).unwrap().children.clone();
        assert_eq!(scene.node(kids[0]).unwrap().text.as_ref().unwrap().content, "Buy now");
        assert!(!scene.node(kids[1]).unwrap().visible);
        // The main changes; the instance keeps its values.
        scene.update(icon, |n| n.fill = Color(0xffff_0000));
        scene.sync_components(&[icon]);
        let kids = scene.node(inst).unwrap().children.clone();
        assert_eq!(scene.node(kids[1]).unwrap().fill, Color(0xffff_0000));
        assert!(!scene.node(kids[1]).unwrap().visible);
        assert_eq!(scene.prop_summary(inst).unwrap(), "Label=Buy now; Show icon=false");
    }

    #[test]
    fn instance_swap_shows_another_component() {
        let mut scene = Scene::default();
        let star = scene.add(frame("Icon/Star", 0.0, 200.0, 24.0, 24.0));
        scene.insert(rect("Star shape", 2.0, 202.0, 20.0, 20.0), Some(star), None);
        let heart = scene.add(frame("Icon/Heart", 40.0, 200.0, 24.0, 24.0));
        scene.insert(rect("Heart shape", 42.0, 202.0, 20.0, 20.0), Some(heart), None);
        scene.insert(rect("Heart dot", 50.0, 210.0, 4.0, 4.0), Some(heart), None);
        for m in [star, heart] {
            scene.update(m, |n| n.component = Some(Component::Main));
        }
        let card = scene.add(frame("Card", 0.0, 0.0, 200.0, 100.0));
        let slot = scene.create_instance(star, Some(card), None, (20.0, 20.0)).unwrap();
        scene.update(card, |n| {
            n.component = Some(Component::Main);
            n.props = vec![ComponentProp { id: "p1".into(), name: "Icon".into(), kind: PropKind::InstanceSwap, default: PropValue::Swap(star) }];
        });
        scene.update(slot, |n| n.prop_refs = Some(Box::new(PropRefs { swap: Some("p1".into()), ..Default::default() })));
        let inst = scene.create_instance(card, None, None, (400.0, 50.0)).unwrap();
        let mut values = BTreeMap::new();
        values.insert("Icon".to_string(), serde_json::json!("Icon/Heart"));
        assert!(scene.set_prop_values_by_name(inst, &values).is_empty());
        let nested = scene.node(inst).unwrap().children[0];
        assert_eq!(scene.main_of(nested), Some(heart));
        assert_eq!(scene.node(nested).unwrap().children.len(), 2, "the heart's two layers");
        // Editing the heart reaches the swapped slot.
        let dot = scene.node(heart).unwrap().children[1];
        scene.update(dot, |n| n.fill = Color(0xff00_ff00));
        scene.sync_components(&[dot]);
        let nested = scene.node(inst).unwrap().children[0];
        let kid = scene.node(nested).unwrap().children[1];
        assert_eq!(scene.node(kid).unwrap().fill, Color(0xff00_ff00));
        // Back to the default.
        values.insert("Icon".to_string(), serde_json::json!(star));
        scene.set_prop_values_by_name(inst, &values);
        scene.sync_instance(inst);
        let nested = scene.node(inst).unwrap().children[0];
        assert_eq!(scene.main_of(nested), Some(star));
        assert_eq!(scene.node(nested).unwrap().children.len(), 1);
    }
}
