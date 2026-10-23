//! Live collaboration merge: three-way, per top-level layer. Each peer keeps the last document
//! everyone agreed on (`base`); when another peer's document arrives, every top-level layer is
//! taken from whichever side changed it since `base` (the other peer when both did), so people
//! working in different frames at the same time keep all of their work.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::clipboard::Tree;
use crate::component_props::PropValue;
use crate::document::Document;
use crate::scene::NodeId;

/// The tree without ids that differ between peers' copies of the same content.
fn plain(t: &Tree) -> Tree {
    let mut t = t.clone();
    fn strip(t: &mut Tree) {
        t.saved_id = None;
        t.children.iter_mut().for_each(strip);
    }
    strip(&mut t);
    t
}

fn same(a: &Tree, b: &Tree) -> bool {
    plain(a) == plain(b)
}

fn ids(t: &Tree, out: &mut HashSet<NodeId>) {
    if let Some(id) = t.saved_id {
        out.insert(id);
    }
    for c in &t.children {
        ids(c, out);
    }
}

/// Gives a tree's layers new ids where they collide with `taken`, rewriting the references
/// inside it (main links, prototype targets, instance-swap values).
fn renumber(t: &mut Tree, taken: &HashSet<NodeId>, next: &mut NodeId) {
    let mut map: HashMap<NodeId, NodeId> = HashMap::new();
    fn assign(t: &mut Tree, taken: &HashSet<NodeId>, next: &mut NodeId, map: &mut HashMap<NodeId, NodeId>) {
        if let Some(id) = t.saved_id
            && taken.contains(&id)
        {
            *next += 1;
            map.insert(id, *next);
            t.saved_id = Some(*next);
        }
        for c in &mut t.children {
            assign(c, taken, next, map);
        }
    }
    fn rewrite(t: &mut Tree, map: &HashMap<NodeId, NodeId>) {
        let fix = |id: &mut NodeId| {
            if let Some(n) = map.get(id) {
                *id = *n;
            }
        };
        if let Some(m) = t.main.as_mut() {
            fix(m);
        }
        for r in &mut t.reactions {
            r.action.targets_mut(&mut |to| fix(to));
        }
        for v in t.prop_values.values_mut() {
            if let PropValue::Swap(m) = v {
                fix(m);
            }
        }
        for p in &mut t.props {
            if let PropValue::Swap(m) = &mut p.default {
                fix(m);
            }
        }
        for c in &mut t.children {
            rewrite(c, map);
        }
    }
    assign(t, taken, next, &mut map);
    if !map.is_empty() {
        rewrite(t, &map);
    }
}

fn pick<T: PartialEq + Clone>(base: &T, local: &T, remote: &T) -> T {
    if local != base && remote == base { local.clone() } else { remote.clone() }
}

/// What changed from `base` to `other`, per top-level layer: `(page, name, "added" | "changed" |
/// "deleted")`, plus document-wide changes (variables, styles).
pub fn diff_documents(base: &Document, other: &Document) -> Vec<(String, String, &'static str)> {
    let mut out = Vec::new();
    for (i, page) in other.pages.iter().enumerate() {
        let before: BTreeMap<String, &Tree> = base.pages.get(i).map(|p| p.layers.iter().filter_map(|t| t.uid.clone().map(|u| (u, t))).collect()).unwrap_or_default();
        let mut seen = HashSet::new();
        for t in &page.layers {
            let change = match t.uid.as_ref().and_then(|u| before.get(u)) {
                None => Some("added"),
                Some(b) if !same(b, t) => Some("changed"),
                _ => None,
            };
            if let Some(u) = &t.uid {
                seen.insert(u.clone());
            }
            if let Some(c) = change {
                out.push((page.name.clone(), t.name.clone(), c));
            }
        }
        for (u, b) in &before {
            if !seen.contains(u) {
                out.push((page.name.clone(), b.name.clone(), "deleted"));
            }
        }
    }
    for p in other.pages.iter().skip(base.pages.len()) {
        out.push((p.name.clone(), "(new page)".into(), "added"));
    }
    if base.variables != other.variables {
        out.push((String::new(), "Variables".into(), "changed"));
    }
    if base.styles != other.styles {
        out.push((String::new(), "Styles".into(), "changed"));
    }
    out
}

/// Merges `local` and `remote` against `base`. Returns the result and whether it holds local
/// changes `remote` doesn't have (so they should be sent on).
pub fn merge_documents(base: &Document, local: &Document, remote: &Document) -> (Document, bool) {
    if base.pages.len() != remote.pages.len() || local.pages.len() != remote.pages.len() {
        // Pages added or removed: the other peer's document as a whole.
        let ahead = local.pages.len() != remote.pages.len() && base.pages.len() == remote.pages.len();
        return if ahead { (local.clone(), true) } else { (remote.clone(), false) };
    }
    let mut out = remote.clone();
    let mut ahead = false;
    // Every id the remote side uses: kept local layers must not collide with them.
    let mut taken: HashSet<NodeId> = HashSet::new();
    for p in &remote.pages {
        for t in &p.layers {
            ids(t, &mut taken);
        }
    }
    let mut next: NodeId = taken.iter().copied().max().unwrap_or(0).max(10_000_000);
    for (i, page) in out.pages.iter_mut().enumerate() {
        let by_uid = |layers: &[Tree]| -> BTreeMap<String, Tree> { layers.iter().filter_map(|t| t.uid.clone().map(|u| (u, t.clone()))).collect() };
        let b = by_uid(&base.pages[i].layers);
        let l = by_uid(&local.pages[i].layers);
        let remote_uids: HashSet<String> = remote.pages[i].layers.iter().filter_map(|t| t.uid.clone()).collect();
        let mut layers: Vec<Tree> = Vec::new();
        // Layers new since `base` (from either side) go last, ordered by uid, so both peers end
        // up with the same order.
        let mut fresh: Vec<Tree> = Vec::new();
        for r in &remote.pages[i].layers {
            let Some(uid) = &r.uid else {
                layers.push(r.clone());
                continue;
            };
            if !b.contains_key(uid) {
                fresh.push(r.clone());
                continue;
            }
            match (b.get(uid), l.get(uid)) {
                // Changed here only: keep the local version.
                (Some(bt), Some(lt)) if !same(lt, bt) && same(r, bt) => {
                    let mut lt = lt.clone();
                    renumber(&mut lt, &taken, &mut next);
                    layers.push(lt);
                    ahead = true;
                }
                // Deleted here, untouched there: stays deleted.
                (Some(bt), None) if same(r, bt) => ahead = true,
                _ => layers.push(r.clone()),
            }
        }
        for (k, lt) in local.pages[i].layers.iter().enumerate() {
            let Some(uid) = &lt.uid else { continue };
            if remote_uids.contains(uid) {
                continue;
            }
            let keep = match b.get(uid) {
                // New here.
                None => true,
                // Deleted there but edited here: the edit wins.
                Some(bt) => !same(lt, bt),
            };
            if keep {
                let mut lt = lt.clone();
                renumber(&mut lt, &taken, &mut next);
                if b.contains_key(uid) {
                    layers.insert(k.min(layers.len()), lt);
                } else {
                    fresh.push(lt);
                }
                ahead = true;
            }
        }
        fresh.sort_by(|a, b| a.uid.cmp(&b.uid));
        layers.extend(fresh);
        page.layers = layers;
        if local.pages[i].guides != base.pages[i].guides && remote.pages[i].guides == base.pages[i].guides {
            page.guides = local.pages[i].guides.clone();
            ahead = true;
        }
    }
    let variables = pick(&base.variables, &local.variables, &remote.variables);
    let styles = pick(&base.styles, &local.styles, &remote.styles);
    let comments = pick(&base.comments, &local.comments, &remote.comments);
    ahead |= variables != remote.variables || styles != remote.styles || comments != remote.comments;
    out.variables = variables;
    out.styles = styles;
    out.comments = comments;
    for (k, v) in &local.images {
        out.images.entry(k.clone()).or_insert_with(|| v.clone());
    }
    (out, ahead)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::Page;
    use crate::geom::Rect;
    use crate::scene::Kind;

    fn frame(uid: &str, name: &str, id: NodeId) -> Tree {
        let mut t = crate::import::blank(Kind::Frame, name, Rect::new(0.0, 0.0, 100.0, 100.0));
        t.uid = Some(uid.into());
        t.saved_id = Some(id);
        t
    }

    fn doc(layers: Vec<Tree>) -> Document {
        Document::new(vec![Page { name: "Page 1".into(), layers, view: serde_json::Value::Null, guides: Vec::new() }])
    }

    #[test]
    fn edits_in_different_frames_both_survive() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        let mut local = base.clone();
        local.pages[0].layers[0].name = "A edited here".into();
        let mut remote = base.clone();
        remote.pages[0].layers[1].name = "B edited there".into();
        remote.pages[0].layers.push(frame("c", "C new there", 3));
        local.pages[0].layers.push(frame("d", "D new here", 3));
        let (m, ahead) = merge_documents(&base, &local, &remote);
        let names: Vec<&str> = m.pages[0].layers.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["A edited here", "B edited there", "C new there", "D new here"]);
        assert!(ahead, "the local edits still have to be sent");
        // "D" was id 3 here, like "C" there: it gets a fresh one.
        assert_ne!(m.pages[0].layers[3].saved_id, Some(3));
    }

    #[test]
    fn the_same_frame_edited_on_both_sides_takes_the_other_peer() {
        let base = doc(vec![frame("a", "A", 1)]);
        let mut local = base.clone();
        local.pages[0].layers[0].name = "mine".into();
        let mut remote = base.clone();
        remote.pages[0].layers[0].name = "theirs".into();
        let (m, _) = merge_documents(&base, &local, &remote);
        assert_eq!(m.pages[0].layers[0].name, "theirs");
        // A frame deleted there but edited here comes back.
        let mut remote = base.clone();
        remote.pages[0].layers.clear();
        let (m, ahead) = merge_documents(&base, &local, &remote);
        assert_eq!(m.pages[0].layers[0].name, "mine");
        assert!(ahead);
    }
}
