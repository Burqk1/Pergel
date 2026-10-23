//! Finds the parts of a design that must ship as files instead of being rebuilt in code:
//! icons and illustrations (exported as SVG) and image fills (the original files). Design to
//! code references them by file name, so an agent reuses the designer's artwork verbatim.

use std::collections::{BTreeMap, HashMap};

use serde::Serialize;

use crate::components::Component;
use crate::paint::{Paint, content_hash, image_extension};
use crate::scene::{Kind, Node, NodeId, Scene};

/// Containers up to this size made only of vector art count as icons; bigger ones are
/// illustrations.
const ICON_MAX: f32 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetKind {
    Icon,
    Illustration,
    Image,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub kind: AssetKind,
    /// Suggested file name, unique within the result (`arrow-right.svg`, `hero.jpg`).
    pub file: String,
    /// The layer (or main component) name it was named after.
    pub name: String,
    /// Every layer in the subtree that shows this asset.
    pub nodes: Vec<NodeId>,
    pub width: f32,
    pub height: f32,
    /// "svg", "png", "jpg", "gif" or "webp".
    pub format: String,
    /// Image assets: whether the layer has children on top (use it as a CSS background).
    pub background: bool,
    #[serde(skip)]
    pub content: AssetContent,
}

#[derive(Debug, Clone, Default)]
pub enum AssetContent {
    Svg(String),
    /// Key into the scene's image store.
    Image(String),
    #[default]
    None,
}

impl Asset {
    /// The file's bytes.
    pub fn bytes(&self, scene: &Scene) -> Option<Vec<u8>> {
        match &self.content {
            AssetContent::Svg(s) => Some(s.as_bytes().to_vec()),
            AssetContent::Image(h) => scene.images.get(h).map(|b| b.to_vec()),
            AssetContent::None => None,
        }
    }

    pub fn mime(&self) -> &'static str {
        match self.format.as_str() {
            "svg" => "image/svg+xml",
            "jpg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            _ => "image/png",
        }
    }
}

/// A file name piece from a layer name: `Icon / Arrow Right` → `icon-arrow-right`.
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-').to_string();
    if out.is_empty() { "asset".into() } else { out }
}

/// Layer names that designers give icons and logos.
fn looks_like_icon(name: &str) -> bool {
    let l = name.to_ascii_lowercase();
    l.contains("icon")
        || l.starts_with("ic_")
        || l.starts_with("ic-")
        || l.contains("logo")
        || l.contains("glyph")
        || l.contains("symbol")
        || (l.contains(':') && !l.contains(' '))
}

impl Scene {
    /// The assets in `root`'s subtree, outermost first in layer order.
    pub fn find_assets(&self, root: NodeId) -> Vec<Asset> {
        let mut found: Vec<(NodeId, AssetKind)> = Vec::new();
        self.collect_assets(root, &mut found);
        let mut by_key: BTreeMap<String, usize> = BTreeMap::new();
        let mut assets: Vec<Asset> = Vec::new();
        let mut names: HashMap<String, usize> = HashMap::new();
        for (id, kind) in found {
            let Some(n) = self.node(id) else { continue };
            let (key, content, format, name) = match kind {
                AssetKind::Image => {
                    let Some(Paint::Image(img)) = n.paint.as_deref() else { continue };
                    let ext = self.images.get(&img.hash).map_or("png", |b| image_extension(b));
                    (img.hash.clone(), AssetContent::Image(img.hash.clone()), ext.to_string(), n.name.clone())
                }
                _ => {
                    let Some(svg) = self.to_svg_at_origin(id) else { continue };
                    // Instances are named after their main component ("Icon/Search").
                    let name = match n.component {
                        Some(Component::Instance(main)) => self.node(main).map_or(n.name.clone(), |m| m.name.clone()),
                        _ => n.name.clone(),
                    };
                    (content_hash(svg.as_bytes()), AssetContent::Svg(svg), "svg".to_string(), name)
                }
            };
            if let Some(&i) = by_key.get(&key) {
                assets[i].nodes.push(id);
                assets[i].background |= kind == AssetKind::Image && !n.children.is_empty();
                continue;
            }
            let base = slug(name.rsplit(['/', ':']).next().unwrap_or(&name));
            let count = names.entry(format!("{base}.{format}")).or_insert(0);
            *count += 1;
            let file = if *count == 1 { format!("{base}.{format}") } else { format!("{base}-{count}.{format}") };
            by_key.insert(key, assets.len());
            let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
            assets.push(Asset {
                kind,
                file,
                name,
                nodes: vec![id],
                width: b.w,
                height: b.h,
                format,
                background: kind == AssetKind::Image && !n.children.is_empty(),
                content,
            });
        }
        assets
    }

    fn collect_assets(&self, id: NodeId, out: &mut Vec<(NodeId, AssetKind)>) {
        let Some(n) = self.node(id) else { return };
        if !n.visible || n.slice {
            return;
        }
        if let Some(kind) = self.vector_art(n) {
            out.push((id, kind));
            return;
        }
        if matches!(n.paint.as_deref(), Some(Paint::Image(_))) && n.fill_on {
            out.push((id, AssetKind::Image));
        }
        for c in &n.children {
            self.collect_assets(*c, out);
        }
    }

    /// Whether the layer is artwork to export as one SVG: a vector, or a container drawn only
    /// with shapes that holds at least one vector (or is named like an icon).
    fn vector_art(&self, n: &Node) -> Option<AssetKind> {
        let b = if n.kind == Kind::Group { n.aabb() } else { n.bounds };
        let kind = if b.w.max(b.h) <= ICON_MAX { AssetKind::Icon } else { AssetKind::Illustration };
        match n.kind {
            Kind::Vector => return Some(kind),
            // Type on a path is artwork code can't lay out: it ships as an SVG.
            Kind::Text if n.text.as_deref().is_some_and(|t| t.path.is_some()) => return Some(AssetKind::Illustration),
            Kind::Text | Kind::Rectangle | Kind::Ellipse => return None,
            Kind::Frame | Kind::Group => {}
        }
        if n.layout.is_some() && !looks_like_icon(&n.name) {
            return None;
        }
        let mut vectors: Vec<crate::Rect> = Vec::new();
        let mut shapes = 0;
        for id in self.subtree(n.id).into_iter().skip(1) {
            let Some(c) = self.node(id) else { continue };
            if !c.visible {
                continue;
            }
            match c.kind {
                Kind::Text => return None,
                Kind::Vector => vectors.push(c.aabb()),
                Kind::Rectangle | Kind::Ellipse => shapes += 1,
                Kind::Frame | Kind::Group => {}
            }
            if matches!(c.paint.as_deref(), Some(Paint::Image(_))) {
                return None;
            }
        }
        if looks_like_icon(&n.name) && (shapes > 0 || !vectors.is_empty()) {
            return Some(kind);
        }
        let Some(union) = crate::Rect::union_all(vectors.iter().copied()) else { return None };
        // A plain wrapper (no fill, stroke or effects) around artwork is the artwork; a styled
        // box (a toolbar, a button) only when the artwork fills most of it.
        let styled = (n.fill_on && (n.paint.is_some() || n.fill.argb() >> 24 != 0)) || n.stroke.is_some() || !n.effects.is_empty();
        let coverage = if b.w * b.h > 0.0 { union.w * union.h / (b.w * b.h) } else { 1.0 };
        let enough = match (styled, kind) {
            (false, AssetKind::Icon) => true,
            (false, _) => coverage >= 0.3,
            (true, _) => coverage >= 0.6,
        };
        enough.then_some(kind)
    }

    /// SVG of a layer moved to the origin, so the same artwork gives the same file anywhere.
    pub fn to_svg_at_origin(&self, id: NodeId) -> Option<String> {
        let mut tree = self.extract(id)?;
        let a = tree.aabb();
        tree.translate(-a.x, -a.y);
        let mut tmp = Scene::default();
        tmp.images = self.images.clone();
        let root = tmp.insert_tree(&tree, None, None);
        tmp.to_svg(root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::tests::{frame, rect};
    use crate::{Color, Images, Rect, Text, import_svg};

    #[test]
    fn icons_and_images_become_deduplicated_files() {
        let mut scene = Scene::default();
        let page = scene.add(frame("Page", 0.0, 0.0, 800.0, 600.0));
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><path d="M4 4h16v16H4z"/></svg>"#;
        let mut images = Images::default();
        let icon = import_svg(svg, "Icon/Search", &mut images).unwrap().tree;
        for x in [10.0, 100.0] {
            let mut t = icon.clone();
            t.translate(x, 10.0);
            scene.insert_tree(&t, Some(page), None);
        }
        // A card with text is not an asset, but the icon inside it is.
        let card = scene.insert(frame("Card", 200.0, 200.0, 300.0, 200.0), Some(page), None);
        scene.insert(Node::text(Text::new("Hello", 16.0, 400), 210.0, 210.0, Color(0xff000000)), Some(card), None);
        let mut t = icon.clone();
        t.translate(400.0, 300.0);
        t.name = "Close".into();
        scene.insert_tree(&t, Some(card), None);
        // An image fill.
        let png = [
            0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0, 0, 2, 0, 0, 0, 3, 8, 6, 0, 0, 0,
        ];
        let (hash, w, h) = scene.images.add(png.to_vec()).unwrap();
        assert_eq!((w, h), (2, 3));
        let photo = scene.insert(rect("Hero photo", 0.0, 400.0, 400.0, 200.0), Some(page), None);
        scene.update(photo, |n| n.paint = Some(Box::new(Paint::Image(crate::ImageFill { hash, mode: crate::ImageMode::Fill, width: w, height: h, scale: 1.0, opacity: 1.0, adjust: Default::default() }))));
        scene.insert(rect("Plain box", 0.0, 0.0, 10.0, 10.0), Some(page), None);

        let assets = scene.find_assets(page);
        assert_eq!(assets.len(), 2, "{assets:?}");
        assert_eq!(assets[0].file, "search.svg");
        assert_eq!(assets[0].kind, AssetKind::Icon);
        assert_eq!(assets[0].nodes.len(), 3, "same artwork, one file");
        assert_eq!((assets[0].width, assets[0].height), (24.0, 24.0));
        assert_eq!(assets[1].file, "hero-photo.png");
        assert_eq!(assets[1].kind, AssetKind::Image);
        let AssetContent::Svg(s) = &assets[0].content else { panic!() };
        assert!(s.contains("viewBox=\"0 0 24 24\""));
        assert_eq!(scene.node(page).unwrap().bounds, Rect::new(0.0, 0.0, 800.0, 600.0));
    }
}
