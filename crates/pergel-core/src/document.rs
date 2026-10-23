//! The `.pergel` file format: pages of layer trees as JSON. Layer ids are not stored; component
//! links are rebuilt on load (instances are re-linked to their main by structure).

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::clipboard::Tree;
use crate::components::Component;
use crate::paint::Images;
use crate::styles::Styles;
use crate::variables::Variables;
use crate::scene::{NodeId, Scene};

pub const FORMAT: &str = "pergel/document";
pub const VERSION: u32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Page {
    pub name: String,
    pub layers: Vec<Tree>,
    /// Viewport and other per-page editor state; opaque to the core.
    #[serde(default, skip_serializing_if = "Value::is_null")]
    pub view: Value,
    /// Ruler guides.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub guides: Vec<Guide>,
}

/// A ruler guide across the whole page: vertical at `x = at`, or horizontal at `y = at`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Guide {
    pub vertical: bool,
    pub at: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub format: String,
    pub version: u32,
    pub pages: Vec<Page>,
    /// Image fills' files, base64 encoded, keyed by content hash.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Variables::is_empty")]
    pub variables: Variables,
    #[serde(default, skip_serializing_if = "Styles::is_empty")]
    pub styles: Styles,
    /// Comment threads on empty canvas (layer threads are saved in their layers).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<crate::comments::PageComment>,
    /// A branch of another file: where it came from and what it started as.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<Branch>,
}

/// Figma-style branching: the main file and the document when the branch was made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Branch {
    /// The main file's id and name.
    pub of: String,
    #[serde(default)]
    pub of_name: String,
    /// The main document when the branch was made (`.pergel` JSON).
    pub base: String,
    /// Merged into the main file (milliseconds), if it was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged: Option<f64>,
}

impl Scene {
    /// The page as a list of top-level layer trees.
    pub fn to_page(&self, name: &str, view: Value) -> Page {
        Page { name: name.to_string(), layers: self.roots().iter().filter_map(|id| self.extract(*id)).collect(), view, guides: Vec::new() }
    }

    /// Rebuilds a scene from saved trees, restoring main components and re-linking instances.
    pub fn from_trees(trees: &[Tree]) -> Scene {
        let mut scene = Scene::default();
        let mut mains: HashMap<NodeId, NodeId> = HashMap::new();
        let mut instances: Vec<(NodeId, NodeId)> = Vec::new();
        let mut ids: HashMap<NodeId, NodeId> = HashMap::new();
        for tree in trees {
            scene.insert_tracked(tree, None, &mut mains, &mut instances, &mut ids);
        }
        // Prototype links point at saved ids; map them to the new layers.
        let linked: Vec<NodeId> = scene.nodes().filter(|n| !n.reactions.is_empty()).map(|n| n.id).collect();
        for id in linked {
            scene.update(id, |n| {
                for r in &mut n.reactions {
                    r.action.targets_mut(&mut |to| {
                        if let Some(new) = ids.get(to) {
                            *to = *new;
                        }
                    });
                }
            });
        }
        // Instance-swap values point at saved main ids too.
        let swapping: Vec<NodeId> = scene.nodes().filter(|n| !n.props.is_empty() || !n.prop_values.is_empty()).map(|n| n.id).collect();
        for id in swapping {
            scene.update(id, |n| {
                let remap = |v: &mut crate::component_props::PropValue| {
                    if let crate::component_props::PropValue::Swap(m) = v
                        && let Some(new) = ids.get(m)
                    {
                        *m = *new;
                    }
                };
                for p in &mut n.props {
                    remap(&mut p.default);
                }
                for v in n.prop_values.values_mut() {
                    remap(v);
                }
            });
        }
        for new in mains.values() {
            scene.update(*new, |n| n.component = Some(Component::Main));
        }
        // Inner instances first, so outer ones link against an already linked structure.
        for (new, old_main) in instances.into_iter().rev() {
            if let Some(main) = mains.get(&old_main) {
                scene.relink_instance(new, *main);
            }
        }
        scene
    }

    fn insert_tracked(
        &mut self,
        tree: &Tree,
        parent: Option<NodeId>,
        mains: &mut HashMap<NodeId, NodeId>,
        instances: &mut Vec<(NodeId, NodeId)>,
        ids: &mut HashMap<NodeId, NodeId>,
    ) -> NodeId {
        // Groups are briefly empty here; unlike pasting, they must stay groups.
        let id = self.insert(crate::clipboard::node_from_tree(tree), parent, None);
        if let Some(old) = tree.saved_id {
            ids.insert(old, id);
        }
        match (tree.main, tree.is_main) {
            (Some(old), true) => {
                mains.insert(old, id);
            }
            (Some(main), false) => instances.push((id, main)),
            _ => {}
        }
        for child in &tree.children {
            self.insert_tracked(child, Some(id), mains, instances, ids);
        }
        id
    }
}

impl Document {
    pub fn new(pages: Vec<Page>) -> Self {
        Self { format: FORMAT.into(), version: VERSION, pages, images: BTreeMap::new(), variables: Variables::default(), styles: Styles::default(), comments: Vec::new(), branch: None }
    }

    /// Embeds the images the pages use (and only those) from `store`.
    pub fn embed_images(&mut self, store: &Images) {
        self.images = crate::clipboard::embed_images(self.pages.iter().flat_map(|p| p.layers.iter()), store);
    }

    /// The embedded images, decoded.
    pub fn image_store(&self) -> Images {
        let mut store = Images::default();
        crate::clipboard::decode_images(&self.images, &mut store);
        store
    }

    pub fn parse(text: &str) -> Result<Document, String> {
        let doc: Document = serde_json::from_str(text).map_err(|e| format!("Not a Pergel file: {e}"))?;
        if doc.format != FORMAT {
            return Err(format!("Not a Pergel file (format {:?})", doc.format));
        }
        if doc.version > VERSION {
            return Err(format!("This file was saved by a newer Pergel (version {})", doc.version));
        }
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};
    use crate::{Color, Kind, Rect};

    #[test]
    fn documents_round_trip_with_component_links() {
        let mut scene = Scene::default();
        let main = scene.add(frame("Button", 0.0, 0.0, 100.0, 40.0));
        let bg = scene.insert(rect("Bg", 0.0, 0.0, 100.0, 40.0), Some(main), None);
        scene.update(main, |n| n.component = Some(Component::Main));
        let inst = scene.create_instance(main, None, None, (300.0, 20.0)).unwrap();
        let inst_bg = scene.node(inst).unwrap().children[0];
        scene.update(inst_bg, |n| n.fill = Color(0xffff0000));
        scene.sync_components(&[inst_bg]);
        let card = scene.add(frame("Card", 0.0, 100.0, 200.0, 100.0));
        let inner = scene.insert(rect("Inner", 10.0, 110.0, 20.0, 20.0), Some(card), None);
        let other = scene.insert(rect("Other", 40.0, 110.0, 20.0, 20.0), Some(card), None);
        scene.group(&[inner, other], Kind::Group);
        let _ = bg;

        let text = serde_json::to_string(&Document::new(vec![scene.to_page("Page 1", Value::Null)])).unwrap();
        let doc = Document::parse(&text).unwrap();
        let mut loaded = Scene::from_trees(&doc.pages[0].layers);

        assert_eq!(loaded.roots().len(), 3);
        let mains = loaded.mains();
        assert_eq!(mains.len(), 1);
        let instances = loaded.instances_of(mains[0]);
        assert_eq!(instances.len(), 1);
        let li = instances[0];
        let li_bg = loaded.node(li).unwrap().children[0];
        assert_eq!(loaded.node(li_bg).unwrap().fill, Color(0xffff0000), "override survived");
        // The link works: editing the main still reaches the instance.
        let main_bg = loaded.node(mains[0]).unwrap().children[0];
        loaded.update(main_bg, |n| n.radius = 9.0);
        loaded.sync_components(&[main_bg]);
        assert_eq!(loaded.node(li_bg).unwrap().radius, 9.0);
        let card = loaded.roots()[2];
        assert_eq!(loaded.node(card).unwrap().kind, Kind::Frame);
        let group = loaded.node(loaded.node(card).unwrap().children[0]).unwrap();
        assert_eq!(group.kind, Kind::Group, "groups stay groups");
        assert_eq!(group.bounds, Rect::new(10.0, 110.0, 50.0, 20.0));
        assert!(Document::parse("{\"format\":\"x\",\"version\":1,\"pages\":[]}").is_err());
    }
}
