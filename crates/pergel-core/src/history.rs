//! Undo/redo as before/after snapshots of only the nodes a transaction touched, so a step costs
//! memory proportional to what changed, not to the document.

use std::collections::HashMap;

use crate::scene::{Node, NodeId, Scene};

const LIMIT: usize = 200;

/// One open step. A step opened inside another is its own level, so it can be thrown away
/// alone ([`Scene::abort`]) while the outer step goes on.
struct Level {
    /// Each layer as it was when this level first touched it.
    nodes: HashMap<NodeId, Node>,
    roots: Option<Vec<NodeId>>,
    selection: Vec<NodeId>,
}

impl Level {
    fn new(selection: &[NodeId]) -> Self {
        Self { nodes: HashMap::new(), roots: None, selection: selection.to_vec() }
    }
}

/// The open levels, outermost first.
struct Recording {
    levels: Vec<Level>,
}

struct Transaction {
    nodes: Vec<(Node, Node)>,
    roots: Option<(Vec<NodeId>, Vec<NodeId>)>,
    selection: (Vec<NodeId>, Vec<NodeId>),
}

#[derive(Default)]
pub(crate) struct History {
    recording: Option<Recording>,
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
}

impl Scene {
    /// Opens an undo step. Nested calls join the outer step, so a drag and the
    /// reparenting at its end undo together.
    pub fn begin(&mut self, selection: &[NodeId]) {
        match &mut self.history.recording {
            Some(r) => r.levels.push(Level::new(selection)),
            None => self.history.recording = Some(Recording { levels: vec![Level::new(selection)] }),
        }
    }

    /// Closes the step opened by [`Scene::begin`]. Returns whether anything changed.
    pub fn commit(&mut self, selection: &[NodeId]) -> bool {
        let Some(r) = &mut self.history.recording else { return false };
        // An inner level joins the outer one, which already holds every layer it touched.
        if r.levels.len() > 1 {
            r.levels.pop();
            return false;
        }
        let Some(r) = self.history.recording.take().and_then(|mut r| r.levels.pop()) else { return false };
        let mut nodes: Vec<(Node, Node)> = r
            .nodes
            .into_values()
            .filter_map(|before| {
                let after = self.nodes[before.id as usize].clone();
                (after != before).then_some((before, after))
            })
            .collect();
        nodes.sort_unstable_by_key(|(n, _)| n.id);
        let roots = r.roots.filter(|before| *before != self.roots).map(|before| (before, self.roots.clone()));
        if nodes.is_empty() && roots.is_none() {
            return false;
        }
        self.history.undo.push(Transaction { nodes, roots, selection: (r.selection, selection.to_vec()) });
        if self.history.undo.len() > LIMIT {
            self.history.undo.remove(0);
        }
        self.history.redo.clear();
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.history.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.history.redo.is_empty()
    }

    /// Reverts the last step and returns the selection it started with.
    pub fn undo(&mut self) -> Option<Vec<NodeId>> {
        self.history.recording = None;
        let t = self.history.undo.pop()?;
        for (before, _) in &t.nodes {
            self.put(before.clone());
        }
        if let Some((before, _)) = &t.roots {
            self.put_roots(before.clone());
        }
        self.restored();
        let selection = t.selection.0.clone();
        self.history.redo.push(t);
        Some(selection)
    }

    /// Re-applies the last undone step and returns the selection it ended with.
    pub fn redo(&mut self) -> Option<Vec<NodeId>> {
        self.history.recording = None;
        let t = self.history.redo.pop()?;
        for (_, after) in &t.nodes {
            self.put(after.clone());
        }
        if let Some((_, after)) = &t.roots {
            self.put_roots(after.clone());
        }
        self.restored();
        let selection = t.selection.1.clone();
        self.history.undo.push(t);
        Some(selection)
    }

    /// Throws the open step away: every layer it touched (and the top-level order) goes back to
    /// how it was, and nothing reaches the undo stack. For previews such as the presentation
    /// view's variant changes.
    pub fn rollback(&mut self) {
        let Some(r) = self.history.recording.take().and_then(|mut r| (!r.levels.is_empty()).then(|| r.levels.swap_remove(0))) else { return };
        for (_, before) in r.nodes {
            self.put(before);
        }
        if let Some(roots) = r.roots {
            self.put_roots(roots);
        }
        self.restored();
    }

    /// Throws away only the innermost open step (the whole step when it is the only one) and
    /// returns the selection it started with. A failed agent call inside a script undoes its own
    /// changes and leaves the script's earlier ones.
    pub fn abort(&mut self) -> Option<Vec<NodeId>> {
        let r = self.history.recording.as_mut()?;
        let level = r.levels.pop()?;
        if r.levels.is_empty() {
            self.history.recording = None;
        }
        for (_, before) in level.nodes {
            self.put(before);
        }
        if let Some(roots) = level.roots {
            self.put_roots(roots);
        }
        self.restored();
        Some(level.selection)
    }

    /// Layers changed so far in the open undo step.
    pub fn touched(&self) -> Vec<NodeId> {
        self.history.recording.as_ref().and_then(|r| r.levels.first()).map_or(Vec::new(), |l| l.nodes.keys().copied().collect())
    }

    pub(crate) fn touch(&mut self, id: NodeId) {
        if let Some(r) = &mut self.history.recording {
            for level in &mut r.levels {
                level.nodes.entry(id).or_insert_with(|| self.nodes[id as usize].clone());
            }
        }
    }

    pub(crate) fn touch_roots(&mut self) {
        if let Some(r) = &mut self.history.recording {
            for level in r.levels.iter_mut().filter(|l| l.roots.is_none()) {
                level.roots = Some(self.roots.clone());
            }
        }
    }

    /// A node created inside a step is recorded as deleted, so undo removes it again.
    pub(crate) fn record_new(&mut self, node: &Node) {
        if let Some(r) = &mut self.history.recording {
            let mut before = node.clone();
            before.deleted = true;
            for level in &mut r.levels {
                level.nodes.entry(node.id).or_insert_with(|| before.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::scene::tests::{frame, rect};
    use crate::{Kind, Rect, Scene};

    #[test]
    fn undo_and_redo_restore_structure_geometry_and_selection() {
        let mut scene = Scene::default();
        let f = scene.add(frame("Frame", 0.0, 0.0, 100.0, 100.0));
        let a = scene.insert(rect("A", 10.0, 10.0, 10.0, 10.0), Some(f), None);
        let b = scene.insert(rect("B", 40.0, 10.0, 10.0, 10.0), Some(f), None);

        scene.begin(&[a, b]);
        let g = scene.group(&[a, b], Kind::Group).unwrap();
        scene.translate(g, 5.0, 5.0);
        assert!(scene.commit(&[g]));

        scene.begin(&[g]);
        scene.update(a, |n| n.name = "Renamed".into());
        scene.remove(b);
        scene.commit(&[]);
        assert!(scene.node(b).is_none());

        assert_eq!(scene.undo(), Some(vec![g]));
        assert_eq!(scene.node(a).unwrap().name, "A");
        assert_eq!(scene.node(b).unwrap().bounds, Rect::new(45.0, 15.0, 10.0, 10.0));
        assert_eq!(scene.undo(), Some(vec![a, b]));
        assert!(scene.node(g).is_none());
        assert_eq!(scene.node(a).unwrap().parent, Some(f));
        assert_eq!(scene.node(f).unwrap().children, vec![a, b]);
        assert_eq!(scene.pick(12.0, 12.0, false, &[]), Some(a));
        assert_eq!(scene.undo(), None);

        assert_eq!(scene.redo(), Some(vec![g]));
        assert_eq!(scene.node(g).unwrap().bounds, Rect::new(15.0, 15.0, 40.0, 10.0));
        assert_eq!(scene.pick(16.0, 16.0, false, &[]), Some(g));
        assert_eq!(scene.redo(), Some(vec![]));
        assert!(scene.node(b).is_none());
        assert!(!scene.can_redo());
    }

    #[test]
    fn nested_steps_merge_and_empty_steps_are_dropped() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 0.0, 0.0, 10.0, 10.0));
        scene.begin(&[]);
        scene.begin(&[]);
        scene.translate(a, 1.0, 0.0);
        assert!(!scene.commit(&[]));
        scene.translate(a, 1.0, 0.0);
        assert!(scene.commit(&[]));
        scene.begin(&[]);
        assert!(!scene.commit(&[]));
        scene.undo();
        assert_eq!(scene.node(a).unwrap().bounds.x, 0.0);
        assert!(!scene.can_undo());
    }

    #[test]
    fn abort_throws_away_only_the_innermost_step() {
        let mut scene = Scene::default();
        let a = scene.add(rect("A", 0.0, 0.0, 10.0, 10.0));
        let b = scene.add(rect("B", 50.0, 0.0, 10.0, 10.0));
        scene.begin(&[a]);
        scene.translate(a, 5.0, 0.0);
        // A nested step that fails: it moved A again, renamed B and added C.
        scene.begin(&[b]);
        scene.translate(a, 5.0, 0.0);
        scene.update(b, |n| n.name = "Renamed".into());
        let c = scene.add(rect("C", 90.0, 0.0, 10.0, 10.0));
        assert_eq!(scene.abort(), Some(vec![b]));
        assert_eq!(scene.node(a).unwrap().bounds.x, 5.0, "the outer step's move stays");
        assert_eq!(scene.node(b).unwrap().name, "B");
        assert!(scene.node(c).is_none());
        assert!(scene.commit(&[a]));
        scene.undo();
        assert_eq!(scene.node(a).unwrap().bounds.x, 0.0, "undo reverts the outer step as one");
        assert!(!scene.can_undo());

        // Aborting the only step reverts it and leaves no undo entry.
        scene.begin(&[]);
        scene.update(a, |n| n.name = "Gone".into());
        assert_eq!(scene.abort(), Some(vec![]));
        assert_eq!(scene.node(a).unwrap().name, "A");
        assert!(!scene.can_undo());
        assert_eq!(scene.abort(), None);
    }
}
