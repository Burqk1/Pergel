//! Layer subtrees as plain data: used for copy/paste, duplicate and (later) saving documents.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use serde::{Deserialize, Serialize};

use crate::geom::{Color, Rect};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::layout::{AutoLayout, Sizing};
use crate::style::{Blend, Effect, ExportSetting, StrokeAlign};
use crate::paint::{Images, Paint, VectorPath};
use crate::text::Text;

const FORMAT: &str = "pergel/layers";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Tree {
    pub name: String,
    pub kind: Kind,
    pub bounds: Rect,
    #[serde(default)]
    pub rotation: f32,
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub corners: Option<[f32; 4]>,
    #[serde(default = "yes")]
    pub fill_on: bool,
    #[serde(default)]
    pub stroke_align: StrokeAlign,
    #[serde(default = "one")]
    pub opacity: f32,
    #[serde(default)]
    pub blend: Blend,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub effects: Vec<Effect>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<ExportSetting>,
    #[serde(default)]
    pub lock_ratio: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<AutoLayout>,
    #[serde(default)]
    pub sizing: [Sizing; 2],
    #[serde(default)]
    pub absolute: bool,
    /// For a main component, its own id; for an instance, the id of its main (same document).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<NodeId>,
    #[serde(default)]
    pub is_main: bool,
    pub fill: Color,
    #[serde(default)]
    pub stroke: Option<(Color, f32)>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub clip: bool,
    #[serde(default)]
    pub children: Vec<Tree>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<Text>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<Paint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vector: Option<VectorPath>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub mask: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bindings: Option<crate::variables::Bindings>,
    #[serde(default, skip_serializing_if = "is_default_constraints")]
    pub constraints: [crate::layout::Constraint; 2],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub styles: Option<crate::styles::StyleLinks>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub variant_set: bool,
    /// Prototype interactions; targets are layer ids of the same document.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reactions: Vec<crate::prototype::Reaction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<crate::code_connect::CodeLink>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grids: Vec<crate::style::LayoutGrid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dash: Option<[f32; 2]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub ready: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub section: bool,
    #[serde(default, skip_serializing_if = "crate::prototype::Scroll::is_none")]
    pub scroll: crate::prototype::Scroll,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sides: Option<[f32; 4]>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub slice: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub props: Vec<crate::component_props::ComponentProp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop_refs: Option<crate::component_props::PropRefs>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_fills: Vec<crate::paint::FillLayer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub comments: Vec<crate::comments::CommentThread>,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub prop_values: std::collections::BTreeMap<String, crate::component_props::PropValue>,
    /// The layer's id when saved, so interactions can point at it after loading.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_id: Option<NodeId>,
    /// A stable id for live collaboration (top-level layers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub library: Option<crate::comments::LibraryRef>,
}

fn is_default_constraints(c: &[crate::layout::Constraint; 2]) -> bool {
    *c == [crate::layout::Constraint::Min; 2]
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

#[derive(Serialize, Deserialize)]
struct Payload {
    format: String,
    version: u32,
    layers: Vec<Tree>,
    /// Image fills the layers use, so they paste into other documents intact.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    images: BTreeMap<String, String>,
}

/// The images `trees` use, base64 encoded, keyed by hash.
pub fn embed_images<'a>(trees: impl IntoIterator<Item = &'a Tree>, store: &Images) -> BTreeMap<String, String> {
    fn walk(t: &Tree, used: &mut BTreeSet<String>) {
        if let Some(Paint::Image(img)) = &t.paint {
            used.insert(img.hash.clone());
        }
        for c in &t.children {
            walk(c, used);
        }
    }
    let mut used = BTreeSet::new();
    for t in trees {
        walk(t, &mut used);
    }
    used.into_iter()
        .filter_map(|h| store.get(&h).map(|b| (h, B64.encode(b.as_slice()))))
        .collect()
}

/// Decodes embedded images into `store`.
pub fn decode_images(map: &BTreeMap<String, String>, store: &mut Images) {
    for (hash, data) in map {
        if store.get(hash).is_none()
            && let Ok(bytes) = B64.decode(data)
        {
            store.insert(hash.clone(), Arc::new(bytes));
        }
    }
}

impl Tree {
    pub fn aabb(&self) -> Rect {
        self.bounds.rotated_aabb(self.rotation)
    }

    /// Scales the tree about the world origin (strokes and radii by the smaller factor).
    pub fn scale(&mut self, sx: f32, sy: f32) {
        let k = sx.abs().min(sy.abs());
        if let Some(s) = self.sides.as_mut() {
            s.iter_mut().for_each(|w| *w *= k);
        }
        self.bounds = Rect::new(self.bounds.x * sx, self.bounds.y * sy, self.bounds.w * sx, self.bounds.h * sy);
        if let Some((_, w)) = self.stroke.as_mut() {
            *w *= k;
        }
        self.radius *= k;
        if let Some(c) = self.corners.as_mut() {
            c.iter_mut().for_each(|r| *r *= k);
        }
        for c in &mut self.children {
            c.scale(sx, sy);
        }
    }

    /// Recolours artwork (icons): every visible fill and stroke becomes `color`, keeping alpha.
    pub fn recolor(&mut self, color: Color) {
        let keep = |c: Color| Color((c.argb() & 0xff00_0000) | (color.argb() & 0x00ff_ffff));
        if self.kind == Kind::Vector || !self.kind.is_container() {
            if self.fill_on && !matches!(self.paint, Some(Paint::Image(_))) {
                self.fill = keep(self.fill);
                self.paint = None;
            }
            if let Some((c, _)) = self.stroke.as_mut() {
                *c = keep(*c);
            }
        }
        for c in &mut self.children {
            c.recolor(color);
        }
    }

    pub fn translate(&mut self, dx: f32, dy: f32) {
        self.bounds = self.bounds.translated(dx, dy);
        for c in &mut self.children {
            c.translate(dx, dy);
        }
    }
}

pub fn bounds_of(trees: &[Tree]) -> Option<Rect> {
    Rect::union_all(trees.iter().map(Tree::aabb))
}

pub fn to_text(trees: Vec<Tree>, store: &Images) -> String {
    let images = embed_images(&trees, store);
    serde_json::to_string(&Payload { format: FORMAT.into(), version: 1, layers: trees, images }).unwrap_or_default()
}

/// The layers in clipboard text; their images are added to `store`.
pub fn from_text(text: &str, store: &mut Images) -> Option<Vec<Tree>> {
    let payload: Payload = serde_json::from_str(text).ok()?;
    if payload.format != FORMAT || payload.layers.is_empty() {
        return None;
    }
    decode_images(&payload.images, store);
    // Pasted copies don't take the original's comments (as in Figma).
    fn strip(t: &mut Tree) {
        t.comments.clear();
        t.uid = None;
        t.children.iter_mut().for_each(strip);
    }
    let mut layers = payload.layers;
    layers.iter_mut().for_each(strip);
    Some(layers)
}

impl Scene {
    pub fn extract(&self, id: NodeId) -> Option<Tree> {
        let n = self.node(id)?;
        Some(Tree {
            name: n.name.clone(),
            kind: n.kind,
            bounds: n.bounds,
            rotation: n.rotation,
            radius: n.radius,
            corners: n.corners,
            fill_on: n.fill_on,
            stroke_align: n.stroke_align,
            opacity: n.opacity,
            blend: n.blend,
            effects: n.effects.clone(),
            exports: n.exports.clone(),
            lock_ratio: n.lock_ratio,
            layout: n.layout.as_deref().cloned(),
            sizing: n.sizing,
            absolute: n.absolute,
            main: match n.component {
                Some(crate::Component::Main) => Some(n.id),
                Some(crate::Component::Instance(m)) => Some(m),
                None => None,
            },
            is_main: n.component == Some(crate::Component::Main),
            fill: n.fill,
            stroke: n.stroke,
            visible: n.visible,
            locked: n.locked,
            clip: n.clip,
            children: n.children.iter().filter_map(|c| self.extract(*c)).collect(),
            text: n.text.as_deref().cloned(),
            paint: n.paint.as_deref().cloned(),
            vector: n.vector.as_deref().cloned(),
            mask: n.mask,
            bindings: n.bindings.as_deref().cloned(),
            constraints: n.constraints,
            styles: n.styles.as_deref().cloned(),
            variant_set: n.variant_set,
            reactions: n.reactions.clone(),
            code: n.code.as_deref().cloned(),
            grids: n.grids.clone(),
            dash: n.dash,
            note: n.note.as_deref().cloned(),
            ready: n.ready,
            section: n.section,
            scroll: n.scroll,
            sides: n.sides,
            slice: n.slice,
            props: n.props.clone(),
            prop_refs: n.prop_refs.as_deref().cloned(),
            prop_values: n.prop_values.clone(),
            extra_fills: n.extra_fills.clone(),
            comments: n.comments.clone(),
            uid: n.uid.clone(),
            library: n.library.as_deref().cloned(),
            saved_id: Some(n.id),
        })
    }

    /// Builds the tree under `parent` at `index` and returns the new root's id.
    pub fn insert_tree(&mut self, tree: &Tree, parent: Option<NodeId>, index: Option<usize>) -> NodeId {
        let mut node = node_from_tree(tree);
        if tree.kind == Kind::Group && tree.children.is_empty() {
            node.kind = Kind::Frame;
        }
        let id = self.insert(node, parent, index);
        for child in &tree.children {
            self.insert_tree(child, Some(id), None);
        }
        id
    }
}

/// A detached node with the tree's own properties (no children, not yet in a scene).
pub(crate) fn node_from_tree(tree: &Tree) -> Node {
    let mut node = Node::new(tree.kind, tree.name.clone(), tree.bounds, tree.fill).radius(tree.radius);
    node.stroke = tree.stroke;
    node.rotation = tree.rotation;
    node.corners = tree.corners;
    node.fill_on = tree.fill_on;
    node.stroke_align = tree.stroke_align;
    node.opacity = tree.opacity;
    node.blend = tree.blend;
    node.lock_ratio = tree.lock_ratio;
    node.effects = tree.effects.clone();
    node.exports = tree.exports.clone();
    node.layout = tree.layout.clone().map(Box::new);
    node.sizing = tree.sizing;
    node.absolute = tree.absolute;
    node.visible = tree.visible;
    node.locked = tree.locked;
    node.clip = tree.clip;
    node.text = tree.text.clone().map(Box::new);
    node.paint = tree.paint.clone().map(Box::new);
    node.vector = tree.vector.clone().map(Box::new);
    node.mask = tree.mask;
    node.bindings = tree.bindings.clone().map(Box::new);
    node.constraints = tree.constraints;
    node.styles = tree.styles.clone().map(Box::new);
    node.variant_set = tree.variant_set;
    node.reactions = tree.reactions.clone();
    node.code = tree.code.clone().map(Box::new);
    node.grids = tree.grids.clone();
    node.dash = tree.dash;
    node.note = tree.note.clone().map(Box::new);
    node.ready = tree.ready;
    node.section = tree.section;
    node.scroll = tree.scroll;
    node.sides = tree.sides;
    node.slice = tree.slice;
    node.props = tree.props.clone();
    node.prop_refs = tree.prop_refs.clone().filter(|r| !r.is_empty()).map(Box::new);
    node.prop_values = tree.prop_values.clone();
    node.extra_fills = tree.extra_fills.clone();
    node.comments = tree.comments.clone();
    node.uid = tree.uid.clone();
    node.library = tree.library.clone().map(Box::new);
    if node.kind == Kind::Text && node.text.is_none() {
        node.text = Some(Box::default());
    }
    node
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};

    #[test]
    fn subtrees_round_trip_through_text() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.insert(rect("A", 10.0, 10.0, 10.0, 10.0).radius(4.0), Some(f), None);
        scene.insert(rect("B", 30.0, 10.0, 10.0, 10.0), Some(f), None);
        scene.group(&[a], Kind::Group);

        let text = to_text(vec![scene.extract(f).unwrap()], &scene.images);
        let mut trees = from_text(&text, &mut Images::default()).unwrap();
        trees[0].translate(200.0, 0.0);
        let copy = scene.insert_tree(&trees[0], None, None);

        let c = scene.node(copy).unwrap();
        assert_eq!(c.bounds.x, 200.0);
        assert_eq!(c.children.len(), 2);
        let group = scene.node(c.children[0]).unwrap();
        assert_eq!(group.kind, Kind::Group);
        let inner = scene.node(group.children[0]).unwrap();
        assert_eq!((inner.name.as_str(), inner.radius, inner.bounds.x), ("A", 4.0, 210.0));
        assert!(from_text("{\"format\":\"other\",\"version\":1,\"layers\":[]}", &mut Images::default()).is_none());
        assert!(from_text("hello", &mut Images::default()).is_none());
    }
}
