//! Figma's variants: a component set is a frame of main components named
//! "Property=Value, Other=Value". Instances switch between them by property value.

use crate::components::Component;
use crate::geom::{Color, Rect};
use crate::scene::{Kind, NodeId, Scene};

/// "Size=Large, State=Hover" → [("Size", "Large"), ("State", "Hover")].
pub fn parse_variant(name: &str) -> Vec<(String, String)> {
    name.split(',')
        .filter_map(|part| {
            let (k, v) = part.split_once('=')?;
            let (k, v) = (k.trim(), v.trim());
            (!k.is_empty()).then(|| (k.to_string(), v.to_string()))
        })
        .collect()
}

pub fn variant_name(props: &[(String, String)]) -> String {
    props.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(", ")
}

/// Purple outline of component sets, as in Figma.
const SET_STROKE: Color = Color::rgba(151, 71, 255, 255);

impl Scene {
    /// The component set holding `main`, if any.
    pub fn variant_set_of(&self, main: NodeId) -> Option<NodeId> {
        let p = self.node(main)?.parent?;
        self.node(p).filter(|n| n.variant_set).map(|n| n.id)
    }

    /// The set's properties and their values, in first-seen order.
    pub fn variant_props(&self, set: NodeId) -> Vec<(String, Vec<String>)> {
        let mut out: Vec<(String, Vec<String>)> = Vec::new();
        for c in self.children(Some(set)) {
            let Some(n) = self.node(*c).filter(|n| n.component == Some(Component::Main)) else { continue };
            for (k, v) in parse_variant(&n.name) {
                match out.iter_mut().find(|(key, _)| *key == k) {
                    Some((_, values)) => {
                        if !values.contains(&v) {
                            values.push(v);
                        }
                    }
                    None => out.push((k, vec![v])),
                }
            }
        }
        out
    }

    /// The variant matching `props` exactly, else the one sharing the most values.
    pub fn find_variant(&self, set: NodeId, props: &[(String, String)]) -> Option<NodeId> {
        let mains: Vec<(NodeId, Vec<(String, String)>)> = self
            .children(Some(set))
            .iter()
            .filter_map(|c| self.node(*c).filter(|n| n.component == Some(Component::Main)).map(|n| (n.id, parse_variant(&n.name))))
            .collect();
        let score = |p: &[(String, String)]| props.iter().filter(|kv| p.contains(kv)).count();
        mains.iter().max_by_key(|(_, p)| (score(p), usize::MAX - p.len())).map(|(id, _)| *id)
    }

    /// "Combine as variants": the main components go into a component set (a frame with a
    /// purple outline and 16 px padding); names without properties become "Property 1=…".
    pub fn combine_as_variants(&mut self, mains: &[NodeId]) -> Option<NodeId> {
        let ids: Vec<NodeId> = self.outermost(mains).into_iter().filter(|id| self.is_main(*id)).collect();
        if ids.is_empty() {
            return None;
        }
        let names: Vec<String> = ids.iter().filter_map(|id| self.node(*id).map(|n| n.name.clone())).collect();
        // "Button/Primary", "Button/Secondary" → set "Button", values "Primary", "Secondary".
        let prefix = names[0].rsplit_once('/').map(|(p, _)| p.to_string()).filter(|p| names.iter().all(|n| n.starts_with(&format!("{p}/"))));
        let set_name = prefix.clone().unwrap_or_else(|| if names.len() == 1 { names[0].clone() } else { "Component".to_string() });
        for (i, id) in ids.iter().enumerate() {
            let name = names[i].clone();
            if parse_variant(&name).is_empty() {
                let value = match &prefix {
                    Some(p) => name[p.len() + 1..].to_string(),
                    None if names.len() == 1 => "Default".to_string(),
                    None => name.clone(),
                };
                self.update(*id, |n| n.name = format!("Property 1={value}"));
            }
        }
        let set = self.group(&ids, Kind::Frame)?;
        let b = self.node(set)?.bounds;
        let pad = 16.0;
        self.set_bounds_raw(set, Rect::new(b.x - pad, b.y - pad, b.w + 2.0 * pad, b.h + 2.0 * pad));
        self.update(set, |n| {
            n.name = set_name;
            n.variant_set = true;
            n.fill_on = false;
            n.stroke = Some((SET_STROKE, 1.0));
            n.dash = Some([4.0, 4.0]);
            n.radius = 5.0;
            n.clip = false;
        });
        Some(set)
    }

    /// Switches an instance to another main (a different variant), keeping its place, size
    /// overrides and the text the user typed into it. Returns the new instance.
    pub fn swap_instance(&mut self, inst: NodeId, main: NodeId) -> Option<NodeId> {
        let old = self.node(inst)?.clone();
        if !self.is_main(main) || self.main_of(inst) == Some(main) {
            return Some(inst);
        }
        let (parent, index) = self.position(inst)?;
        // Text overrides by layer path (names from the instance root).
        let mut texts: Vec<(Vec<String>, String)> = Vec::new();
        for id in self.subtree(inst) {
            let Some(n) = self.node(id) else { continue };
            if n.overrides & crate::overrides::TEXT != 0
                && let Some(t) = n.text.as_deref()
            {
                texts.push((self.source_path(inst, id), t.content.clone()));
            }
        }
        let (cx, cy) = old.bounds.center();
        let new = self.create_instance(main, parent, Some(index + 1), (cx, cy))?;
        // Same top-left corner as the old instance.
        let nb = self.node(new)?.bounds;
        self.translate(new, old.bounds.x - nb.x, old.bounds.y - nb.y);
        if old.overrides & crate::overrides::SIZE != 0 {
            let b = self.node(new)?.bounds;
            self.set_bounds(new, Rect::new(b.x, b.y, old.bounds.w, old.bounds.h));
        }
        for id in self.subtree(new) {
            let path = self.name_path(new, id);
            if let Some((_, content)) = texts.iter().find(|(p, _)| *p == path) {
                let content = content.clone();
                self.update(id, |n| {
                    if let Some(t) = n.text.as_mut() {
                        t.content = content;
                    }
                });
                self.nodes[id as usize].overrides |= crate::overrides::TEXT;
            }
        }
        // Keep a name the user gave the instance; default names follow the new variant.
        let old_main = self.main_of(inst).and_then(|m| self.node(m)).map(|m| m.name.clone());
        let set_name = self.variant_set_of(main).and_then(|s| self.node(s)).map(|s| s.name.clone());
        let renamed = Some(&old.name) != old_main.as_ref() && Some(&old.name) != set_name.as_ref();
        self.update(new, |n| {
            if renamed {
                n.name = old.name.clone();
            }
            n.sizing = old.sizing;
            n.absolute = old.absolute;
            n.constraints = old.constraints;
            // Property values carry over to the other variant (same set, same properties).
            n.prop_values = old.prop_values.clone();
        });
        self.remove(inst);
        Some(new)
    }

    /// Like `name_path`, but with the names of the main's layers the instance mirrors (a text
    /// layer named after what was typed into it still matches its counterpart).
    fn source_path(&self, root: NodeId, id: NodeId) -> Vec<String> {
        let mut path = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            if i == root {
                break;
            }
            let Some(n) = self.node(i) else { break };
            let name = n.source.and_then(|s| self.node(s)).map_or(n.name.clone(), |s| s.name.clone());
            path.push(name);
            cur = n.parent;
        }
        path.reverse();
        path
    }

    /// Layer names from `root` (exclusive) down to `id`.
    fn name_path(&self, root: NodeId, id: NodeId) -> Vec<String> {
        let mut path = Vec::new();
        let mut cur = Some(id);
        while let Some(i) = cur {
            if i == root {
                break;
            }
            let Some(n) = self.node(i) else { break };
            path.push(n.name.clone());
            cur = n.parent;
        }
        path.reverse();
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};
    use crate::{Node, Text};

    #[test]
    fn instances_switch_variants_and_keep_their_text() {
        let mut scene = Scene::default();
        let mut mains = Vec::new();
        for (i, (name, color)) in [("Button/Primary", 0xff7c3aed), ("Button/Secondary", 0xffe5e7eb)].iter().enumerate() {
            let f = scene.add(frame(name, 0.0, i as f32 * 60.0, 120.0, 40.0));
            scene.update(f, |n| {
                n.fill = Color(*color);
                n.component = Some(Component::Main);
            });
            scene.insert(Node::text(Text::new("Label", 14.0, 600), 10.0, i as f32 * 60.0 + 10.0, Color(0xff000000)), Some(f), None);
            mains.push(f);
        }
        let set = scene.combine_as_variants(&mains).unwrap();
        assert_eq!(scene.node(set).unwrap().name, "Button");
        assert_eq!(scene.node(mains[1]).unwrap().name, "Property 1=Secondary");
        assert_eq!(scene.variant_props(set), vec![("Property 1".to_string(), vec!["Primary".to_string(), "Secondary".to_string()])]);
        assert_eq!(scene.node(mains[0]).unwrap().bounds.x, 0.0, "mains stay put");

        let inst = scene.create_instance(mains[0], None, None, (500.0, 500.0)).unwrap();
        let label = scene.node(inst).unwrap().children[0];
        scene.update(label, |n| n.text.as_mut().unwrap().content = "Buy now".into());
        scene.sync_components(&[label]);
        let target = scene.find_variant(set, &[("Property 1".into(), "Secondary".into())]).unwrap();
        assert_eq!(target, mains[1]);
        let swapped = scene.swap_instance(inst, target).unwrap();
        let n = scene.node(swapped).unwrap();
        assert_eq!(n.fill, Color(0xffe5e7eb));
        assert_eq!(scene.main_of(swapped), Some(mains[1]));
        let text = scene.node(n.children[0]).unwrap().text.as_ref().unwrap().content.clone();
        assert_eq!(text, "Buy now", "typed text survives the switch");
        let _ = rect("unused", 0.0, 0.0, 1.0, 1.0);
    }

    #[test]
    fn variant_names_parse() {
        assert_eq!(parse_variant("Size=Large, State = Hover"), vec![("Size".into(), "Large".into()), ("State".into(), "Hover".into())]);
        assert!(parse_variant("Button").is_empty());
        assert_eq!(variant_name(&parse_variant("A=1,B=2")), "A=1, B=2");
    }
}
