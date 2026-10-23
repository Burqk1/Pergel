//! Prototyping: interactions on layers (on click / hover / after a delay → navigate to a frame,
//! go back, open a link) with transitions, as in Figma's Prototype tab.

use serde::{Deserialize, Serialize};

use crate::scene::{NodeId, Scene};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Trigger {
    #[default]
    OnClick,
    OnHover,
    /// Milliseconds after the frame appears.
    AfterDelay { ms: u32 },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    Navigate { to: NodeId },
    Back,
    OpenUrl { url: String },
    /// Interactive components: the instance becomes another variant of its set.
    ChangeTo { to: NodeId },
    /// Shows a frame above the screen (a menu, a dialog, a sheet).
    OpenOverlay {
        to: NodeId,
        #[serde(default)]
        position: OverlayPosition,
        #[serde(default = "yes", rename = "closeOutside")]
        close_outside: bool,
        /// Darkens what is behind the overlay.
        #[serde(default)]
        dim: bool,
    },
    /// Closes the topmost overlay.
    CloseOverlay,
    /// Sets a variable (by id) for the rest of the prototype run.
    SetVariable { variable: String, value: crate::variables::VarValue },
    /// Runs `then` when the variable equals `equals`, otherwise `otherwise`.
    Conditional {
        variable: String,
        equals: crate::variables::VarValue,
        #[serde(default)]
        then: Vec<Action>,
        #[serde(default)]
        otherwise: Vec<Action>,
    },
    /// Scrolls the screen to a layer.
    ScrollTo { to: NodeId },
}

fn yes() -> bool {
    true
}

/// Where an overlay sits on the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayPosition {
    #[default]
    Center,
    Top,
    Bottom,
    Left,
    Right,
}

impl Action {
    /// Every layer the action points at, nested conditionals included (for id remapping).
    pub fn targets_mut(&mut self, f: &mut impl FnMut(&mut NodeId)) {
        match self {
            Action::Navigate { to } | Action::ChangeTo { to } | Action::OpenOverlay { to, .. } | Action::ScrollTo { to } => f(to),
            Action::Conditional { then, otherwise, .. } => {
                for a in then.iter_mut().chain(otherwise.iter_mut()) {
                    a.targets_mut(f);
                }
            }
            Action::Back | Action::OpenUrl { .. } | Action::CloseOverlay | Action::SetVariable { .. } => {}
        }
    }

    /// One line for agents ("open overlay Menu (center)").
    pub fn describe(&self, name: &dyn Fn(NodeId) -> String, var: &dyn Fn(&str) -> String) -> String {
        match self {
            Action::Navigate { to } => format!("navigate to {}", name(*to)),
            Action::Back => "back".into(),
            Action::OpenUrl { url } => format!("open {url}"),
            Action::ChangeTo { to } => format!("change to {}", name(*to)),
            Action::OpenOverlay { to, position, .. } => format!("open overlay {} ({})", name(*to), format!("{position:?}").to_lowercase()),
            Action::CloseOverlay => "close overlay".into(),
            Action::SetVariable { variable, value } => format!("set {} = {}", var(variable), crate::variables::Variables::display(value)),
            Action::Conditional { variable, equals, then, otherwise } => {
                let list = |a: &[Action]| a.iter().map(|x| x.describe(name, var)).collect::<Vec<_>>().join(", ");
                format!("if {} = {} then [{}] else [{}]", var(variable), crate::variables::Variables::display(equals), list(then), list(otherwise))
            }
            Action::ScrollTo { to } => format!("scroll to {}", name(*to)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Transition {
    #[default]
    Instant,
    Dissolve,
    SmartAnimate,
    SlideLeft,
    SlideRight,
    SlideUp,
    SlideDown,
    Push,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reaction {
    pub trigger: Trigger,
    pub action: Action,
    #[serde(default)]
    pub transition: Transition,
    /// Milliseconds.
    #[serde(default = "duration")]
    pub duration: u32,
}

fn duration() -> u32 {
    300
}

impl Scene {
    /// Top-level frames (prototype screens) in canvas order.
    pub fn screens(&self) -> Vec<NodeId> {
        self.roots().iter().copied().filter(|id| self.node(*id).is_some_and(|n| n.kind == crate::Kind::Frame)).collect()
    }

    /// Every interaction in the page: (layer, reaction).
    pub fn reactions(&self) -> Vec<(NodeId, Reaction)> {
        self.nodes().flat_map(|n| n.reactions.iter().map(move |r| (n.id, r.clone()))).collect()
    }

    /// The layer and all its ancestors are visible.
    pub fn ancestors_visible(&self, id: NodeId) -> bool {
        let mut cur = Some(id);
        while let Some(i) = cur {
            let Some(n) = self.node(i) else { return false };
            if !n.visible {
                return false;
            }
            cur = n.parent;
        }
        true
    }

    /// The area a scrolling screen's content covers, relative to the screen: from its top-left
    /// corner to the farthest layer in the directions it scrolls.
    pub fn scroll_content(&self, screen: NodeId) -> Option<crate::geom::Rect> {
        let s = self.node(screen)?;
        let b = s.bounds;
        let (mut r, mut bottom) = (b.right(), b.bottom());
        for id in self.subtree(screen).into_iter().skip(1) {
            let Some(n) = self.node(id).filter(|_| self.ancestors_visible(id)) else { continue };
            let a = n.aabb();
            r = r.max(a.right());
            bottom = bottom.max(a.bottom());
        }
        let w = if s.scroll.x() { r - b.x } else { b.w };
        let h = if s.scroll.y() { bottom - b.y } else { b.h };
        Some(crate::geom::Rect::new(0.0, 0.0, w, h))
    }

    /// The top-level frame a layer is in (itself for a screen).
    pub fn screen_of(&self, id: NodeId) -> Option<NodeId> {
        let mut cur = id;
        while let Some(p) = self.node(cur)?.parent {
            cur = p;
        }
        Some(cur)
    }
}

/// Prototype overflow of a frame that clips: which way its content scrolls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Scroll {
    #[default]
    None,
    Vertical,
    Horizontal,
    Both,
}

impl Scroll {
    pub fn is_none(&self) -> bool {
        *self == Scroll::None
    }

    pub fn label(self) -> &'static str {
        match self {
            Scroll::None => "none",
            Scroll::Vertical => "vertical",
            Scroll::Horizontal => "horizontal",
            Scroll::Both => "both",
        }
    }

    pub fn x(self) -> bool {
        matches!(self, Scroll::Horizontal | Scroll::Both)
    }

    pub fn y(self) -> bool {
        matches!(self, Scroll::Vertical | Scroll::Both)
    }
}

/// Screen-relative interaction targets, for the presentation viewer.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Hotspot {
    pub id: NodeId,
    pub name: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub reactions: Vec<Reaction>,
}

impl Scene {
    /// The layers with interactions inside `screen`, positioned relative to it, topmost last.
    pub fn hotspots(&self, screen: NodeId) -> Vec<Hotspot> {
        let Some(s) = self.node(screen) else { return Vec::new() };
        let origin = (s.bounds.x, s.bounds.y);
        let scrolls = !s.scroll.is_none();
        self.subtree(screen)
            .into_iter()
            .filter_map(|id| {
                let n = self.node(id)?;
                // Content past a scrolling screen's edge is reachable by scrolling.
                let shown = if scrolls { self.ancestors_visible(id) } else { self.reach(id).is_some() };
                if n.reactions.is_empty() || !shown {
                    return None;
                }
                let b = n.aabb();
                Some(Hotspot { id, name: n.name.clone(), x: b.x - origin.0, y: b.y - origin.1, w: b.w, h: b.h, reactions: n.reactions.clone() })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};

    #[test]
    fn hotspots_are_relative_to_their_screen() {
        let mut scene = Scene::default();
        let home = scene.add(frame("Home", 100.0, 100.0, 375.0, 812.0));
        let detail = scene.add(frame("Detail", 600.0, 100.0, 375.0, 812.0));
        let button = scene.insert(rect("Open", 120.0, 700.0, 335.0, 48.0), Some(home), None);
        scene.update(button, |n| {
            n.reactions = vec![Reaction { trigger: Trigger::OnClick, action: Action::Navigate { to: detail }, transition: Transition::SlideLeft, duration: 300 }]
        });
        assert_eq!(scene.screens(), vec![home, detail]);
        let spots = scene.hotspots(home);
        assert_eq!(spots.len(), 1);
        assert_eq!((spots[0].x, spots[0].y, spots[0].w), (20.0, 600.0, 335.0));
        assert_eq!(scene.screen_of(button), Some(home));
        let json = serde_json::to_string(&spots[0].reactions[0]).unwrap();
        assert!(json.contains("\"type\":\"on-click\"") && json.contains("slide-left"));
    }
}
