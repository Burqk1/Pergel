use std::cell::{Cell, Ref, RefCell};
use std::sync::atomic::{AtomicU64, Ordering};

use rstar::{AABB, RTree, RTreeObject};
use serde::{Deserialize, Serialize};

use crate::geom::{Color, Rect, normalize_degrees, rotate_point};
use crate::style::{Blend, Effect, ExportSetting, StrokeAlign};
use crate::components::Component;
use crate::history::History;
use crate::layout::{AutoLayout, Constraint, Sizing};
use crate::paint::{Images, Paint, VectorPath};
use crate::text::Text;

pub type NodeId = u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Frame,
    Group,
    Rectangle,
    Ellipse,
    Text,
    /// A path (icons, illustrations, imported SVG); its outline is in `Node::vector`.
    Vector,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Frame => "Frame",
            Kind::Group => "Group",
            Kind::Rectangle => "Rectangle",
            Kind::Ellipse => "Ellipse",
            Kind::Text => "Text",
            Kind::Vector => "Vector",
        }
    }

    pub fn is_container(self) -> bool {
        matches!(self, Kind::Frame | Kind::Group)
    }
}

/// Bounds are absolute world coordinates of the unrotated box; `rotation` turns it about its
/// centre. Moving or rotating a container moves every descendant with it.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub id: NodeId,
    pub name: String,
    pub kind: Kind,
    pub bounds: Rect,
    /// Degrees, counter-clockwise positive (Figma's convention).
    pub rotation: f32,
    pub radius: f32,
    /// Independent corner radii (top-left, top-right, bottom-right, bottom-left).
    pub corners: Option<[f32; 4]>,
    pub fill: Color,
    pub fill_on: bool,
    pub stroke: Option<(Color, f32)>,
    pub stroke_align: StrokeAlign,
    /// Layer opacity, 0–1, separate from the fill colour's alpha.
    pub opacity: f32,
    pub blend: Blend,
    pub effects: Vec<Effect>,
    pub exports: Vec<ExportSetting>,
    pub lock_ratio: bool,
    /// Auto layout of a frame's children (Figma's Shift+A).
    pub layout: Option<Box<AutoLayout>>,
    /// Horizontal and vertical resizing: fixed, hug contents, or fill the auto-layout parent.
    pub sizing: [Sizing; 2],
    /// Ignores the parent's auto layout and keeps its own position.
    pub absolute: bool,
    /// Main component or instance role.
    pub component: Option<Component>,
    /// In an instance: the layer of the main component this one mirrors.
    pub source: Option<NodeId>,
    /// In an instance: property groups changed away from the main (see `components::overrides`).
    pub overrides: u16,
    pub visible: bool,
    pub locked: bool,
    pub clip: bool,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    pub text: Option<Box<Text>>,
    /// A gradient or image replacing the solid fill.
    pub paint: Option<Box<Paint>>,
    /// The outline of a vector layer.
    pub vector: Option<Box<VectorPath>>,
    /// "Use as mask": not drawn itself; clips the layers above it in the same parent.
    pub mask: bool,
    /// Properties driven by variables.
    pub bindings: Option<Box<crate::variables::Bindings>>,
    /// Horizontal and vertical constraints inside a frame.
    pub constraints: [Constraint; 2],
    /// Linked text, colour and effect styles.
    pub styles: Option<Box<crate::styles::StyleLinks>>,
    /// A component set (Figma's variants container).
    pub variant_set: bool,
    /// Prototype interactions.
    pub reactions: Vec<crate::prototype::Reaction>,
    /// Code Connect: the codebase component this main component (or set) maps to.
    pub code: Option<Box<crate::code_connect::CodeLink>>,
    /// Layout guides (frames).
    pub grids: Vec<crate::style::LayoutGrid>,
    /// Dashed stroke: dash and gap lengths (Figma's "Dashes").
    pub dash: Option<[f32; 2]>,
    /// Designer's note for developers and agents (Figma's annotations).
    pub note: Option<Box<String>>,
    /// Marked "Ready for dev" (Dev Mode status).
    pub ready: bool,
    /// A section (Shift+S): a frame that organises the canvas — titled, never clips.
    pub section: bool,
    /// Prototype overflow scrolling of a clipping frame.
    pub scroll: crate::prototype::Scroll,
    /// Individual stroke weights: top, right, bottom, left (rectangles and frames).
    pub sides: Option<[f32; 4]>,
    /// A slice (S): an export area, not drawn.
    pub slice: bool,
    /// Main components (and component sets): their properties.
    pub props: Vec<crate::component_props::ComponentProp>,
    /// Layers inside a main component: the properties they show.
    pub prop_refs: Option<Box<crate::component_props::PropRefs>>,
    /// Fills above the layer's own fill, bottom to top.
    pub extra_fills: Vec<crate::paint::FillLayer>,
    /// Comment threads pinned to this layer.
    pub comments: Vec<crate::comments::CommentThread>,
    /// Stable id of a top-level layer for live collaboration.
    pub uid: Option<String>,
    /// A main component brought in from a team library.
    pub library: Option<Box<crate::comments::LibraryRef>>,
    /// Instances: property values that differ from the defaults.
    pub prop_values: std::collections::BTreeMap<String, crate::component_props::PropValue>,
    pub deleted: bool,
}

impl Node {
    pub fn new(kind: Kind, name: impl Into<String>, bounds: Rect, fill: Color) -> Self {
        Self {
            id: 0,
            name: name.into(),
            kind,
            bounds,
            rotation: 0.0,
            radius: 0.0,
            corners: None,
            fill,
            fill_on: true,
            stroke: None,
            stroke_align: StrokeAlign::Inside,
            opacity: 1.0,
            blend: if kind.is_container() { Blend::PassThrough } else { Blend::Normal },
            effects: Vec::new(),
            exports: Vec::new(),
            lock_ratio: false,
            layout: None,
            sizing: [Sizing::Fixed; 2],
            absolute: false,
            component: None,
            source: None,
            overrides: 0,
            visible: true,
            locked: false,
            clip: kind == Kind::Frame,
            parent: None,
            children: Vec::new(),
            text: None,
            paint: None,
            vector: None,
            mask: false,
            bindings: None,
            constraints: [Constraint::Min; 2],
            styles: None,
            variant_set: false,
            reactions: Vec::new(),
            code: None,
            grids: Vec::new(),
            dash: None,
            note: None,
            ready: false,
            section: false,
            scroll: crate::prototype::Scroll::None,
            sides: None,
            slice: false,
            props: Vec::new(),
            prop_refs: None,
            prop_values: std::collections::BTreeMap::new(),
            extra_fills: Vec::new(),
            comments: Vec::new(),
            uid: None,
            library: None,
            deleted: false,
        }
    }

    /// A text layer; its size is set by the renderer once the text is laid out.
    pub fn text(text: Text, x: f32, y: f32, fill: Color) -> Self {
        let mut node = Self::new(Kind::Text, text.layer_name(), Rect::new(x, y, 0.0, 0.0), fill);
        node.text = Some(Box::new(text));
        node
    }

    pub fn radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn stroke(mut self, color: Color, width: f32) -> Self {
        self.stroke = Some((color, width));
        self
    }

    /// The axis-aligned box around the (possibly rotated) layer.
    pub fn aabb(&self) -> Rect {
        self.bounds.rotated_aabb(self.rotation)
    }

    /// Where the layer can paint, shadows and blurs included.
    pub fn paint_bounds(&self) -> Rect {
        let out = self.effects.iter().map(|e| e.outset()).fold(0.0f32, f32::max);
        let b = self.aabb();
        if out > 0.0 { Rect::new(b.x - out, b.y - out, b.w + 2.0 * out, b.h + 2.0 * out) } else { b }
    }

    /// Corner radii as drawn: the independent corners or the uniform radius.
    pub fn radii(&self) -> [f32; 4] {
        self.corners.unwrap_or([self.radius; 4])
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        let b = self.bounds;
        let (x, y) = if self.rotation == 0.0 {
            (x, y)
        } else {
            let (cx, cy) = b.center();
            rotate_point(x, y, cx, cy, -self.rotation)
        };
        if !b.contains_point(x, y) {
            return false;
        }
        match self.kind {
            Kind::Ellipse => {
                let (rx, ry) = (b.w / 2.0, b.h / 2.0);
                if rx <= 0.0 || ry <= 0.0 {
                    return false;
                }
                let (dx, dy) = ((x - b.x - rx) / rx, (y - b.y - ry) / ry);
                dx * dx + dy * dy <= 1.0
            }
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Order {
    Forward,
    Backward,
    Front,
    Back,
}

/// One clipping ancestor: a frame's box, corner radii and rotation — or a mask layer, whose
/// outline the renderer clips to (`rect` is then its box, for hit testing).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClipShape {
    pub rect: Rect,
    pub radii: [f32; 4],
    pub rotation: f32,
    pub mask: Option<NodeId>,
}

impl ClipShape {
    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        self.rect.contains_rotated(x, y, self.rotation)
    }

    /// Whether the clip cannot cut into `node` at all (conservative near rounded corners).
    pub fn encloses(&self, node: &Node) -> bool {
        if self.mask.is_some() {
            return false;
        }
        let r = self.radii.iter().fold(0.0f32, |a, b| a.max(*b));
        let c = self.rect;
        let wide = Rect::new(c.x + r, c.y, c.w - 2.0 * r, c.h);
        let tall = Rect::new(c.x, c.y + r, c.w, c.h - 2.0 * r);
        let (cx, cy) = c.center();
        node.bounds.corners(node.rotation).iter().all(|(x, y)| {
            let (lx, ly) = rotate_point(*x, *y, cx, cy, -self.rotation);
            wide.contains_point(lx, ly) || tall.contains_point(lx, ly)
        })
    }
}

/// The clipping ancestors a node is drawn under, outermost first.
pub type Clip = Vec<ClipShape>;

#[derive(Debug, Clone, Copy, PartialEq)]
struct Entry {
    index: usize,
    envelope: AABB<[f32; 2]>,
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f32; 2]>;

    fn envelope(&self) -> Self::Envelope {
        self.envelope
    }
}

fn entry(n: &Node) -> Entry {
    Entry { index: n.id as usize, envelope: envelope(&n.paint_bounds()) }
}

fn envelope(r: &Rect) -> AABB<[f32; 2]> {
    AABB::from_corners([r.x, r.y], [r.right(), r.bottom()])
}

fn extent(r: &Rect) -> f32 {
    r.w.max(r.h)
}

static GENERATION: AtomicU64 = AtomicU64::new(1);

fn next_generation() -> u64 {
    GENERATION.fetch_add(1, Ordering::Relaxed)
}

#[derive(Default)]
pub struct Scene {
    pub(crate) nodes: Vec<Node>,
    pub(crate) roots: Vec<NodeId>,
    index: RTree<Entry>,
    generation: u64,
    max_extent: f32,
    ranks: RefCell<Vec<u32>>,
    ranks_dirty: Cell<bool>,
    pub(crate) history: History,
    /// Encoded image files referenced by image fills.
    pub images: Images,
}

impl Scene {
    /// A flat document: every node is top-level, painted in the given order.
    pub fn new(mut nodes: Vec<Node>) -> Self {
        for (i, n) in nodes.iter_mut().enumerate() {
            n.id = i as NodeId;
            n.parent = None;
            n.children.clear();
        }
        let entries = nodes.iter().map(entry).collect();
        let max_extent = nodes.iter().map(|n| extent(&n.aabb())).fold(0.0, f32::max);
        let roots = (0..nodes.len() as NodeId).collect();
        let scene = Self {
            nodes,
            roots,
            index: RTree::bulk_load(entries),
            generation: next_generation(),
            max_extent,
            ..Default::default()
        };
        scene.ranks_dirty.set(true);
        scene
    }

    pub fn len(&self) -> usize {
        self.nodes.iter().filter(|n| !n.deleted).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().filter(|n| !n.deleted)
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize).filter(|n| !n.deleted)
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn max_extent(&self) -> f32 {
        self.max_extent
    }

    pub fn bounds(&self) -> Rect {
        Rect::union_all(self.roots.iter().map(|id| self.nodes[*id as usize].aabb())).unwrap_or_default()
    }

    pub fn children(&self, parent: Option<NodeId>) -> &[NodeId] {
        match parent {
            Some(p) => self.node(p).map_or(&[], |n| n.children.as_slice()),
            None => &self.roots,
        }
    }

    /// Depth-first paint order (0 = drawn first). Deleted nodes rank `u32::MAX`.
    fn ranks(&self) -> Ref<'_, Vec<u32>> {
        if self.ranks_dirty.get() {
            let mut ranks = self.ranks.borrow_mut();
            ranks.clear();
            ranks.resize(self.nodes.len(), u32::MAX);
            let mut next = 0;
            let mut stack: Vec<NodeId> = self.roots.iter().rev().copied().collect();
            while let Some(id) = stack.pop() {
                ranks[id as usize] = next;
                next += 1;
                stack.extend(self.nodes[id as usize].children.iter().rev());
            }
            self.ranks_dirty.set(false);
        }
        self.ranks.borrow()
    }

    pub fn rank(&self, id: NodeId) -> u32 {
        self.ranks().get(id as usize).copied().unwrap_or(u32::MAX)
    }

    pub fn sort_by_rank(&self, ids: &mut [NodeId]) {
        let ranks = self.ranks();
        ids.sort_unstable_by_key(|id| ranks.get(*id as usize).copied().unwrap_or(u32::MAX));
    }

    /// Nodes whose bounds touch `area`, in paint order.
    pub fn visible<'a>(&'a self, area: &Rect) -> impl Iterator<Item = &'a Node> + 'a {
        let mut hits: Vec<NodeId> =
            self.index.locate_in_envelope_intersecting(envelope(area)).map(|e| e.index as NodeId).collect();
        self.sort_by_rank(&mut hits);
        hits.into_iter().map(move |i| &self.nodes[i as usize]).filter(|n| !n.deleted)
    }

    /// `None` when the node or an ancestor is hidden or it is clipped away entirely; otherwise the
    /// clipping frames it is drawn under.
    pub fn reach(&self, id: NodeId) -> Option<Clip> {
        let node = self.node(id)?;
        if !node.visible {
            return None;
        }
        let aabb = node.aabb();
        let mut clip: Clip = Vec::new();
        self.push_mask(id, &mut clip);
        let mut parent = node.parent;
        while let Some(p) = parent {
            let n = &self.nodes[p as usize];
            if !n.visible {
                return None;
            }
            if n.clip && n.kind == Kind::Frame {
                if !n.aabb().intersects(&aabb) && aabb.w > 0.0 && aabb.h > 0.0 {
                    return None;
                }
                clip.push(ClipShape { rect: n.bounds, radii: n.radii(), rotation: n.rotation, mask: None });
            }
            self.push_mask(p, &mut clip);
            parent = n.parent;
        }
        clip.reverse();
        Some(clip)
    }

    /// The nearest visible mask below `id` among its siblings clips it.
    fn push_mask(&self, id: NodeId, clip: &mut Clip) {
        let Some(parent) = self.nodes[id as usize].parent else {
            return;
        };
        let siblings = &self.nodes[parent as usize].children;
        let Some(i) = siblings.iter().position(|s| *s == id) else { return };
        if let Some(m) = siblings[..i].iter().rev().map(|s| &self.nodes[*s as usize]).find(|s| s.mask && s.visible && !s.deleted) {
            clip.push(ClipShape { rect: m.bounds, radii: m.radii(), rotation: m.rotation, mask: Some(m.id) });
        }
    }

    /// Product of the layer's and its ancestors' opacity.
    pub fn opacity(&self, id: NodeId) -> f32 {
        let mut o = 1.0;
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(n) = self.node(i) else { break };
            o *= n.opacity;
            cur = n.parent;
        }
        o
    }

    pub fn is_locked(&self, id: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(n) = self.node(i) else { return true };
            if n.locked {
                return true;
            }
            cur = n.parent;
        }
        false
    }

    pub fn path(&self, id: NodeId) -> Vec<NodeId> {
        let mut path = vec![id];
        let mut cur = self.node(id).and_then(|n| n.parent);
        while let Some(p) = cur {
            path.push(p);
            cur = self.nodes[p as usize].parent;
        }
        path.reverse();
        path
    }

    pub fn is_ancestor(&self, ancestor: NodeId, id: NodeId) -> bool {
        let mut cur = self.node(id).and_then(|n| n.parent);
        while let Some(p) = cur {
            if p == ancestor {
                return true;
            }
            cur = self.nodes[p as usize].parent;
        }
        false
    }

    /// Scale tool (K): scales a layer and everything inside it by `k` about (px, py) — sizes
    /// together with strokes, radii, dashes, text sizes, effects and auto-layout spacing.
    pub fn scale_about(&mut self, id: NodeId, k: f32, px: f32, py: f32) {
        use crate::style::Effect;
        for i in self.subtree(id) {
            let Some(b) = self.node(i).map(|n| n.bounds) else { continue };
            self.update(i, |n| {
                if let Some((_, w)) = n.stroke.as_mut() {
                    *w *= k;
                }
                if let Some(s) = n.sides.as_mut() {
                    s.iter_mut().for_each(|w| *w *= k);
                }
                n.radius *= k;
                if let Some(c) = n.corners.as_mut() {
                    c.iter_mut().for_each(|r| *r *= k);
                }
                if let Some(d) = n.dash.as_mut() {
                    d.iter_mut().for_each(|v| *v *= k);
                }
                for e in &mut n.effects {
                    match e {
                        Effect::DropShadow { x, y, blur, spread, .. } | Effect::InnerShadow { x, y, blur, spread, .. } => {
                            for v in [x, y, blur, spread] {
                                *v *= k;
                            }
                        }
                        Effect::LayerBlur { radius, .. } | Effect::BackgroundBlur { radius, .. } => *radius *= k,
                    }
                }
                if let Some(l) = n.layout.as_mut() {
                    l.gap *= k;
                    l.counter_gap *= k;
                    l.padding.iter_mut().for_each(|v| *v *= k);
                }
                if let Some(t) = n.text.as_mut() {
                    t.size *= k;
                    if let crate::text::LineHeight::Px(v) = &mut t.line_height {
                        *v *= k;
                    }
                    for r in &mut t.runs {
                        if let Some(s) = r.size.as_mut() {
                            *s *= k;
                        }
                    }
                }
            });
            self.set_bounds_raw(i, Rect::new(px + (b.x - px) * k, py + (b.y - py) * k, b.w * k, b.h * k));
        }
        self.changed(false);
    }

    /// Live collaboration: gives every top-level layer a unique `uid` (copies get new ones).
    /// Not an edit: it doesn't touch the undo history. Returns whether any changed.
    pub fn assign_uids(&mut self, mut fresh: impl FnMut() -> String) -> bool {
        let mut seen = std::collections::HashSet::new();
        let mut changed = false;
        for id in self.roots.clone() {
            let n = &mut self.nodes[id as usize];
            let ok = n.uid.as_ref().is_some_and(|u| seen.insert(u.clone()));
            if !ok {
                let u = fresh();
                seen.insert(u.clone());
                n.uid = Some(u);
                changed = true;
            }
        }
        changed
    }

    /// A stable `uid` for one layer (made with `fresh` when it has none). Not an undoable edit.
    pub fn ensure_uid(&mut self, id: NodeId, fresh: impl FnOnce() -> String) -> String {
        let n = &mut self.nodes[id as usize];
        n.uid.get_or_insert_with(fresh).clone()
    }

    pub fn subtree(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack = vec![id];
        while let Some(i) = stack.pop() {
            if let Some(n) = self.node(i) {
                out.push(i);
                stack.extend(n.children.iter().rev());
            }
        }
        out
    }

    /// Live ids without any that sit inside another listed id, in paint order.
    pub fn outermost(&self, ids: &[NodeId]) -> Vec<NodeId> {
        let mut out: Vec<NodeId> = ids
            .iter()
            .copied()
            .filter(|id| self.node(*id).is_some() && !ids.iter().any(|a| a != id && self.is_ancestor(*a, *id)))
            .collect();
        out.sort_unstable();
        out.dedup();
        self.sort_by_rank(&mut out);
        out
    }

    /// The topmost painted node under a world point, ignoring hidden, clipped-away and locked layers.
    pub fn hit(&self, x: f32, y: f32) -> Option<NodeId> {
        let point = Rect::new(x, y, 0.0, 0.0);
        let hits: Vec<&Node> = self.visible(&point).collect();
        hits.into_iter()
            .rev()
            .filter(|n| n.kind != Kind::Group && n.contains(x, y) && !self.is_locked(n.id))
            .find(|n| self.reach(n.id).is_some_and(|clip| clip.iter().all(|c| c.contains_point(x, y))))
            .map(|n| n.id)
    }

    /// What a click selects, following Figma: the direct child of a top-level frame, a whole
    /// group, a sibling of the current selection, or an already selected ancestor (so it can be
    /// dragged). `deep` selects the innermost layer. `None` over a top-level frame's empty
    /// background means the click should start a marquee.
    pub fn pick(&self, x: f32, y: f32, deep: bool, context: &[NodeId]) -> Option<NodeId> {
        let leaf = self.hit(x, y)?;
        if deep {
            return Some(leaf);
        }
        let path = self.path(leaf);
        let top = &self.nodes[path[0] as usize];
        let top_frame = top.kind == Kind::Frame && !matches!(top.component, Some(Component::Instance(_)));
        if let Some(i) = path.iter().rposition(|id| context.contains(id))
            && !(i == 0 && top_frame && path.len() > 1)
        {
            return Some(path[i]);
        }
        for i in (0..path.len() - 1).rev() {
            if context.iter().any(|c| self.node(*c).and_then(|n| n.parent) == Some(path[i])) {
                return Some(path[i + 1]);
            }
        }
        if !top_frame {
            Some(path[0])
        } else if path.len() > 1 {
            Some(path[1])
        } else if self.nodes[path[0] as usize].children.is_empty() {
            Some(path[0])
        } else {
            None
        }
    }

    /// Double-click: one level deeper than the selected layer under the point.
    pub fn dive(&self, x: f32, y: f32, context: &[NodeId]) -> Option<NodeId> {
        let leaf = self.hit(x, y)?;
        let path = self.path(leaf);
        match path.iter().rposition(|id| context.contains(id)) {
            Some(i) if i + 1 < path.len() => Some(path[i + 1]),
            Some(i) => Some(path[i]),
            None => self.pick(x, y, false, context),
        }
    }

    /// The top-level frame whose background a marquee starting here should select inside.
    pub fn marquee_scope(&self, x: f32, y: f32) -> Option<NodeId> {
        let id = self.hit(x, y)?;
        let n = &self.nodes[id as usize];
        (n.parent.is_none() && n.kind == Kind::Frame && !n.children.is_empty()).then_some(id)
    }

    /// Marquee selection among the children of `scope` (the canvas when `None`). Top-level
    /// frames must be fully enclosed; everything else only has to touch the area.
    pub fn select_area(&self, area: &Rect, scope: Option<NodeId>) -> Vec<NodeId> {
        self.visible(area)
            .filter(|n| n.parent == scope && self.reach(n.id).is_some() && !self.is_locked(n.id))
            .filter(|n| if scope.is_none() && n.kind == Kind::Frame { area.contains(&n.aabb()) } else { true })
            .map(|n| n.id)
            .collect()
    }

    /// The innermost visible frame under a point that `moving` could be dropped into.
    pub fn drop_target(&self, x: f32, y: f32, moving: &[NodeId]) -> Option<NodeId> {
        let point = Rect::new(x, y, 0.0, 0.0);
        let hits: Vec<&Node> = self.visible(&point).collect();
        hits.into_iter()
            .rev()
            .filter(|n| n.kind == Kind::Frame && n.contains(x, y) && self.reach(n.id).is_some())
            .filter(|n| !self.is_instance(n.id) && !self.in_instance(n.id))
            .find(|n| !moving.iter().any(|m| *m == n.id || self.is_ancestor(*m, n.id)))
            .map(|n| n.id)
    }

    /// The box drawn around a selection: a single layer's own (rotated) box, or the
    /// axis-aligned box around several layers. Returns the unrotated rect and its rotation.
    pub fn selection_box(&self, ids: &[NodeId]) -> Option<(Rect, f32)> {
        let nodes: Vec<&Node> = ids.iter().filter_map(|id| self.node(*id)).collect();
        match nodes.as_slice() {
            [] => None,
            [one] => Some((one.bounds, one.rotation)),
            many => Rect::union_all(many.iter().map(|n| n.aabb())).map(|r| (r, 0.0)),
        }
    }

    pub fn next_name(&self, kind: Kind) -> String {
        self.next_name_for(kind.label())
    }

    /// "Prefix N" with N one past the highest number already used.
    pub fn next_name_for(&self, prefix: &str) -> String {
        let n = self
            .nodes
            .iter()
            .filter_map(|n| n.name.strip_prefix(prefix)?.trim().parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix} {}", n + 1)
    }

    // ----- mutations; each one records the before-state it overwrites for undo -----

    pub(crate) fn changed_structure(&mut self) {
        self.changed(true);
    }

    /// Sets a layer's box and rotation without touching its children.
    pub(crate) fn set_geometry(&mut self, id: NodeId, bounds: Rect, rotation: f32) {
        self.set_geometry_raw(id, bounds, rotation);
        self.changed(false);
    }

    fn changed(&mut self, structure: bool) {
        self.generation = next_generation();
        if structure {
            self.ranks_dirty.set(true);
        }
    }

    fn list_mut(&mut self, parent: Option<NodeId>) -> &mut Vec<NodeId> {
        match parent {
            Some(p) => {
                self.touch(p);
                &mut self.nodes[p as usize].children
            }
            None => {
                self.touch_roots();
                &mut self.roots
            }
        }
    }

    pub(crate) fn set_bounds_raw(&mut self, id: NodeId, bounds: Rect) {
        let rotation = self.nodes[id as usize].rotation;
        self.set_geometry_raw(id, bounds, rotation);
    }

    fn set_geometry_raw(&mut self, id: NodeId, bounds: Rect, rotation: f32) {
        let bounds = Rect::new(bounds.x, bounds.y, bounds.w.max(0.0), bounds.h.max(0.0));
        let node = &self.nodes[id as usize];
        if node.bounds == bounds && node.rotation == rotation {
            return;
        }
        self.touch(id);
        self.index.remove(&entry(&self.nodes[id as usize]));
        let node = &mut self.nodes[id as usize];
        node.bounds = bounds;
        node.rotation = rotation;
        self.max_extent = self.max_extent.max(extent(&node.aabb()));
        self.index.insert(entry(node));
    }

    /// Replaces a node wholesale (undo/redo), keeping the spatial index in sync.
    pub(crate) fn put(&mut self, node: Node) {
        let id = node.id as usize;
        if id >= self.nodes.len() {
            return;
        }
        if !self.nodes[id].deleted {
            self.index.remove(&entry(&self.nodes[id]));
        }
        if !node.deleted {
            self.index.insert(entry(&node));
            self.max_extent = self.max_extent.max(extent(&node.aabb()));
        }
        self.nodes[id] = node;
    }

    pub(crate) fn put_roots(&mut self, roots: Vec<NodeId>) {
        self.roots = roots;
    }

    pub(crate) fn restored(&mut self) {
        self.changed(true);
    }

    /// Fits groups along the parent chain to their children; removes groups left empty.
    fn refit(&mut self, mut cur: Option<NodeId>) {
        while let Some(p) = cur {
            let Some(node) = self.node(p) else { return };
            if node.kind != Kind::Group {
                return;
            }
            let parent = node.parent;
            match Rect::union_all(node.children.iter().map(|c| self.nodes[*c as usize].aabb())) {
                Some(b) => self.set_bounds_raw(p, b),
                None => {
                    self.remove(p);
                    return;
                }
            }
            cur = parent;
        }
    }

    pub fn insert(&mut self, mut node: Node, parent: Option<NodeId>, index: Option<usize>) -> NodeId {
        let parent = parent.filter(|p| self.node(*p).is_some_and(|n| n.kind.is_container()));
        let id = self.nodes.len() as NodeId;
        node.id = id;
        node.parent = parent;
        node.children.clear();
        node.deleted = false;
        self.record_new(&node);
        self.max_extent = self.max_extent.max(extent(&node.aabb()));
        self.index.insert(entry(&node));
        self.nodes.push(node);
        let list = self.list_mut(parent);
        let at = index.unwrap_or(list.len()).min(list.len());
        list.insert(at, id);
        self.changed(true);
        self.refit(parent);
        id
    }

    /// Adds a top-level node above everything else.
    pub fn add(&mut self, node: Node) -> NodeId {
        self.insert(node, None, None)
    }

    /// Changes a node's own properties. Bounds changes go through [`Scene::set_bounds`].
    pub fn update(&mut self, id: NodeId, f: impl FnOnce(&mut Node)) -> bool {
        let Some(before) = self.node(id).cloned() else { return false };
        let mut after = before.clone();
        f(&mut after);
        let (bounds, rotation) = (after.bounds, after.rotation);
        after.id = before.id;
        after.bounds = before.bounds;
        after.rotation = before.rotation;
        after.parent = before.parent;
        after.children = before.children.clone();
        after.deleted = false;
        if after != before {
            self.touch(id);
            self.nodes[id as usize] = after;
            self.changed(false);
        }
        if bounds != before.bounds {
            self.set_bounds(id, bounds);
        }
        if rotation != before.rotation
            && let Some((cx, cy)) = self.node(id).map(|n| n.bounds.center())
        {
            self.rotate(id, normalize_degrees(rotation - before.rotation), cx, cy);
        }
        true
    }

    pub fn remove(&mut self, id: NodeId) -> bool {
        let Some(parent) = self.node(id).map(|n| n.parent) else { return false };
        self.list_mut(parent).retain(|c| *c != id);
        for i in self.subtree(id) {
            self.touch(i);
            self.index.remove(&entry(&self.nodes[i as usize]));
            self.nodes[i as usize].deleted = true;
        }
        self.changed(true);
        self.refit(parent);
        true
    }

    /// Moves a node and its descendants.
    pub fn translate(&mut self, id: NodeId, dx: f32, dy: f32) {
        let Some(parent) = self.node(id).map(|n| n.parent) else { return };
        if dx == 0.0 && dy == 0.0 {
            return;
        }
        for i in self.subtree(id) {
            let b = self.nodes[i as usize].bounds;
            self.set_bounds_raw(i, b.translated(dx, dy));
        }
        self.changed(false);
        self.refit(parent);
    }

    /// Sets bounds the way the properties panel does: frames carry their children along when
    /// moved, groups scale their contents, shapes simply take the new rectangle. Resizing a
    /// rotated layer keeps its (rotated) top-left corner in place, as Figma does.
    pub fn set_bounds(&mut self, id: NodeId, bounds: Rect) {
        let Some(node) = self.node(id) else { return };
        let (old, kind, parent, rotation) = (node.bounds, node.kind, node.parent, node.rotation);
        let mut bounds = bounds;
        if rotation != 0.0 && (bounds.x, bounds.y) == (old.x, old.y) {
            let (ox, oy) = old.corners(rotation)[0];
            let (nx, ny) = bounds.corners(rotation)[0];
            bounds = bounds.translated(ox - nx, oy - ny);
        }
        let inner = self.subtree(id).into_iter().skip(1).collect::<Vec<_>>();
        match kind {
            Kind::Group => {
                let sx = if old.w > 0.0 { bounds.w / old.w } else { 1.0 };
                let sy = if old.h > 0.0 { bounds.h / old.h } else { 1.0 };
                for i in inner {
                    let b = self.nodes[i as usize].bounds;
                    let r = Rect::new(bounds.x + (b.x - old.x) * sx, bounds.y + (b.y - old.y) * sy, b.w * sx, b.h * sy);
                    self.set_bounds_raw(i, r);
                }
            }
            Kind::Frame if rotation == 0.0 && (old.w, old.h) != (bounds.w, bounds.h) => {
                // A resize: each child follows its constraints (and takes its own subtree along).
                let kids = self.nodes[id as usize].children.clone();
                for c in kids {
                    let n = &self.nodes[c as usize];
                    if n.rotation != 0.0 {
                        let b = n.bounds;
                        self.translate(c, bounds.x - old.x, bounds.y - old.y);
                        let _ = b;
                        continue;
                    }
                    let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
                    let [ch, cv] = n.constraints;
                    let (x, w) = ch.place(b.x, b.w, old.x, old.w, bounds.x, bounds.w);
                    let (y, h) = cv.place(b.y, b.h, old.y, old.h, bounds.y, bounds.h);
                    let r = Rect::new(x, y, w, h);
                    if r != b {
                        self.set_bounds(c, r);
                    }
                }
            }
            Kind::Frame => {
                let (ox, oy) = old.corners(rotation)[0];
                let (nx, ny) = bounds.corners(rotation)[0];
                for i in inner {
                    let b = self.nodes[i as usize].bounds;
                    self.set_bounds_raw(i, b.translated(nx - ox, ny - oy));
                }
            }
            _ => {}
        }
        self.set_bounds_raw(id, bounds);
        self.changed(false);
        self.refit(parent);
    }

    /// Turns a layer and everything inside it by `degrees` about (px, py). Groups have no rotation
    /// of their own; they refit around their turned children.
    pub fn rotate(&mut self, id: NodeId, degrees: f32, px: f32, py: f32) {
        let Some(parent) = self.node(id).map(|n| n.parent) else { return };
        if degrees == 0.0 {
            return;
        }
        let ids = self.subtree(id);
        for i in &ids {
            let n = &self.nodes[*i as usize];
            if n.kind == Kind::Group {
                continue;
            }
            let (cx, cy) = n.bounds.center();
            let (nx, ny) = rotate_point(cx, cy, px, py, degrees);
            let bounds = n.bounds.centered(nx, ny);
            let rotation = normalize_degrees(n.rotation + degrees);
            self.set_geometry_raw(*i, bounds, rotation);
        }
        self.refit_inside(&ids);
        self.changed(false);
        self.refit(parent);
    }

    /// Mirrors a layer and its contents across the vertical (or horizontal) line through the
    /// pivot. Shapes are symmetric, so mirroring moves them and negates their rotation.
    pub fn flip(&mut self, id: NodeId, horizontal: bool, pivot: f32) {
        let Some(parent) = self.node(id).map(|n| n.parent) else { return };
        let ids = self.subtree(id);
        for i in &ids {
            let n = &self.nodes[*i as usize];
            if n.kind == Kind::Group {
                continue;
            }
            let (cx, cy) = n.bounds.center();
            let (nx, ny) = if horizontal { (2.0 * pivot - cx, cy) } else { (cx, 2.0 * pivot - cy) };
            let bounds = n.bounds.centered(nx, ny);
            let rotation = normalize_degrees(-n.rotation);
            self.set_geometry_raw(*i, bounds, rotation);
        }
        self.refit_inside(&ids);
        self.changed(false);
        self.refit(parent);
    }

    /// Refits the groups among `ids` (a subtree in pre-order), innermost first.
    pub(crate) fn refit_inside(&mut self, ids: &[NodeId]) {
        for i in ids.iter().rev() {
            if self.node(*i).is_some_and(|n| n.kind == Kind::Group && !n.children.is_empty()) {
                let b = Rect::union_all(self.nodes[*i as usize].children.iter().map(|c| self.nodes[*c as usize].aabb()));
                if let Some(b) = b {
                    self.set_bounds_raw(*i, b);
                }
            }
        }
    }

    /// Moves `id` under `parent` at `index` (on top when `None`), keeping its place on canvas.
    pub fn reparent(&mut self, id: NodeId, parent: Option<NodeId>, index: Option<usize>) -> bool {
        let Some(old_parent) = self.node(id).map(|n| n.parent) else { return false };
        if let Some(p) = parent
            && (p == id || self.is_ancestor(id, p) || !self.node(p).is_some_and(|n| n.kind.is_container()))
        {
            return false;
        }
        let Some(pos) = self.children(old_parent).iter().position(|c| *c == id) else { return false };
        self.list_mut(old_parent).remove(pos);
        let index = if old_parent == parent { index.map(|i| if i > pos { i - 1 } else { i }) } else { index };
        let list = self.list_mut(parent);
        let at = index.unwrap_or(list.len()).min(list.len());
        list.insert(at, id);
        self.touch(id);
        self.nodes[id as usize].parent = parent;
        self.changed(true);
        self.refit(parent);
        self.refit(old_parent);
        true
    }

    pub fn position(&self, id: NodeId) -> Option<(Option<NodeId>, usize)> {
        let parent = self.node(id)?.parent;
        Some((parent, self.children(parent).iter().position(|c| *c == id)?))
    }

    /// Wraps the layers in a new group (or frame), placed where the topmost of them was.
    pub fn group(&mut self, ids: &[NodeId], kind: Kind) -> Option<NodeId> {
        let ids = self.outermost(ids);
        let top = *ids.last()?;
        let (parent, index) = self.position(top)?;
        let bounds = Rect::union_all(ids.iter().map(|id| self.nodes[*id as usize].bounds))?;
        let fill = if kind == Kind::Frame { Color::rgba(255, 255, 255, 255) } else { Color(0) };
        let group = self.insert(Node::new(kind, self.next_name(kind), bounds, fill), parent, Some(index + 1));
        for id in ids {
            self.reparent(id, Some(group), None);
        }
        Some(group)
    }

    /// Replaces a group or frame with its children (Figma's Ungroup / Remove frame).
    pub fn ungroup(&mut self, id: NodeId) -> Vec<NodeId> {
        let Some(node) = self.node(id) else { return Vec::new() };
        if !node.kind.is_container() || node.children.is_empty() {
            return Vec::new();
        }
        let children = node.children.clone();
        let Some((parent, index)) = self.position(id) else { return Vec::new() };
        for (k, c) in children.iter().enumerate() {
            self.reparent(*c, parent, Some(index + k));
        }
        if self.node(id).is_some() {
            self.remove(id);
        }
        children
    }

    pub fn reorder(&mut self, ids: &[NodeId], order: Order) {
        let ids = self.outermost(ids);
        let mut parents: Vec<Option<NodeId>> = ids.iter().map(|id| self.nodes[*id as usize].parent).collect();
        parents.sort_unstable();
        parents.dedup();
        for parent in parents {
            let mut list = self.children(parent).to_vec();
            let picked = |c: &NodeId| ids.contains(c);
            match order {
                Order::Front | Order::Back => {
                    let (moved, rest): (Vec<NodeId>, Vec<NodeId>) = list.iter().partition(|c| picked(c));
                    list = if order == Order::Front { [rest, moved].concat() } else { [moved, rest].concat() };
                }
                Order::Forward => {
                    for i in (0..list.len().saturating_sub(1)).rev() {
                        if picked(&list[i]) && !picked(&list[i + 1]) {
                            list.swap(i, i + 1);
                        }
                    }
                }
                Order::Backward => {
                    for i in 1..list.len() {
                        if picked(&list[i]) && !picked(&list[i - 1]) {
                            list.swap(i, i - 1);
                        }
                    }
                }
            }
            if list != self.children(parent) {
                *self.list_mut(parent) = list;
                self.changed(true);
            }
        }
    }

    pub fn demo(count: usize) -> Self {
        let columns = (count as f32).sqrt().ceil() as usize;
        let mut seed = 0x2545_f491_u32;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        let nodes = (0..count)
            .map(|i| {
                let (col, row) = ((i % columns.max(1)) as f32, (i / columns.max(1)) as f32);
                let r = next();
                let size = 24.0 + (r % 40) as f32;
                let fill = Color::rgba((r >> 8) as u8, (r >> 16) as u8, (r >> 24) as u8 | 0x40, 0xff);
                let bounds = Rect::new(col * 80.0, row * 80.0, size, size);
                let node = if r % 3 == 0 {
                    Node::new(Kind::Ellipse, format!("Ellipse {}", i + 1), bounds, fill)
                } else {
                    Node::new(Kind::Rectangle, format!("Rectangle {}", i + 1), bounds, fill).radius((r % 12) as f32)
                };
                if r % 5 == 0 { node.stroke(Color::rgba(20, 20, 30, 255), 2.0) } else { node }
            })
            .collect();
        Self::new(nodes)
    }
}

#[cfg(test)]
mod constraint_tests {
    use super::tests::{frame, rect};
    use super::*;

    #[test]
    fn children_follow_their_constraints_when_a_frame_resizes() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Screen", 0.0, 0.0, 400.0, 300.0));
        let left = scene.insert(rect("Left", 10.0, 10.0, 50.0, 20.0), Some(f), None);
        let right = scene.insert(rect("Right", 340.0, 10.0, 50.0, 20.0), Some(f), None);
        let bar = scene.insert(rect("Bar", 10.0, 260.0, 380.0, 30.0), Some(f), None);
        let mid = scene.insert(rect("Mid", 175.0, 125.0, 50.0, 50.0), Some(f), None);
        scene.update(right, |n| n.constraints = [Constraint::Max, Constraint::Min]);
        scene.update(bar, |n| n.constraints = [Constraint::Stretch, Constraint::Max]);
        scene.update(mid, |n| n.constraints = [Constraint::Center, Constraint::Scale]);
        scene.set_bounds(f, Rect::new(0.0, 0.0, 600.0, 600.0));
        assert_eq!(scene.node(left).unwrap().bounds, Rect::new(10.0, 10.0, 50.0, 20.0));
        assert_eq!(scene.node(right).unwrap().bounds, Rect::new(540.0, 10.0, 50.0, 20.0));
        assert_eq!(scene.node(bar).unwrap().bounds, Rect::new(10.0, 560.0, 580.0, 30.0));
        assert_eq!(scene.node(mid).unwrap().bounds, Rect::new(275.0, 250.0, 50.0, 100.0));
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn rect(name: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
        Node::new(Kind::Rectangle, name, Rect::new(x, y, w, h), Color(0))
    }

    pub fn frame(name: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
        Node::new(Kind::Frame, name, Rect::new(x, y, w, h), Color(0))
    }

    #[test]
    fn visible_returns_only_nodes_in_area_in_paint_order() {
        let scene = Scene::demo(100);
        let area = Rect::new(0.0, 0.0, 170.0, 70.0);
        let ids: Vec<NodeId> = scene.visible(&area).map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 2]);
    }

    #[test]
    fn each_scene_gets_a_new_generation() {
        assert_ne!(Scene::demo(4).generation(), Scene::demo(4).generation());
        assert!(Scene::demo(50).max_extent() >= 24.0);
    }

    #[test]
    fn bounds_cover_all_nodes() {
        let scene = Scene::demo(16);
        let b = scene.bounds();
        assert!(scene.nodes().all(|n| n.bounds.x >= b.x && n.bounds.right() <= b.right() + 0.01));
    }

    #[test]
    fn clicks_select_children_of_top_level_frames() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.insert(rect("A", 10.0, 10.0, 50.0, 50.0), Some(f), None);
        let b = scene.insert(rect("B", 30.0, 30.0, 50.0, 50.0), Some(f), None);
        assert_eq!(scene.pick(40.0, 40.0, false, &[]), Some(b));
        assert_eq!(scene.pick(15.0, 15.0, false, &[]), Some(a));
        assert_eq!(scene.pick(90.0, 5.0, false, &[]), None, "empty frame background starts a marquee");
        assert_eq!(scene.marquee_scope(90.0, 5.0), Some(f));
        assert_eq!(scene.pick(90.0, 5.0, false, &[f]), Some(f), "a selected frame can be dragged");
        assert_eq!(scene.pick(15.0, 15.0, false, &[f]), Some(a));
        assert_eq!(scene.pick(500.0, 5.0, false, &[]), None);
    }

    #[test]
    fn groups_select_as_a_whole_until_entered() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 0.0, 0.0, 10.0, 10.0));
        let b = scene.add(rect("B", 20.0, 0.0, 10.0, 10.0));
        let g = scene.group(&[a, b], Kind::Group).unwrap();
        assert_eq!(scene.node(g).unwrap().bounds, Rect::new(0.0, 0.0, 30.0, 10.0));
        assert_eq!(scene.pick(5.0, 5.0, false, &[]), Some(g));
        assert_eq!(scene.pick(5.0, 5.0, true, &[]), Some(a));
        assert_eq!(scene.dive(25.0, 5.0, &[g]), Some(b));
        assert_eq!(scene.pick(5.0, 5.0, false, &[b]), Some(a), "siblings of the selection are picked directly");
        assert_eq!(scene.hit(15.0, 5.0), None, "groups have no area of their own");
    }

    #[test]
    fn frames_clip_and_hidden_layers_are_skipped() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.insert(rect("A", 80.0, 0.0, 50.0, 50.0), Some(f), None);
        assert_eq!(scene.pick(90.0, 10.0, false, &[]), Some(a));
        assert_eq!(scene.hit(120.0, 10.0), None, "outside the frame the child is clipped");
        scene.update(a, |n| n.visible = false);
        assert_eq!(scene.hit(90.0, 10.0), Some(f));
        assert!(scene.reach(a).is_none());
    }

    #[test]
    fn moving_a_frame_moves_its_children_and_groups_refit() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.insert(rect("A", 10.0, 10.0, 10.0, 10.0), Some(f), None);
        let b = scene.insert(rect("B", 40.0, 10.0, 10.0, 10.0), Some(f), None);
        let g = scene.group(&[a, b], Kind::Group).unwrap();
        assert_eq!(scene.node(g).unwrap().parent, Some(f));
        scene.translate(f, 100.0, 0.0);
        assert_eq!(scene.node(a).unwrap().bounds.x, 110.0);
        assert_eq!(scene.node(g).unwrap().bounds.x, 110.0);
        scene.translate(b, 10.0, 0.0);
        assert_eq!(scene.node(g).unwrap().bounds.w, 50.0);
        scene.set_bounds(g, Rect::new(110.0, 10.0, 100.0, 10.0));
        assert_eq!(scene.node(b).unwrap().bounds, Rect::new(190.0, 10.0, 20.0, 10.0));
    }

    #[test]
    fn ungroup_restores_children_in_place_and_empty_groups_vanish() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 0.0, 0.0, 10.0, 10.0));
        let b = scene.add(rect("B", 20.0, 0.0, 10.0, 10.0));
        let c = scene.add(rect("C", 40.0, 0.0, 10.0, 10.0));
        let g = scene.group(&[a, b], Kind::Group).unwrap();
        assert_eq!(scene.roots(), &[g, c]);
        assert_eq!(scene.ungroup(g), vec![a, b]);
        assert_eq!(scene.roots(), &[a, b, c]);
        let g = scene.group(&[a], Kind::Group).unwrap();
        scene.remove(a);
        assert!(scene.node(g).is_none());
        assert_eq!(scene.roots(), &[b, c]);
    }

    #[test]
    fn reorder_moves_layers_within_their_parent() {
        let mut scene = Scene::default();
        let ids: Vec<NodeId> = (0..4).map(|i| scene.add(rect(&format!("{i}"), 0.0, 0.0, 1.0, 1.0))).collect();
        scene.reorder(&[ids[0]], Order::Front);
        assert_eq!(scene.roots(), &[1, 2, 3, 0]);
        scene.reorder(&[ids[0]], Order::Backward);
        assert_eq!(scene.roots(), &[1, 2, 0, 3]);
        scene.reorder(&[ids[3]], Order::Back);
        assert_eq!(scene.roots(), &[3, 1, 2, 0]);
        scene.reorder(&[ids[3]], Order::Forward);
        assert_eq!(scene.roots(), &[1, 3, 2, 0]);
        assert!(scene.rank(1) < scene.rank(3));
    }

    #[test]
    fn dropping_into_a_frame_reparents() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.add(rect("A", 200.0, 0.0, 10.0, 10.0));
        assert_eq!(scene.drop_target(50.0, 50.0, &[a]), Some(f));
        assert_eq!(scene.drop_target(50.0, 50.0, &[f]), None);
        assert!(scene.reparent(a, Some(f), None));
        assert!(!scene.reparent(f, Some(a), None));
        assert_eq!(scene.node(a).unwrap().parent, Some(f));
        assert!(scene.select_area(&Rect::new(150.0, -10.0, 100.0, 50.0), None).is_empty());
        assert_eq!(scene.select_area(&Rect::new(-10.0, -10.0, 300.0, 300.0), None), vec![f]);
    }

    #[test]
    fn rotation_turns_hit_testing_bounds_and_children() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 20.0));
        let a = scene.insert(rect("A", 80.0, 0.0, 20.0, 20.0), Some(f), None);
        assert_eq!(scene.hit(50.0, 40.0), None);
        scene.rotate(f, 90.0, 50.0, 10.0);
        // A long thin frame turned upright: (50, 40) is now inside it, (90, 10) is not.
        assert_eq!(scene.hit(50.0, 40.0), Some(f));
        assert_eq!(scene.hit(90.0, 10.0), None);
        let n = scene.node(a).unwrap();
        assert_eq!(n.rotation, 90.0);
        let (cx, cy) = n.bounds.center();
        assert!((cx - 50.0).abs() < 1e-3 && (cy - -30.0).abs() < 1e-3, "child centre turned about the pivot");
        assert_eq!(scene.pick(50.0, -30.0, false, &[]), Some(a));
        let b = scene.node(f).unwrap().aabb();
        assert!((b.w - 20.0).abs() < 1e-3 && (b.h - 100.0).abs() < 1e-3);
        scene.flip(f, true, 0.0);
        assert_eq!(scene.node(f).unwrap().rotation, -90.0);
        assert!((scene.node(a).unwrap().bounds.center().0 + 50.0).abs() < 1e-3);
    }

    #[test]
    fn resizing_a_rotated_layer_keeps_its_corner() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 0.0, 0.0, 100.0, 50.0));
        scene.update(a, |n| n.rotation = 30.0);
        let before = scene.node(a).unwrap().bounds.corners(30.0)[0];
        scene.set_bounds(a, Rect::new(0.0, 0.0, 200.0, 80.0));
        let after = scene.node(a).unwrap();
        let corner = after.bounds.corners(30.0)[0];
        assert!((before.0 - corner.0).abs() < 1e-3 && (before.1 - corner.1).abs() < 1e-3);
        assert_eq!((after.bounds.w, after.bounds.h), (200.0, 80.0));
    }

    #[test]
    fn ellipse_hit_ignores_corners() {
        let scene = Scene::new(vec![Node::new(Kind::Ellipse, "E", Rect::new(0.0, 0.0, 100.0, 100.0), Color(0))]);
        assert_eq!(scene.hit(50.0, 50.0), Some(0));
        assert_eq!(scene.hit(3.0, 3.0), None);
    }

    #[test]
    fn update_moves_the_node_in_the_index_and_remove_hides_it() {
        let mut scene = Scene::demo(4);
        scene.update(0, |n| n.bounds.x = 5000.0);
        assert_eq!(scene.hit(5010.0, 10.0), Some(0));
        assert!(scene.remove(0));
        assert_eq!(scene.hit(5010.0, 10.0), None);
        assert_eq!(scene.len(), 3);
    }

    #[test]
    fn names_count_up_per_kind() {
        let mut scene = Scene::demo(0);
        assert_eq!(scene.next_name(Kind::Rectangle), "Rectangle 1");
        scene.add(Node::new(Kind::Rectangle, "Rectangle 1", Rect::default(), Color(0)));
        assert_eq!(scene.next_name(Kind::Rectangle), "Rectangle 2");
        assert_eq!(scene.next_name(Kind::Frame), "Frame 1");
    }
}
