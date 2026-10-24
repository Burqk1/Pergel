//! Merging documents.
//!
//! Live collaboration ([`stamp_changes`], [`merge_stamped`]): every part of the document (each
//! top-level layer, each page's layer order, name and guides, the variables, styles and canvas
//! comments) carries a stamp saying who changed it last, as a Lamport clock and the peer's id.
//! When documents meet, each part is taken from the side with the later stamp. People working
//! in different frames keep all of their work; when two change the same frame at once, both end
//! up with the same one (the later stamp), and edits made while disconnected are merged on
//! rejoining instead of lost. Peers send each other only the parts that changed
//! ([`diff_parts`], [`apply_patch`]), and each person's undo reverts only their own changes
//! ([`revert_patch`]).
//!
//! Branches ([`merge_documents`]): three-way against the document the branch started from.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

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

/// Who last changed a part of a live document: a Lamport clock, then the peer's id to order
/// changes made at the same time.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Stamp(pub u64, pub String);

/// Stamps by document part (`layer:<uid>`, `order:<page>`, `page:<page>`, `pages`,
/// `variables`, `styles`, `comments`). A stamp on a part a document lacks means it was deleted.
pub type Stamps = BTreeMap<String, Stamp>;

/// One part of a live document: its value as sent between peers, and a fingerprint without the
/// layer ids that differ between peers' copies of the same content.
#[derive(Clone)]
struct Part {
    value: Value,
    print: String,
}

impl Part {
    fn of(key: &str, value: Value) -> Part {
        let print = if key.starts_with("layer:") {
            // {page, tree}: compared without ids.
            let tree = serde_json::from_value::<Tree>(value["tree"].clone()).map(|t| plain(&t));
            json(&(&value["page"], tree.ok()))
        } else {
            json(&value)
        };
        Part { value, print }
    }

    /// A top-level layer, fingerprinted from the tree itself.
    fn layer(page: usize, tree: &Tree) -> Part {
        Part { value: json!({ "page": page, "tree": tree }), print: json(&(page, plain(tree))) }
    }
}

type Parts = BTreeMap<String, Part>;

fn json<T: Serialize + ?Sized>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// A live document taken apart.
fn split(doc: &Document) -> Parts {
    let mut out = Parts::new();
    let mut out_layers = Vec::new();
    let mut put = |key: String, value: Value| {
        let part = Part::of(&key, value);
        out.insert(key, part);
    };
    put("pages".into(), json!(doc.pages.len()));
    for (i, page) in doc.pages.iter().enumerate() {
        put(format!("page:{i}"), json!({ "name": page.name, "guides": page.guides }));
        let order: Vec<&str> = page.layers.iter().filter_map(|t| t.uid.as_deref()).collect();
        put(format!("order:{i}"), json!(order));
        for t in &page.layers {
            if let Some(uid) = &t.uid {
                out_layers.push((format!("layer:{uid}"), Part::layer(i, t)));
            }
        }
    }
    put("variables".into(), json!(doc.variables));
    put("styles".into(), json!(doc.styles));
    put("comments".into(), json!(doc.comments));
    out.extend(out_layers);
    out
}

/// A document from its parts. `base` gives what isn't shared: this peer's views of the pages,
/// the branch info and the images (to which `images` are added).
fn join(parts: &Parts, base: &Document, images: &BTreeMap<String, String>) -> Document {
    let value = |key: &str| parts.get(key).map(|p| &p.value);
    let count = value("pages").and_then(Value::as_u64).unwrap_or(0) as usize;
    let mut out = base.clone();
    out.pages = (0..count)
        .map(|i| {
            let info = value(&format!("page:{i}"));
            crate::document::Page {
                name: info.and_then(|v| v["name"].as_str()).map_or_else(|| format!("Page {}", i + 1), str::to_string),
                layers: Vec::new(),
                view: base.pages.get(i).map_or(Value::Null, |p| p.view.clone()),
                guides: info.and_then(|v| serde_json::from_value(v["guides"].clone()).ok()).unwrap_or_default(),
            }
        })
        .collect();
    let mut placed: Vec<BTreeMap<&str, Tree>> = vec![BTreeMap::new(); count];
    for (key, part) in parts {
        let Some(uid) = key.strip_prefix("layer:") else { continue };
        let page = part.value["page"].as_u64().unwrap_or(u64::MAX) as usize;
        if let (Some(slot), Ok(tree)) = (placed.get_mut(page), serde_json::from_value::<Tree>(part.value["tree"].clone())) {
            slot.insert(uid, tree);
        }
    }
    for (i, page) in out.pages.iter_mut().enumerate() {
        // The page's order; layers it doesn't list go on top, by uid.
        let order: Vec<String> = value(&format!("order:{i}")).and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
        let here = &mut placed[i];
        for uid in &order {
            if let Some(t) = here.remove(uid.as_str()) {
                page.layers.push(t);
            }
        }
        page.layers.extend(std::mem::take(here).into_values());
    }
    out.variables = value("variables").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    out.styles = value("styles").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    out.comments = value("comments").and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default();
    for (k, v) in images {
        out.images.entry(k.clone()).or_insert_with(|| v.clone());
    }
    out
}

/// Gives the layer trees under `fresh` keys new ids where they collide with the other parts'.
fn renumber_parts(parts: &mut Parts, fresh: &BTreeSet<String>) {
    let mut taken: HashSet<NodeId> = HashSet::new();
    for (key, part) in parts.iter() {
        if key.starts_with("layer:")
            && !fresh.contains(key)
            && let Ok(t) = serde_json::from_value::<Tree>(part.value["tree"].clone())
        {
            ids(&t, &mut taken);
        }
    }
    let mut next: NodeId = taken.iter().copied().max().unwrap_or(0).max(10_000_000);
    for key in fresh {
        let Some(part) = parts.get_mut(key) else { continue };
        let Ok(mut t) = serde_json::from_value::<Tree>(part.value["tree"].clone()) else { continue };
        let mut mine = HashSet::new();
        ids(&t, &mut mine);
        if mine.is_disjoint(&taken) {
            taken.extend(mine);
            continue;
        }
        renumber(&mut t, &taken, &mut next);
        ids(&t, &mut taken);
        part.value["tree"] = json!(t);
    }
}

/// The document's images whose hash appears in `parts`' values.
fn images_used(doc: &Document, parts: &[&Value]) -> BTreeMap<String, String> {
    let text: String = parts.iter().map(json).collect();
    doc.images.iter().filter(|(hash, _)| text.contains(hash.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect()
}

fn same_part(a: Option<&Part>, b: Option<&Part>) -> bool {
    a.map(|p| &p.print) == b.map(|p| &p.print)
}

/// Stamps every part that changed from `before` to `after` as this peer's newest change.
pub fn stamp_changes(before: &Document, after: &Document, stamps: &mut Stamps, peer: &str) {
    let (a, b) = (split(before), split(after));
    let clock = stamps.values().map(|s| s.0).max().unwrap_or(0) + 1;
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        if !same_part(a.get(k), b.get(k)) {
            stamps.insert(k.clone(), Stamp(clock, peer.to_string()));
        }
    }
}

/// The later of two stamp sets, part by part.
fn newest(a: &Stamps, b: &Stamps) -> Stamps {
    let mut out = a.clone();
    for (k, s) in b {
        if out.get(k).is_none_or(|r| s > r) {
            out.insert(k.clone(), s.clone());
        }
    }
    out
}

/// Merges two peers' live documents part by part, each from the side with the later stamp.
/// Both peers get the same result whichever side is "local". Returns the document, the merged
/// stamps, and whether the result has changes `remote` lacks (so it should be sent on).
pub fn merge_stamped(local: &Document, local_stamps: &Stamps, remote: &Document, remote_stamps: &Stamps) -> (Document, Stamps, bool) {
    let (lp, rp) = (split(local), split(remote));
    let mut out = Parts::new();
    let mut from_local = BTreeSet::new();
    let keys: BTreeSet<&String> = lp.keys().chain(rp.keys()).collect();
    for k in keys {
        let mine = local_stamps.get(k) > remote_stamps.get(k);
        if let Some(p) = if mine { lp.get(k) } else { rp.get(k) } {
            out.insert(k.clone(), p.clone());
            if mine {
                from_local.insert(k.clone());
            }
        }
    }
    renumber_parts(&mut out, &from_local);
    let ahead = out.len() != rp.len() || out.iter().any(|(k, p)| !same_part(Some(p), rp.get(k)));
    (join(&out, local, &remote.images), newest(local_stamps, remote_stamps), ahead)
}

/// What changed from one document to the next, part by part: sent between peers instead of
/// whole documents, and kept for undo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Patch {
    /// Each changed part as it was before and after (None: absent).
    pub parts: BTreeMap<String, (Option<Value>, Option<Value>)>,
    /// The parts' stamps (when sent between peers).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub stamps: Stamps,
    /// Images the parts use.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub images: BTreeMap<String, String>,
}

impl Patch {
    pub fn is_empty(&self) -> bool {
        self.parts.is_empty()
    }
}

/// The parts that differ from `before` to `after`, with their stamps from `stamps`. Without
/// `with_before` only the new values are kept (enough to send to other peers).
pub fn diff_parts(before: &Document, after: &Document, stamps: &Stamps, with_before: bool) -> Patch {
    let (a, b) = (split(before), split(after));
    let mut patch = Patch::default();
    let keys: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    for k in keys {
        if !same_part(a.get(k), b.get(k)) {
            let was = if with_before { a.get(k).map(|p| p.value.clone()) } else { None };
            patch.parts.insert(k.clone(), (was, b.get(k).map(|p| p.value.clone())));
            if let Some(s) = stamps.get(k) {
                patch.stamps.insert(k.clone(), s.clone());
            }
        }
    }
    let values: Vec<&Value> = patch.parts.values().flat_map(|(x, y)| x.iter().chain(y.iter())).collect();
    patch.images = images_used(before, &values);
    patch.images.extend(images_used(after, &values));
    patch
}

/// Another peer's changes applied to this peer's document: each part whose stamp is later than
/// this peer's. Returns the document and the merged stamps.
pub fn apply_patch(local: &Document, local_stamps: &Stamps, patch: &Patch) -> (Document, Stamps) {
    let mut parts = split(local);
    let mut fresh = BTreeSet::new();
    for (k, (_, after)) in &patch.parts {
        if patch.stamps.get(k) <= local_stamps.get(k) {
            continue;
        }
        match after {
            Some(v) => {
                parts.insert(k.clone(), Part::of(k, v.clone()));
                fresh.insert(k.clone());
            }
            None => {
                parts.remove(k);
            }
        }
    }
    renumber_parts(&mut parts, &fresh);
    (join(&parts, local, &patch.images), newest(local_stamps, &patch.stamps))
}

/// Undoes (or with `redo`, re-applies) this peer's change `patch` in `current`: only the parts
/// still as the change left them, so later changes by others stay. Returns the document and how
/// many parts it changed.
pub fn revert_patch(current: &Document, patch: &Patch, redo: bool) -> (Document, usize) {
    let mut parts = split(current);
    let mut fresh = BTreeSet::new();
    let mut n = 0;
    for (k, (before, after)) in &patch.parts {
        let (expect, target) = if redo { (before, after) } else { (after, before) };
        let expect = expect.as_ref().map(|v| Part::of(k, v.clone()));
        if !same_part(parts.get(k), expect.as_ref()) {
            continue;
        }
        match target {
            Some(v) => {
                parts.insert(k.clone(), Part::of(k, v.clone()));
                fresh.insert(k.clone());
            }
            None => {
                parts.remove(k);
            }
        }
        n += 1;
    }
    // A layer that comes back on a page whose order others changed since goes after the layer it
    // followed before, not on top.
    for k in &fresh {
        let Some(uid) = k.strip_prefix("layer:") else { continue };
        let page = parts[k].value["page"].as_u64().unwrap_or(0);
        let key = format!("order:{page}");
        let list = |v: Option<&Value>| -> Vec<String> { v.and_then(|v| serde_json::from_value(v.clone()).ok()).unwrap_or_default() };
        let mut order = list(parts.get(&key).map(|p| &p.value));
        if order.iter().any(|u| u == uid) {
            continue;
        }
        let was = patch.parts.get(&key).map(|(b, a)| list(if redo { a.as_ref() } else { b.as_ref() })).unwrap_or_default();
        let Some(at) = was.iter().position(|u| u == uid) else { continue };
        let after = was[..at].iter().rev().find_map(|prev| order.iter().position(|u| u == prev));
        order.insert(after.map_or(0, |i| i + 1), uid.to_string());
        parts.insert(key.clone(), Part::of(&key, json!(order)));
    }
    renumber_parts(&mut parts, &fresh);
    (join(&parts, current, &patch.images), n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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

    /// Two peers editing from `base`: each stamps its own edits, then both merge the other's.
    fn exchange(base: &Document, a: &Document, b: &Document) -> (Document, Document) {
        let (mut sa, mut sb) = (Stamps::new(), Stamps::new());
        stamp_changes(base, a, &mut sa, "peer-a");
        stamp_changes(base, b, &mut sb, "peer-b");
        let (at_a, _, _) = merge_stamped(a, &sa, b, &sb);
        let (at_b, _, _) = merge_stamped(b, &sb, a, &sa);
        (at_a, at_b)
    }

    fn names(d: &Document) -> Vec<&str> {
        d.pages[0].layers.iter().map(|t| t.name.as_str()).collect()
    }

    #[test]
    fn live_peers_editing_the_same_frame_end_up_with_the_same_one() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        let mut a = base.clone();
        a.pages[0].layers[0].name = "A by peer a".into();
        let mut b = base.clone();
        b.pages[0].layers[0].name = "A by peer b".into();
        b.pages[0].layers[1].name = "B by peer b".into();
        let (at_a, at_b) = exchange(&base, &a, &b);
        assert_eq!(names(&at_a), names(&at_b), "both peers converge");
        // Same clock, so the peer id decides; peer b's other edit survives as well.
        assert_eq!(names(&at_a), ["A by peer b", "B by peer b"]);
    }

    #[test]
    fn live_edits_in_different_frames_and_new_layers_all_survive() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        let mut a = base.clone();
        a.pages[0].layers[0].name = "A edited".into();
        a.pages[0].layers.push(frame("a-1", "New at a", 3));
        let mut b = base.clone();
        b.pages[0].layers[1].name = "B edited".into();
        b.pages[0].layers.push(frame("b-1", "New at b", 3));
        let (at_a, at_b) = exchange(&base, &a, &b);
        assert_eq!(names(&at_a), names(&at_b));
        // Peer b's layer order wins the tie; peer a's new layer goes on top of it.
        assert_eq!(names(&at_a), ["A edited", "B edited", "New at b", "New at a"]);
        let ids: HashSet<Option<NodeId>> = at_a.pages[0].layers.iter().map(|t| t.saved_id).collect();
        assert_eq!(ids.len(), 4, "the two new layers don't share an id");
    }

    #[test]
    fn a_later_change_wins_and_deletions_carry_over() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        let (mut sa, sb) = (Stamps::new(), Stamps::new());
        // Peer a renames A and the change reaches b.
        let mut a = base.clone();
        a.pages[0].layers[0].name = "A1".into();
        stamp_changes(&base, &a, &mut sa, "peer-a");
        let (b, mut sb, ahead) = merge_stamped(&base, &sb, &a, &sa);
        assert!(!ahead);
        // Peer b, now knowing a's stamp, renames A again and deletes B: later stamps win at a.
        let mut b2 = b.clone();
        b2.pages[0].layers[0].name = "A2".into();
        b2.pages[0].layers.remove(1);
        stamp_changes(&b, &b2, &mut sb, "peer-b");
        let (at_a, _, ahead) = merge_stamped(&a, &sa, &b2, &sb);
        assert_eq!(names(&at_a), ["A2"]);
        assert!(!ahead, "nothing for a to send back");
    }

    #[test]
    fn edits_made_while_disconnected_are_merged_on_rejoining() {
        let base = doc(vec![frame("a", "A", 1)]);
        let (mut sa, mut sb) = (Stamps::new(), Stamps::new());
        let mut offline = base.clone();
        offline.pages[0].layers.push(frame("a-1", "Drawn offline", 2));
        stamp_changes(&base, &offline, &mut sa, "peer-a");
        let mut meanwhile = base.clone();
        meanwhile.pages[0].layers[0].name = "A renamed meanwhile".into();
        stamp_changes(&base, &meanwhile, &mut sb, "peer-b");
        let (rejoined, _, ahead) = merge_stamped(&offline, &sa, &meanwhile, &sb);
        assert_eq!(names(&rejoined), ["A renamed meanwhile", "Drawn offline"]);
        assert!(ahead, "the offline work goes out to the others");
    }

    #[test]
    fn peers_send_only_what_changed() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        let mut sa = Stamps::new();
        let mut a = base.clone();
        a.pages[0].layers[0].name = "A edited".into();
        stamp_changes(&base, &a, &mut sa, "peer-a");
        let patch = diff_parts(&base, &a, &sa, false);
        assert_eq!(patch.parts.keys().collect::<Vec<_>>(), ["layer:a"]);
        assert!(patch.parts["layer:a"].0.is_none(), "no old value on the wire");
        let (at_b, sb) = apply_patch(&base, &Stamps::new(), &patch);
        assert_eq!(names(&at_b), ["A edited", "B"]);
        // An older change than the one this peer has is ignored.
        let mut stale = patch.clone();
        stale.parts.get_mut("layer:a").unwrap().1 = Some(json!({ "page": 0, "tree": frame("a", "Old", 1) }));
        stale.stamps.insert("layer:a".into(), Stamp(0, "peer-z".into()));
        assert_eq!(names(&apply_patch(&at_b, &sb, &stale).0), ["A edited", "B"]);
    }

    #[test]
    fn undo_reverts_only_this_peers_own_change() {
        let base = doc(vec![frame("a", "A", 1), frame("b", "B", 2)]);
        // This peer renames A and deletes B.
        let mut mine = base.clone();
        mine.pages[0].layers[0].name = "A mine".into();
        mine.pages[0].layers.remove(1);
        let change = diff_parts(&base, &mine, &Stamps::new(), true);
        // Meanwhile someone adds C.
        let mut now = mine.clone();
        now.pages[0].layers.push(frame("c", "C theirs", 3));
        let (undone, n) = revert_patch(&now, &change, false);
        assert_eq!(names(&undone), ["A", "B", "C theirs"], "B is back in place and C stays");
        assert!(n >= 2);
        let (redone, _) = revert_patch(&undone, &change, true);
        assert_eq!(names(&redone), ["A mine", "C theirs"]);
        // Once someone else renamed A again, undoing this peer's rename leaves theirs.
        let mut later = mine.clone();
        later.pages[0].layers[0].name = "A theirs".into();
        let (undone, _) = revert_patch(&later, &change, false);
        assert_eq!(names(&undone)[0], "A theirs");
    }
}
