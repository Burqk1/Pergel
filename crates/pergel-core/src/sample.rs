use crate::geom::{Color, Rect};
use crate::layout::{AutoLayout, KeepSize, Sizing};
use crate::scene::{Kind, Node, NodeId, Scene};
use crate::text::{LineHeight, Text, TextAlign, TextResize, VerticalAlign};

const WHITE: Color = Color::rgba(255, 255, 255, 255);
const INK: Color = Color::rgba(17, 24, 39, 255);
const MUTED: Color = Color::rgba(75, 85, 99, 255);
const FAINT: Color = Color::rgba(156, 163, 175, 255);
const BLUE: Color = Color::rgba(13, 153, 255, 255);
const SOFT: Color = Color::rgba(243, 244, 246, 255);
const LINE: Color = Color::rgba(229, 231, 235, 255);
const INDIGO: Color = Color::rgba(199, 210, 254, 255);
const LAVENDER: Color = Color::rgba(238, 242, 255, 255);

fn frame(name: &str, x: f32, y: f32, w: f32, h: f32) -> Node {
    Node::new(Kind::Frame, name, Rect::new(x, y, w, h), WHITE)
}

fn rect(name: &str, x: f32, y: f32, w: f32, h: f32, fill: Color) -> Node {
    Node::new(Kind::Rectangle, name, Rect::new(x, y, w, h), fill)
}

fn ellipse(name: &str, x: f32, y: f32, w: f32, h: f32, fill: Color) -> Node {
    Node::new(Kind::Ellipse, name, Rect::new(x, y, w, h), fill)
}

/// An auto-width text layer at (x, y).
fn text(content: &str, x: f32, y: f32, size: f32, weight: u16, fill: Color) -> Node {
    Node::text(Text::new(content, size, weight), x, y, fill)
}

/// A text block wrapping inside `w`.
fn paragraph(content: &str, x: f32, y: f32, w: f32, size: f32, line: f32, fill: Color) -> Node {
    let mut t = Text::new(content, size, 400);
    t.resize = TextResize::AutoHeight;
    t.line_height = LineHeight::Percent(line);
    let mut node = Node::text(t, x, y, fill);
    node.bounds.w = w;
    node
}

/// A label centred in a fixed box, as on a button.
fn label(content: &str, bounds: Rect, size: f32, weight: u16, fill: Color) -> Node {
    let mut t = Text::new(content, size, weight);
    t.resize = TextResize::Fixed;
    t.align = TextAlign::Center;
    t.vertical = VerticalAlign::Middle;
    let mut node = Node::text(t, bounds.x, bounds.y, fill);
    node.bounds = bounds;
    node
}

impl Scene {
    fn named_group(&mut self, ids: &[NodeId], name: &str) -> Option<NodeId> {
        let g = self.group(ids, Kind::Group)?;
        self.update(g, |n| n.name = name.into());
        Some(g)
    }

    fn button(&mut self, parent: Option<NodeId>, name: &str, bounds: Rect, radius: f32, style: (Color, Color, bool)) {
        let (fill, ink, outline) = style;
        let mut bg = rect("Background", bounds.x, bounds.y, bounds.w, bounds.h, fill).radius(radius);
        if outline {
            bg = bg.stroke(BLUE, 2.0);
        }
        let bg = self.insert(bg, parent, None);
        let text = self.insert(label(name, bounds, 16.0, 600, ink), parent, None);
        self.named_group(&[bg, text], name);
    }

    pub fn sample() -> Self {
        let mut s = Scene::default();
        let desktop = s.add(frame("Desktop", 0.0, 0.0, 1440.0, 1024.0));
        let d = Some(desktop);
        let nav = s.insert(rect("Nav bar", 0.0, 0.0, 1440.0, 72.0, INK), d, None);
        let logo = s.insert(ellipse("Logo", 40.0, 16.0, 40.0, 40.0, BLUE), d, None);
        let brand = s.insert(text("Pergel", 96.0, 24.0, 20.0, 700, WHITE), d, None);
        let menu = [
            s.insert(text("Product", 1004.0, 26.0, 15.0, 500, FAINT), d, None),
            s.insert(text("Pricing", 1100.0, 26.0, 15.0, 500, FAINT), d, None),
        ];
        if let Some(header) = s.named_group(&[nav, logo, brand], "Header")
            && let Some(m) = s.named_group(&menu, "Menu")
        {
            s.reparent(m, Some(header), None);
        }
        s.button(d, "Sign in", Rect::new(1240.0, 16.0, 160.0, 40.0), 8.0, (BLUE, WHITE, false));

        let hero = [
            s.insert(text("Design at the", 120.0, 168.0, 64.0, 800, INK), d, None),
            s.insert(text("speed of thought", 120.0, 246.0, 64.0, 800, BLUE), d, None),
            s.insert(
                paragraph(
                    "A fast, open design tool built in Rust. Frames, groups, undo and real text, rendered by Skia on the GPU.",
                    120.0,
                    344.0,
                    520.0,
                    20.0,
                    150.0,
                    MUTED,
                ),
                d,
                None,
            ),
        ];
        s.named_group(&hero, "Hero text");
        s.insert(rect("Hero image", 760.0, 140.0, 560.0, 380.0, INDIGO).radius(24.0), d, None);
        let buttons_start = s.children(d).len();
        s.button(d, "Get started", Rect::new(120.0, 500.0, 180.0, 52.0), 12.0, (BLUE, WHITE, false));
        s.button(d, "Learn more", Rect::new(320.0, 500.0, 180.0, 52.0), 12.0, (WHITE, BLUE, true));
        let buttons: Vec<NodeId> = s.children(d)[buttons_start..].to_vec();
        // An auto-layout row: try changing the gap or resizing a button.
        if let Some(row) = s.group(&buttons, Kind::Frame) {
            s.update(row, |n| {
                n.name = "Buttons".into();
                n.fill_on = false;
                n.clip = false;
                n.layout = Some(Box::new(AutoLayout { gap: 20.0, ..AutoLayout::default() }));
                n.sizing = [Sizing::Hug, Sizing::Hug];
            });
        }

        let cards = s.insert(frame("Cards", 120.0, 640.0, 1200.0, 280.0), d, None);
        s.update(cards, |n| n.fill = Color(0));
        let titles = ["Vector networks", "Auto layout", "Dev Mode"];
        for (i, x) in [120.0, 520.0, 920.0].into_iter().enumerate() {
            let card = s.insert(frame(&format!("Card {}", i + 1), x, 640.0, 400.0, 280.0), Some(cards), None);
            s.update(card, |n| {
                n.fill = SOFT;
                n.radius = 16.0;
                n.stroke = Some((LINE, 1.0));
            });
            let c = Some(card);
            s.insert(ellipse("Avatar", x + 24.0, 664.0, 48.0, 48.0, INDIGO), c, None);
            s.insert(text(titles[i], x + 24.0, 736.0, 22.0, 700, INK), c, None);
            s.insert(
                paragraph(
                    "Placeholder copy for the card. Double-click to edit this text.",
                    x + 24.0,
                    776.0,
                    340.0,
                    16.0,
                    150.0,
                    MUTED,
                ),
                c,
                None,
            );
        }

        let mobile = Some(s.add(frame("Mobile", 1600.0, 0.0, 390.0, 844.0)));
        s.insert(rect("Status bar", 1600.0, 0.0, 390.0, 44.0, INK), mobile, None);
        s.insert(rect("Header", 1616.0, 64.0, 358.0, 180.0, LAVENDER).radius(20.0), mobile, None);
        s.insert(text("Good morning", 1640.0, 96.0, 28.0, 700, INK), mobile, None);
        let items: Vec<_> = (0..3)
            .map(|i| {
                let y = 264.0 + i as f32 * 84.0;
                let bg = s.insert(rect("Background", 1616.0, y, 358.0, 72.0, SOFT).radius(12.0), mobile, None);
                let t = s.insert(text(&format!("List item {}", i + 1), 1640.0, y + 24.0, 17.0, 500, INK), mobile, None);
                s.named_group(&[bg, t], &format!("Item {}", i + 1)).unwrap_or(bg)
            })
            .collect();
        s.named_group(&items, "List");
        s.button(mobile, "Continue", Rect::new(1616.0, 760.0, 358.0, 52.0), 26.0, (BLUE, WHITE, false));
        s.relayout(&KeepSize);
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_has_two_top_level_frames() {
        let scene = Scene::sample();
        let name = |id: Option<crate::NodeId>| id.and_then(|id| scene.node(id)).map(|n| n.name.as_str());
        let top: Vec<_> = scene.roots().iter().map(|id| name(Some(*id)).unwrap()).collect();
        assert_eq!(top, ["Desktop", "Mobile"]);
        assert_eq!(name(scene.pick(200.0, 520.0, false, &[])), Some("Buttons"));
        assert_eq!(name(scene.pick(200.0, 520.0, true, &[])), Some("Get started"));
        assert_eq!(name(scene.pick(1100.0, 10.0, false, &[])), Some("Header"));
        assert_eq!(name(scene.pick(150.0, 680.0, false, &[])), Some("Cards"));
        let label = scene.pick(200.0, 520.0, true, &[]).and_then(|id| scene.node(id)).unwrap();
        assert_eq!(label.text.as_ref().map(|t| t.content.as_str()), Some("Get started"));
    }
}
