//! Components and instances. An instance is a copy of its main component in which every layer
//! remembers its counterpart (`source`). Edits to the main flow into every instance, except the
//! properties an instance has overridden. As in Figma, layers inside an instance cannot be
//! moved, resized, reordered or deleted; only their properties change.

use serde::{Deserialize, Serialize};

use crate::geom::{Rect, normalize_degrees, rotate_point};
use crate::scene::{Kind, Node, NodeId, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "role", content = "main", rename_all = "lowercase")]
pub enum Component {
    Main,
    Instance(NodeId),
}

/// Property groups an instance layer has overridden.
pub mod overrides {
    pub const FILL: u16 = 1;
    pub const STROKE: u16 = 1 << 1;
    pub const TEXT: u16 = 1 << 2;
    pub const TEXT_STYLE: u16 = 1 << 3;
    pub const VISIBLE: u16 = 1 << 4;
    pub const NAME: u16 = 1 << 5;
    pub const OPACITY: u16 = 1 << 6;
    pub const RADIUS: u16 = 1 << 7;
    pub const SIZE: u16 = 1 << 8;
    pub const LAYOUT: u16 = 1 << 9;
    pub const EFFECTS: u16 = 1 << 10;
    /// A nested instance swapped to another component by an instance-swap property.
    pub const SWAP: u16 = 1 << 11;
}

use overrides::*;

/// Which overridable properties differ between an instance layer and its source.
fn differences(i: &Node, m: &Node, root: bool) -> u16 {
    let mut d = 0;
    if i.fill != m.fill || i.fill_on != m.fill_on || i.paint != m.paint || i.extra_fills != m.extra_fills {
        d |= FILL;
    }
    if i.stroke != m.stroke || i.stroke_align != m.stroke_align {
        d |= STROKE;
    }
    match (i.text.as_deref(), m.text.as_deref()) {
        (Some(a), Some(b)) => {
            if a.content != b.content {
                d |= TEXT;
            }
            let strip = |t: &crate::Text| crate::Text { content: String::new(), auto_name: false, ..t.clone() };
            if strip(a) != strip(b) {
                d |= TEXT_STYLE;
            }
        }
        (None, None) => {}
        _ => d |= TEXT | TEXT_STYLE,
    }
    if i.visible != m.visible {
        d |= VISIBLE;
    }
    if i.name != m.name && !root && i.text.as_ref().is_none_or(|t| !t.auto_name) {
        d |= NAME;
    }
    if i.opacity != m.opacity || i.blend != m.blend {
        d |= OPACITY;
    }
    if i.radius != m.radius || i.corners != m.corners {
        d |= RADIUS;
    }
    if i.layout != m.layout {
        d |= LAYOUT;
    }
    if i.effects != m.effects {
        d |= EFFECTS;
    }
    if root && (i.bounds.w != m.bounds.w || i.bounds.h != m.bounds.h) {
        d |= SIZE;
    }
    d
}

impl Scene {
    pub fn is_main(&self, id: NodeId) -> bool {
        self.node(id).is_some_and(|n| n.component == Some(Component::Main))
    }

    pub fn is_instance(&self, id: NodeId) -> bool {
        self.node(id).is_some_and(|n| matches!(n.component, Some(Component::Instance(_))))
    }

    /// The main component an instance follows, if it still exists.
    pub fn main_of(&self, id: NodeId) -> Option<NodeId> {
        match self.node(id)?.component? {
            Component::Instance(main) => self.node(main).map(|n| n.id),
            Component::Main => None,
        }
    }

    /// Whether a strict ancestor is an instance: the layer belongs to that instance.
    pub fn in_instance(&self, id: NodeId) -> bool {
        let mut cur = self.node(id).and_then(|n| n.parent);
        while let Some(p) = cur {
            if self.is_instance(p) {
                return true;
            }
            cur = self.nodes[p as usize].parent;
        }
        false
    }

    /// Whether the layer is a component, an instance, or inside one (drawn in purple).
    pub fn in_component(&self, id: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(n) = self.node(i) else { return false };
            if n.component.is_some() {
                return true;
            }
            cur = n.parent;
        }
        false
    }

    fn enclosing_main(&self, id: NodeId) -> Option<NodeId> {
        let mut cur = Some(id);
        while let Some(i) = cur {
            let n = self.node(i)?;
            if n.component == Some(Component::Main) {
                return Some(i);
            }
            cur = n.parent;
        }
        None
    }

    pub fn mains(&self) -> Vec<NodeId> {
        self.nodes().filter(|n| n.component == Some(Component::Main)).map(|n| n.id).collect()
    }

    pub fn instances_of(&self, main: NodeId) -> Vec<NodeId> {
        self.nodes().filter(|n| n.component == Some(Component::Instance(main))).map(|n| n.id).collect()
    }

    pub(crate) fn link_subtree(&mut self, copy: NodeId, original: NodeId) -> bool {
        self.link(copy, original)
    }

    /// Links a copied subtree to the subtree it was copied from, layer by layer.
    fn link(&mut self, copy: NodeId, original: NodeId) -> bool {
        let (a, b) = (self.subtree(copy), self.subtree(original));
        if a.len() != b.len() {
            return false;
        }
        for (c, o) in a.into_iter().zip(b) {
            self.touch(c);
            let n = &mut self.nodes[c as usize];
            n.source = Some(o);
            n.overrides = 0;
        }
        true
    }

    /// A new instance of `main` centred at (x, y) under `parent`.
    pub fn create_instance(&mut self, main: NodeId, parent: Option<NodeId>, index: Option<usize>, at: (f32, f32)) -> Option<NodeId> {
        let tree = self.extract(main)?;
        let id = self.insert_tree(&tree, parent, index);
        self.link(id, main);
        let b = self.node(id)?.bounds;
        // Instances of a variant are named after their component set ("Button").
        let set_name = self.variant_set_of(main).and_then(|s| self.node(s)).map(|s| s.name.clone());
        self.update(id, |n| {
            n.component = Some(Component::Instance(main));
            // Definitions stay on the main; the instance keeps only values.
            n.props.clear();
            // Only the main is the library's component.
            n.library = None;
            n.uid = None;
            if let Some(name) = set_name {
                n.name = name;
            }
        });
        self.translate(id, at.0 - b.center().0, at.1 - b.center().1);
        Some(id)
    }

    /// Makes a pasted copy of an instance follow `main` again; falls back to `false` when the
    /// copy no longer matches the main's structure.
    pub fn relink_instance(&mut self, id: NodeId, main: NodeId) -> bool {
        if self.node(main).is_none() || !self.link(id, main) {
            return false;
        }
        self.update(id, |n| n.component = Some(Component::Instance(main)));
        let pairs: Vec<NodeId> = self.subtree(id);
        for c in pairs {
            let (Some(i), Some(m)) = (self.node(c), self.node(c).and_then(|n| n.source).and_then(|s| self.node(s))) else {
                continue;
            };
            let d = differences(i, m, c == id);
            self.nodes[c as usize].overrides = d;
        }
        true
    }

    /// Turns an instance into a plain frame (Ctrl+Alt+B). Nested instances stay instances.
    pub fn detach(&mut self, id: NodeId) -> bool {
        if !self.is_instance(id) {
            return false;
        }
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            let Some(n) = self.node(i) else { continue };
            let nested = i != id && n.component.is_some();
            let kids = n.children.clone();
            self.touch(i);
            let n = &mut self.nodes[i as usize];
            if !nested {
                n.source = None;
                n.overrides = 0;
                if i == id {
                    n.component = None;
                }
                stack.extend(kids);
            }
        }
        self.changed_structure();
        true
    }

    /// Drops an instance's overrides so it matches its main again.
    pub fn reset_instance(&mut self, id: NodeId) {
        for i in self.subtree(id) {
            if self.nodes[i as usize].overrides != 0 {
                self.touch(i);
                self.nodes[i as usize].overrides = 0;
            }
        }
        self.sync_instance(id);
    }

    /// After an edit: records overrides on touched instance layers, then pushes changes of
    /// every touched main component into its instances (repeating for nested components).
    pub fn sync_components(&mut self, touched: &[NodeId]) {
        for id in touched {
            let Some(n) = self.node(*id) else { continue };
            let Some(src) = n.source.and_then(|s| self.node(s)) else { continue };
            let root = n.component.is_some_and(|c| matches!(c, Component::Instance(_)));
            let d = differences(n, src, root);
            if d & !n.overrides != 0 {
                self.touch(*id);
                self.nodes[*id as usize].overrides |= d;
            }
        }
        let mut dirty: Vec<NodeId> = touched.iter().filter_map(|id| self.enclosing_main(*id)).collect();
        for _ in 0..8 {
            dirty.sort_unstable();
            dirty.dedup();
            if dirty.is_empty() {
                break;
            }
            let mut next = Vec::new();
            for main in std::mem::take(&mut dirty) {
                for inst in self.instances_of(main) {
                    if self.sync_instance(inst) {
                        next.extend(self.enclosing_main(inst));
                    }
                }
            }
            dirty = next;
        }
    }

    /// Brings one instance up to date with its main. Returns whether anything changed.
    pub fn sync_instance(&mut self, inst: NodeId) -> bool {
        let Some(main) = self.main_of(inst) else { return false };
        let before = self.generation();
        let (Some(m), Some(i)) = (self.node(main).cloned(), self.node(inst).cloned()) else { return false };
        // Size follows the main unless the instance was resized.
        if i.overrides & SIZE == 0 && (i.bounds.w, i.bounds.h) != (m.bounds.w, m.bounds.h) {
            self.set_bounds(inst, Rect::new(i.bounds.x, i.bounds.y, m.bounds.w, m.bounds.h));
        }
        self.copy_props(inst, &m, true);
        self.apply_swaps(inst);
        let frame = self.node(inst).map(|n| (n.bounds, n.rotation)).unwrap_or((i.bounds, i.rotation));
        self.sync_children(inst, main, (m.bounds, m.rotation), frame);
        self.apply_values(inst);
        self.generation() != before
    }

    fn copy_props(&mut self, id: NodeId, m: &Node, root: bool) {
        let Some(o) = self.node(id).map(|n| n.overrides) else { return };
        self.update(id, |n| {
            if !root && o & NAME == 0 {
                n.name = m.name.clone();
            }
            if o & FILL == 0 {
                n.fill = m.fill;
                n.fill_on = m.fill_on;
                n.paint = m.paint.clone();
                n.extra_fills = m.extra_fills.clone();
            }
            if o & STROKE == 0 {
                n.stroke = m.stroke;
                n.stroke_align = m.stroke_align;
                n.dash = m.dash;
            }
            if o & OPACITY == 0 {
                n.opacity = m.opacity;
                n.blend = m.blend;
            }
            if o & RADIUS == 0 {
                n.radius = m.radius;
                n.corners = m.corners;
            }
            if o & VISIBLE == 0 {
                n.visible = m.visible;
            }
            if o & LAYOUT == 0 {
                n.layout = m.layout.clone();
            }
            if o & EFFECTS == 0 {
                n.effects = m.effects.clone();
            }
            // Instances keep their own comments; a main's comments stay on the main.
            n.clip = m.clip;
            n.vector = m.vector.clone();
            n.mask = m.mask;
            n.grids = m.grids.clone();
            n.reactions = m.reactions.clone();
            n.constraints = m.constraints;
            if o & TEXT_STYLE == 0 && o & FILL == 0 {
                n.styles = m.styles.clone();
            }
            if o & FILL == 0 {
                n.bindings = m.bindings.clone();
            }
            n.sizing = m.sizing;
            n.absolute = m.absolute;
            n.lock_ratio = m.lock_ratio;
            if !root {
                n.kind = m.kind;
                if let Some(Component::Instance(nested)) = m.component
                    && o & SWAP == 0
                {
                    n.component = Some(Component::Instance(nested));
                }
            }
            match (n.text.as_mut(), m.text.as_deref()) {
                (Some(t), Some(src)) => {
                    let content = if o & TEXT == 0 { src.content.clone() } else { std::mem::take(&mut t.content) };
                    if o & TEXT_STYLE == 0 {
                        **t = crate::Text { content, ..src.clone() };
                    } else {
                        t.content = content;
                    }
                }
                (None, Some(src)) => n.text = Some(Box::new(src.clone())),
                _ => {}
            }
            if o & NAME == 0 && !root && n.text.as_ref().is_some_and(|t| t.auto_name) {
                n.name = n.text.as_ref().map(|t| t.layer_name()).unwrap_or_default();
            }
        });
    }

    /// Mirrors the main's children (structure, order, geometry and properties) under `inst`.
    fn sync_children(&mut self, inst: NodeId, main: NodeId, mframe: (Rect, f32), iframe: (Rect, f32)) {
        let main_kids: Vec<NodeId> = self.node(main).map(|n| n.children.clone()).unwrap_or_default();
        let inst_kids: Vec<NodeId> = self.node(inst).map(|n| n.children.clone()).unwrap_or_default();
        let mut ordered = Vec::with_capacity(main_kids.len());
        for mk in &main_kids {
            let existing = inst_kids.iter().copied().find(|c| self.node(*c).and_then(|n| n.source) == Some(*mk));
            let id = match existing {
                Some(id) => id,
                None => {
                    let Some(tree) = self.extract(*mk) else { continue };
                    let id = self.insert_tree(&tree, Some(inst), None);
                    self.link(id, *mk);
                    id
                }
            };
            ordered.push(id);
        }
        for c in inst_kids {
            if !ordered.contains(&c) {
                self.remove(c);
            }
        }
        for (k, id) in ordered.iter().enumerate() {
            if self.position(*id).map(|p| p.1) != Some(k) {
                self.reparent(*id, Some(inst), Some(k));
            }
        }
        for (id, mk) in ordered.into_iter().zip(main_kids) {
            let Some(m) = self.node(mk).cloned() else { continue };
            if self.node(id).is_some_and(|n| n.overrides & SWAP != 0) {
                // A swapped slot: placed like the main's layer, sized and filled by its own component.
                let size = self.main_of(id).and_then(|t| self.node(t)).map(|t| (t.bounds.w, t.bounds.h));
                let mut placed = m.clone();
                if let Some((w, h)) = size {
                    let c = m.bounds.center();
                    placed.bounds = Rect::new(c.0 - w / 2.0, c.1 - h / 2.0, w, h);
                }
                self.place_like(id, &placed, mframe, iframe);
                self.sync_instance(id);
                continue;
            }
            self.place_like(id, &m, mframe, iframe);
            self.copy_props(id, &m, false);
            if let Some(frame) = self.node(id).map(|n| (n.bounds, n.rotation)) {
                self.sync_children(id, mk, (m.bounds, m.rotation), frame);
            }
            if self.node(id).is_some_and(|n| n.kind == Kind::Group) {
                self.refit_inside(&[id]);
            }
        }
    }

    /// Puts an instance layer where its main counterpart sits relative to the main's top-left,
    /// carried over to the instance's frame (which may be moved or turned).
    fn place_like(&mut self, id: NodeId, m: &Node, mframe: (Rect, f32), iframe: (Rect, f32)) {
        let (mb, mr) = mframe;
        let (ib, ir) = iframe;
        let (mcx, mcy) = mb.center();
        let (lx, ly) = rotate_point(m.bounds.center().0, m.bounds.center().1, mcx, mcy, -mr);
        let (ox, oy) = (lx - mb.x, ly - mb.y);
        let (icx, icy) = ib.center();
        let (wx, wy) = rotate_point(ib.x + ox, ib.y + oy, icx, icy, ir);
        let target = Rect::new(0.0, 0.0, m.bounds.w, m.bounds.h).centered(wx, wy);
        let rotation = normalize_degrees(m.rotation - mr + ir);
        let Some(n) = self.node(id) else { return };
        if n.kind == Kind::Group {
            let c = n.aabb().center();
            if (c.0 - wx).abs() > 1e-4 || (c.1 - wy).abs() > 1e-4 {
                self.translate(id, wx - c.0, wy - c.1);
            }
            return;
        }
        if n.bounds != target || n.rotation != rotation {
            self.set_geometry(id, target, rotation);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Color;
    use crate::scene::tests::{frame, rect};

    fn button(scene: &mut Scene) -> (NodeId, NodeId) {
        let main = scene.add(frame("Button", 0.0, 0.0, 100.0, 40.0));
        let bg = scene.insert(rect("Bg", 0.0, 0.0, 100.0, 40.0), Some(main), None);
        scene.update(main, |n| n.component = Some(Component::Main));
        (main, bg)
    }

    #[test]
    fn instances_follow_the_main_but_keep_overrides() {
        let mut scene = Scene::default();
        let (main, bg) = button(&mut scene);
        let a = scene.create_instance(main, None, None, (300.0, 20.0)).unwrap();
        let b = scene.create_instance(main, None, None, (300.0, 120.0)).unwrap();
        let a_bg = scene.node(a).unwrap().children[0];
        let b_bg = scene.node(b).unwrap().children[0];
        assert_eq!(scene.node(a_bg).unwrap().bounds, Rect::new(250.0, 0.0, 100.0, 40.0));

        // Override the fill in instance A.
        scene.update(a_bg, |n| n.fill = Color(0xffff0000));
        scene.sync_components(&[a_bg]);
        // Change the main: fill and a new child.
        scene.update(bg, |n| {
            n.fill = Color(0xff00ff00);
            n.radius = 8.0;
        });
        let dot = scene.insert(rect("Dot", 10.0, 10.0, 5.0, 5.0), Some(main), None);
        scene.sync_components(&[bg, dot]);

        let a_bg_node = scene.node(a_bg).unwrap();
        assert_eq!(a_bg_node.fill, Color(0xffff0000), "override kept");
        assert_eq!(a_bg_node.radius, 8.0, "other properties follow");
        assert_eq!(scene.node(b_bg).unwrap().fill, Color(0xff00ff00));
        let b_dot = scene.node(b).unwrap().children[1];
        assert_eq!(scene.node(b_dot).unwrap().bounds, Rect::new(260.0, 110.0, 5.0, 5.0));
        assert_eq!(scene.node(b_dot).unwrap().source, Some(dot));

        scene.remove(dot);
        scene.sync_components(&[main]);
        assert_eq!(scene.node(b).unwrap().children.len(), 1);

        scene.reset_instance(a);
        assert_eq!(scene.node(a_bg).unwrap().fill, Color(0xff00ff00));
        assert!(scene.detach(a));
        assert!(!scene.is_instance(a));
        assert_eq!(scene.node(a_bg).unwrap().source, None);
        assert!(scene.in_instance(b_bg) && !scene.in_instance(a_bg));
    }

    #[test]
    fn resized_and_turned_instances_keep_their_own_frame() {
        let mut scene = Scene::default();
        let (main, bg) = button(&mut scene);
        let a = scene.create_instance(main, None, None, (500.0, 500.0)).unwrap();
        scene.set_bounds(a, Rect::new(450.0, 480.0, 200.0, 40.0));
        scene.sync_components(&[a]);
        scene.update(main, |n| n.bounds.h = 50.0);
        scene.sync_components(&[main]);
        assert_eq!(scene.node(a).unwrap().bounds.w, 200.0, "the resized width stays");
        let (cx, cy) = scene.node(a).unwrap().bounds.center();
        scene.rotate(a, 90.0, cx, cy);
        scene.update(bg, |n| n.bounds.w = 50.0);
        scene.sync_components(&[bg]);
        let child = scene.node(scene.node(a).unwrap().children[0]).unwrap();
        assert_eq!(child.rotation, 90.0);
        assert_eq!((child.bounds.w, child.bounds.h), (50.0, 40.0));
    }
}
