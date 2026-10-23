//! Figma-style auto layout, computed with taffy (flexbox and grid). Bounds stay absolute: the
//! layout runs in each auto-layout frame's own coordinates and the results are written back.

use serde::{Deserialize, Serialize};
use taffy::prelude::{
    AlignItems, AlignSelf, AvailableSpace, Dimension, Display, FlexDirection, FlexWrap, JustifyContent, LengthPercentage,
    Rect as TRect, Size, Style, TaffyTree, TrackSizingFunction, auto, fr, length, repeat,
};

use crate::geom::{Rect, rotate_point};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::text::{Text, TextResize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Flow {
    #[default]
    Horizontal,
    Vertical,
    Wrap,
    Grid,
}

impl Flow {
    pub fn is_horizontal(self) -> bool {
        matches!(self, Flow::Horizontal | Flow::Wrap)
    }
}

/// Distribution along the flow ("primary axis").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Justify {
    #[default]
    Start,
    Center,
    End,
    SpaceBetween,
}

/// Alignment across the flow ("counter axis").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Baseline,
}

/// Figma's constraints: how a layer follows its frame when the frame is resized (per axis:
/// left/top, right/bottom, both, centre or scale).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Constraint {
    #[default]
    Min,
    Max,
    Stretch,
    Center,
    Scale,
}

impl Constraint {
    /// The new start and length on one axis, for a layer at `pos`/`len` in a frame that went
    /// from `old_start`/`old_len` to `new_start`/`new_len`.
    pub fn place(self, pos: f32, len: f32, old_start: f32, old_len: f32, new_start: f32, new_len: f32) -> (f32, f32) {
        match self {
            Constraint::Min => (new_start + (pos - old_start), len),
            Constraint::Max => (new_start + new_len - (old_start + old_len - pos), len),
            Constraint::Stretch => (new_start + (pos - old_start), (len + new_len - old_len).max(0.0)),
            Constraint::Center => {
                let offset = pos + len / 2.0 - (old_start + old_len / 2.0);
                (new_start + new_len / 2.0 + offset - len / 2.0, len)
            }
            Constraint::Scale => {
                let k = if old_len > 0.0 { new_len / old_len } else { 1.0 };
                (new_start + (pos - old_start) * k, len * k)
            }
        }
    }
}

/// Figma's resizing per axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sizing {
    #[default]
    Fixed,
    Hug,
    Fill,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoLayout {
    pub flow: Flow,
    /// Spacing between items along the flow (column gap for grids).
    pub gap: f32,
    /// Spacing between wrapped rows (row gap for grids).
    pub counter_gap: f32,
    /// Top, right, bottom, left.
    pub padding: [f32; 4],
    pub justify: Justify,
    pub align: Align,
    pub columns: u16,
    pub rows: u16,
}

impl Default for AutoLayout {
    fn default() -> Self {
        Self {
            flow: Flow::Horizontal,
            gap: 10.0,
            counter_gap: 10.0,
            padding: [0.0; 4],
            justify: Justify::Start,
            align: Align::Start,
            columns: 2,
            rows: 2,
        }
    }
}

/// Measures text for the layout: `width` is the wrapping width, `None` for one line per paragraph.
pub trait Measure {
    fn text(&self, text: &Text, width: Option<f32>) -> (f32, f32);
}

/// No fonts: text keeps its current size (used in tests and headless tools without a renderer).
pub struct KeepSize;

impl Measure for KeepSize {
    fn text(&self, _: &Text, _: Option<f32>) -> (f32, f32) {
        (f32::NAN, f32::NAN)
    }
}

struct Placement {
    origin: (f32, f32),
    pivot: (f32, f32),
    rotation: f32,
}

struct Leaf {
    text: Option<Text>,
    size: (f32, f32),
}

fn sizing_dimension(sizing: Sizing, value: f32) -> Dimension {
    match sizing {
        Sizing::Fixed => length(value),
        Sizing::Hug | Sizing::Fill => auto(),
    }
}

impl Scene {
    /// Whether the layer is placed by its parent's auto layout.
    pub fn in_auto_layout(&self, id: NodeId) -> bool {
        let Some(n) = self.node(id) else { return false };
        n.visible && !n.absolute && n.parent.and_then(|p| self.node(p)).is_some_and(|p| p.layout.is_some())
    }

    fn flow_children(&self, id: NodeId) -> Vec<NodeId> {
        self.node(id).map_or(Vec::new(), |n| n.children.iter().copied().filter(|c| self.in_auto_layout(*c)).collect())
    }

    /// Re-runs every auto layout in the document, outermost frames first.
    pub fn relayout(&mut self, measure: &dyn Measure) {
        let roots: Vec<NodeId> = self
            .nodes()
            .filter(|n| n.layout.is_some() && n.kind == Kind::Frame)
            .map(|n| n.id)
            .filter(|id| !self.in_auto_layout(*id))
            .collect();
        for root in roots {
            self.layout_tree(root, measure);
        }
    }

    fn item_style(&self, n: &Node, parent: Option<&Node>) -> Style {
        let mut style = Style { flex_shrink: 0.0, ..Style::default() };
        let w = n.bounds.w;
        let h = n.bounds.h;
        let [sx, sy] = self.effective_sizing(n);
        style.size = Size { width: sizing_dimension(sx, w), height: sizing_dimension(sy, h) };
        if let Some(layout) = parent.and_then(|p| p.layout.as_deref()) {
            let horizontal = layout.flow.is_horizontal();
            let (main, cross) = if horizontal { (sx, sy) } else { (sy, sx) };
            if layout.flow == Flow::Grid {
                if sx == Sizing::Fill {
                    style.justify_self = Some(AlignSelf::STRETCH);
                }
                if sy == Sizing::Fill {
                    style.align_self = Some(AlignSelf::STRETCH);
                }
            } else {
                if main == Sizing::Fill {
                    style.flex_grow = 1.0;
                    style.flex_shrink = 1.0;
                    style.flex_basis = length(0.0);
                }
                if cross == Sizing::Fill {
                    style.align_self = Some(AlignSelf::STRETCH);
                }
            }
        }
        style
    }

    /// Sizing as the layout sees it: Fill only inside auto layout, Hug only where content can hug.
    pub fn effective_sizing(&self, n: &Node) -> [Sizing; 2] {
        let in_layout = self.in_auto_layout(n.id);
        let can_hug = n.layout.is_some() || n.kind == Kind::Text;
        let mut s = n.sizing;
        if let Some(t) = n.text.as_deref() {
            s = match t.resize {
                TextResize::AutoWidth => [Sizing::Hug, Sizing::Hug],
                TextResize::AutoHeight => [if s[0] == Sizing::Fill { Sizing::Fill } else { Sizing::Fixed }, Sizing::Hug],
                TextResize::Fixed => [
                    if s[0] == Sizing::Fill { Sizing::Fill } else { Sizing::Fixed },
                    if s[1] == Sizing::Fill { Sizing::Fill } else { Sizing::Fixed },
                ],
            };
        }
        s.map(|v| match v {
            Sizing::Fill if !in_layout => Sizing::Fixed,
            Sizing::Hug if !can_hug => Sizing::Fixed,
            v => v,
        })
    }

    fn container_style(layout: &crate::layout::AutoLayout, mut style: Style, hug_width: bool, hug_height: bool) -> Style {
        let [top, right, bottom, left] = layout.padding.map(|p| LengthPercentage::length(p.max(0.0)));
        style.padding = TRect { left, right, top, bottom };
        let justify = match layout.justify {
            Justify::Start => JustifyContent::FLEX_START,
            Justify::Center => JustifyContent::CENTER,
            Justify::End => JustifyContent::FLEX_END,
            Justify::SpaceBetween => JustifyContent::SPACE_BETWEEN,
        };
        let align = match layout.align {
            Align::Start => AlignItems::FLEX_START,
            Align::Center => AlignItems::CENTER,
            Align::End => AlignItems::FLEX_END,
            Align::Baseline => AlignItems::BASELINE,
        };
        let gap = LengthPercentage::length(layout.gap.max(0.0));
        let counter = LengthPercentage::length(layout.counter_gap.max(0.0));
        match layout.flow {
            Flow::Grid => {
                style.display = Display::Grid;
                let track = |hug: bool| -> TrackSizingFunction { if hug { auto() } else { fr(1.0) } };
                style.grid_template_columns = vec![repeat(layout.columns.max(1), vec![track(hug_width)])];
                style.grid_template_rows = vec![repeat(layout.rows.max(1), vec![track(hug_height)])];
                style.gap = Size { width: gap, height: counter };
                style.justify_items = align_to_items(layout.justify);
                style.align_items = align;
            }
            flow => {
                style.display = Display::Flex;
                let horizontal = flow.is_horizontal();
                style.flex_direction = if horizontal { FlexDirection::Row } else { FlexDirection::Column };
                style.flex_wrap = if flow == Flow::Wrap { FlexWrap::Wrap } else { FlexWrap::NoWrap };
                style.gap = if horizontal { Size { width: gap, height: counter } } else { Size { width: counter, height: gap } };
                style.justify_content = justify;
                style.align_items = align;
                style.align_content = taffy::prelude::AlignContent::FLEX_START;
            }
        }
        style
    }

    fn build(&self, tree: &mut TaffyTree<Leaf>, id: NodeId) -> Option<taffy::NodeId> {
        let n = self.node(id)?;
        let parent = n.parent.and_then(|p| self.node(p)).filter(|_| self.in_auto_layout(id));
        let mut style = self.item_style(n, parent);
        if let Some(layout) = n.layout.as_deref().filter(|_| n.kind == Kind::Frame) {
            let [sx, sy] = self.effective_sizing(n);
            style = Self::container_style(layout, style, sx == Sizing::Hug, sy == Sizing::Hug);
            let children: Vec<taffy::NodeId> =
                self.flow_children(id).into_iter().filter_map(|c| self.build(tree, c)).collect();
            return tree.new_with_children(style, &children).ok();
        }
        let size = if n.kind == Kind::Group { (n.aabb().w, n.aabb().h) } else { (n.bounds.w, n.bounds.h) };
        tree.new_leaf_with_context(style, Leaf { text: n.text.as_deref().cloned(), size }).ok()
    }

    fn layout_tree(&mut self, root: NodeId, measure: &dyn Measure) {
        let mut tree: TaffyTree<Leaf> = TaffyTree::new();
        let Some(top) = self.build(&mut tree, root) else { return };
        let computed = tree.compute_layout_with_measure(top, Size { width: AvailableSpace::MaxContent, height: AvailableSpace::MaxContent }, |inputs, _, leaf, style| {
            taffy::compute_leaf_layout(inputs, style, |_, _| 0.0, |known, available| {
                let Some(leaf) = leaf else { return Size::ZERO };
                if let (Some(width), Some(height)) = (known.width, known.height) {
                    return Size { width, height };
                }
                let Some(text) = &leaf.text else {
                    return Size { width: known.width.unwrap_or(leaf.size.0), height: known.height.unwrap_or(leaf.size.1) };
                };
                let wrap = known.width.or(match (text.resize, available.width) {
                    (TextResize::AutoWidth, _) => None,
                    (_, AvailableSpace::Definite(w)) => Some(w),
                    _ => Some(leaf.size.0),
                });
                let (w, h) = measure.text(text, wrap);
                let (w, h) = if w.is_nan() { leaf.size } else { (w, h) };
                Size { width: known.width.unwrap_or(w), height: known.height.unwrap_or(h) }
            })
        });
        if computed.is_err() {
            return;
        }
        // Hug: the root takes its laid-out size (set_bounds keeps a rotated root's corner).
        let Ok(size) = tree.layout(top).map(|l| l.size) else { return };
        let Some(old) = self.node(root).map(|n| n.bounds) else { return };
        if (size.width - old.w).abs() > 1e-3 || (size.height - old.h).abs() > 1e-3 {
            self.set_bounds(root, Rect::new(old.x, old.y, size.width, size.height));
        }
        let Some(n) = self.node(root) else { return };
        let frame = Placement { origin: (n.bounds.x, n.bounds.y), pivot: n.bounds.center(), rotation: n.rotation };
        self.place_children(&tree, top, root, (0.0, 0.0), &frame);
    }

    fn place_children(
        &mut self,
        tree: &TaffyTree<Leaf>,
        tnode: taffy::NodeId,
        id: NodeId,
        offset: (f32, f32),
        frame: &Placement,
    ) {
        if !self.node(id).is_some_and(|n| n.layout.is_some() && n.kind == Kind::Frame) {
            return;
        }
        let kids = self.flow_children(id);
        let Ok(tkids) = tree.children(tnode) else { return };
        for (child, tchild) in kids.into_iter().zip(tkids) {
            let Ok(l) = tree.layout(tchild) else { continue };
            let local = Rect::new(offset.0 + l.location.x, offset.1 + l.location.y, l.size.width, l.size.height);
            self.place(child, local, frame);
            self.place_children(tree, tchild, child, (local.x, local.y), frame);
        }
    }

    /// Moves and sizes a laid-out layer to `local` (relative to the root frame's unrotated
    /// top-left), turned with the root frame.
    fn place(&mut self, id: NodeId, local: Rect, frame: &Placement) {
        let Some(n) = self.node(id) else { return };
        let (old, kind) = (n.bounds, n.kind);
        let (ox, oy) = frame.origin;
        let (cx, cy) = (ox + local.x + local.w / 2.0, oy + local.y + local.h / 2.0);
        let (cx, cy) = rotate_point(cx, cy, frame.pivot.0, frame.pivot.1, frame.rotation);
        if kind != Kind::Group && ((local.w - old.w).abs() > 1e-3 || (local.h - old.h).abs() > 1e-3) {
            self.set_bounds(id, Rect::new(old.x, old.y, local.w, local.h));
        }
        let Some((nx, ny)) = self.node(id).map(|n| if n.kind == Kind::Group { n.aabb().center() } else { n.bounds.center() })
        else {
            return;
        };
        if (cx - nx).abs() > 1e-4 || (cy - ny).abs() > 1e-4 {
            self.translate(id, cx - nx, cy - ny);
        }
    }

    /// Guesses an auto layout from how the children are arranged now (Figma's Shift+A).
    pub fn infer_layout(&self, frame: Option<Rect>, children: &[NodeId]) -> AutoLayout {
        let boxes: Vec<Rect> = children.iter().filter_map(|c| self.node(*c)).map(|n| n.aabb()).collect();
        let mut layout = AutoLayout { gap: 0.0, counter_gap: 0.0, ..AutoLayout::default() };
        let Some(union) = Rect::union_all(boxes.iter().copied()) else {
            return AutoLayout { padding: [10.0; 4], ..AutoLayout::default() };
        };
        let widest = boxes.iter().map(|b| b.w).fold(0.0, f32::max).max(1.0);
        let tallest = boxes.iter().map(|b| b.h).fold(0.0, f32::max).max(1.0);
        let horizontal = boxes.len() < 2 || union.w / widest >= union.h / tallest;
        layout.flow = if horizontal { Flow::Horizontal } else { Flow::Vertical };
        let mut sorted = boxes.clone();
        sorted.sort_by(|a, b| if horizontal { a.x.total_cmp(&b.x) } else { a.y.total_cmp(&b.y) });
        let mut gaps: Vec<f32> = sorted
            .windows(2)
            .map(|w| if horizontal { w[1].x - w[0].right() } else { w[1].y - w[0].bottom() })
            .map(|g| g.max(0.0).round())
            .collect();
        gaps.sort_by(f32::total_cmp);
        layout.gap = gaps.get(gaps.len() / 2).copied().unwrap_or(10.0);
        if let Some(f) = frame {
            layout.padding = [
                (union.y - f.y).max(0.0).round(),
                (f.right() - union.right()).max(0.0).round(),
                (f.bottom() - union.bottom()).max(0.0).round(),
                (union.x - f.x).max(0.0).round(),
            ];
        }
        layout
    }
}

fn align_to_items(j: Justify) -> AlignItems {
    match j {
        Justify::Start | Justify::SpaceBetween => AlignItems::START,
        Justify::Center => AlignItems::CENTER,
        Justify::End => AlignItems::END,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};

    fn auto_frame(scene: &mut Scene, layout: AutoLayout, sizing: [Sizing; 2]) -> NodeId {
        let f = scene.add(frame("Stack", 0.0, 0.0, 300.0, 100.0));
        scene.update(f, |n| {
            n.layout = Some(Box::new(layout));
            n.sizing = sizing;
        });
        f
    }

    #[test]
    fn horizontal_stack_hugs_its_children_with_gap_and_padding() {
        let mut scene = Scene::default();
        let layout = AutoLayout { gap: 10.0, padding: [5.0, 20.0, 5.0, 20.0], ..AutoLayout::default() };
        let f = auto_frame(&mut scene, layout, [Sizing::Hug, Sizing::Hug]);
        let a = scene.insert(rect("A", 50.0, 50.0, 40.0, 20.0), Some(f), None);
        let b = scene.insert(rect("B", 0.0, 0.0, 60.0, 30.0), Some(f), None);
        scene.relayout(&KeepSize);
        assert_eq!(scene.node(a).unwrap().bounds, Rect::new(20.0, 5.0, 40.0, 20.0));
        assert_eq!(scene.node(b).unwrap().bounds, Rect::new(70.0, 5.0, 60.0, 30.0));
        assert_eq!(scene.node(f).unwrap().bounds, Rect::new(0.0, 0.0, 150.0, 40.0));
    }

    #[test]
    fn fill_children_share_the_space_and_alignment_centres() {
        let mut scene = Scene::default();
        let layout = AutoLayout { gap: 0.0, align: Align::Center, flow: Flow::Vertical, ..AutoLayout::default() };
        let f = auto_frame(&mut scene, layout, [Sizing::Fixed, Sizing::Fixed]);
        let a = scene.insert(rect("A", 0.0, 0.0, 100.0, 10.0), Some(f), None);
        let b = scene.insert(rect("B", 0.0, 0.0, 100.0, 10.0), Some(f), None);
        scene.update(b, |n| n.sizing = [Sizing::Fill, Sizing::Fill]);
        scene.relayout(&KeepSize);
        assert_eq!(scene.node(a).unwrap().bounds, Rect::new(100.0, 0.0, 100.0, 10.0));
        assert_eq!(scene.node(b).unwrap().bounds, Rect::new(0.0, 10.0, 300.0, 90.0));
    }

    #[test]
    fn nested_layouts_absolute_children_and_moving_the_frame() {
        let mut scene = Scene::default();
        let f = auto_frame(&mut scene, AutoLayout { gap: 0.0, ..AutoLayout::default() }, [Sizing::Hug, Sizing::Hug]);
        let inner = scene.insert(frame("Inner", 0.0, 0.0, 10.0, 10.0), Some(f), None);
        scene.update(inner, |n| {
            n.layout = Some(Box::new(AutoLayout { flow: Flow::Vertical, gap: 4.0, ..AutoLayout::default() }));
            n.sizing = [Sizing::Hug, Sizing::Hug];
        });
        let x = scene.insert(rect("X", 0.0, 0.0, 20.0, 20.0), Some(inner), None);
        let y = scene.insert(rect("Y", 0.0, 0.0, 20.0, 20.0), Some(inner), None);
        let free = scene.insert(rect("Free", 500.0, 500.0, 5.0, 5.0), Some(f), None);
        scene.update(free, |n| n.absolute = true);
        let side = scene.insert(rect("Side", 0.0, 0.0, 30.0, 10.0), Some(f), None);
        scene.relayout(&KeepSize);
        assert_eq!(scene.node(inner).unwrap().bounds, Rect::new(0.0, 0.0, 20.0, 44.0));
        assert_eq!(scene.node(y).unwrap().bounds.y, 24.0);
        assert_eq!(scene.node(side).unwrap().bounds.x, 20.0);
        assert_eq!(scene.node(free).unwrap().bounds.x, 500.0, "absolute children keep their place");
        assert_eq!(scene.node(f).unwrap().bounds.w, 50.0);
        scene.translate(f, 100.0, 0.0);
        scene.relayout(&KeepSize);
        assert_eq!(scene.node(x).unwrap().bounds.x, 100.0);
    }

    #[test]
    fn grid_places_items_in_cells() {
        let mut scene = Scene::default();
        let layout = AutoLayout { flow: Flow::Grid, gap: 10.0, counter_gap: 10.0, columns: 2, rows: 2, ..AutoLayout::default() };
        let f = auto_frame(&mut scene, layout, [Sizing::Fixed, Sizing::Fixed]);
        scene.set_bounds(f, Rect::new(0.0, 0.0, 210.0, 110.0));
        let ids: Vec<NodeId> = (0..4).map(|i| scene.insert(rect(&format!("{i}"), 0.0, 0.0, 20.0, 20.0), Some(f), None)).collect();
        scene.relayout(&KeepSize);
        assert_eq!(scene.node(ids[1]).unwrap().bounds.x, 110.0);
        assert_eq!(scene.node(ids[2]).unwrap().bounds.y, 60.0);
    }

    #[test]
    fn inference_reads_direction_gap_and_padding() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 10.0, 20.0, 50.0, 50.0));
        let b = scene.add(rect("B", 10.0, 86.0, 50.0, 50.0));
        let c = scene.add(rect("C", 10.0, 150.0, 50.0, 50.0));
        let l = scene.infer_layout(Some(Rect::new(0.0, 0.0, 80.0, 220.0)), &[a, b, c]);
        assert_eq!(l.flow, Flow::Vertical);
        assert_eq!(l.gap, 16.0);
        assert_eq!(l.padding, [20.0, 20.0, 20.0, 10.0]);
    }
}
