//! Mermaid flowcharts to layers (Figma's `generate_diagram`): parses `graph`/`flowchart`
//! source, lays the nodes out in ranks (longest path, barycentre ordering) and returns one
//! node spec — a frame holding the shapes, their labels and the connecting arrows.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::agent::NodeSpec;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Shape {
    Rect,
    Round,
    Stadium,
    Circle,
    Diamond,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Line {
    Solid,
    Dotted,
    Thick,
}

#[derive(Debug, Clone)]
struct DNode {
    id: String,
    label: String,
    shape: Shape,
}

#[derive(Debug, Clone)]
struct DEdge {
    from: usize,
    to: usize,
    label: Option<String>,
    line: Line,
    arrow: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Dir {
    Down,
    Up,
    Right,
    Left,
}

#[derive(Default)]
struct Graph {
    nodes: Vec<DNode>,
    edges: Vec<DEdge>,
    index: HashMap<String, usize>,
}

impl Graph {
    fn node(&mut self, id: &str, label: Option<String>, shape: Option<Shape>) -> usize {
        if let Some(&i) = self.index.get(id) {
            if let Some(l) = label {
                self.nodes[i].label = l;
            }
            if let Some(s) = shape {
                self.nodes[i].shape = s;
            }
            return i;
        }
        self.nodes.push(DNode { id: id.to_string(), label: label.unwrap_or_else(|| id.to_string()), shape: shape.unwrap_or(Shape::Rect) });
        self.index.insert(id.to_string(), self.nodes.len() - 1);
        self.nodes.len() - 1
    }
}

struct Scanner<'a> {
    s: &'a str,
    i: usize,
}

impl<'a> Scanner<'a> {
    fn rest(&self) -> &'a str {
        &self.s[self.i..]
    }

    fn ws(&mut self) {
        while let Some(c) = self.rest().chars().next() {
            if c == ' ' || c == '\t' {
                self.i += c.len_utf8();
            } else {
                break;
            }
        }
    }

    fn eat(&mut self, t: &str) -> bool {
        if self.rest().starts_with(t) {
            self.i += t.len();
            true
        } else {
            false
        }
    }

    fn ident(&mut self) -> Option<&'a str> {
        let start = self.i;
        while let Some(c) = self.rest().chars().next() {
            if c.is_alphanumeric() || c == '_' || c == '.' && !self.rest().starts_with(".-") {
                self.i += c.len_utf8();
            } else if c == '-' && !self.rest().starts_with("--") && !self.rest().starts_with("-.") && self.i > start {
                self.i += 1;
            } else {
                break;
            }
        }
        (self.i > start).then(|| &self.s[start..self.i])
    }

    /// Text up to `close`, without quotes.
    fn until(&mut self, close: &str) -> Option<String> {
        let at = self.rest().find(close)?;
        let text = self.rest()[..at].trim().trim_matches('"').to_string();
        self.i += at + close.len();
        Some(text.replace("<br>", "\n").replace("<br/>", "\n").replace("\\n", "\n"))
    }

    fn shape(&mut self) -> Option<(String, Shape)> {
        const SHAPES: [(&str, &str, Shape); 10] = [
            ("([", "])", Shape::Stadium),
            ("((", "))", Shape::Circle),
            ("[(", ")]", Shape::Round),
            ("[[", "]]", Shape::Rect),
            ("{{", "}}", Shape::Diamond),
            ("[/", "/]", Shape::Rect),
            ("[", "]", Shape::Rect),
            ("(", ")", Shape::Round),
            ("{", "}", Shape::Diamond),
            (">", "]", Shape::Rect),
        ];
        for (open, close, shape) in SHAPES {
            if self.rest().starts_with(open) {
                let save = self.i;
                self.i += open.len();
                match self.until(close) {
                    Some(label) => return Some((label, shape)),
                    None => self.i = save,
                }
            }
        }
        None
    }

    /// A link: `-->`, `---`, `-.->`, `==>`, with an optional `-- text -->` or `|text|` label.
    fn link(&mut self) -> Option<(Option<String>, Line, bool)> {
        let start = self.i;
        let run: String = self.rest().chars().take_while(|c| matches!(c, '-' | '.' | '=' | '>' | 'o' | 'x' | '<')).collect();
        // `o`/`x` only count as arrowheads right after the dashes.
        let run = run.trim_end_matches(|c: char| c.is_alphabetic() && c != 'o' && c != 'x').to_string();
        if run.len() < 2 || !(run.starts_with("--") || run.starts_with("-.") || run.starts_with("==") || run.starts_with("<-")) {
            return None;
        }
        let mut run = run;
        // "A --o B" vs a node named "o…": only the first o/x after dashes is an end.
        if let Some(p) = run.find(['o', 'x']) {
            run.truncate(p + 1);
        }
        self.i += run.len();
        let mut label = None;
        let mut full = run.clone();
        if matches!(run.as_str(), "--" | "-." | "==") {
            // Labelled form: "-- text -->".
            let closes = ["-->", "---", ".->", ".-", "==>", "==="];
            let rest = self.rest();
            let found = closes.iter().filter_map(|c| rest.find(c).map(|at| (at, *c))).min_by_key(|x| x.0);
            let Some((at, close)) = found else {
                self.i = start;
                return None;
            };
            label = Some(rest[..at].trim().trim_matches('"').to_string());
            self.i += at + close.len();
            full.push_str(close);
        }
        self.ws();
        if self.eat("|") {
            label = self.until("|");
        }
        let line = if full.contains('.') {
            Line::Dotted
        } else if full.contains('=') {
            Line::Thick
        } else {
            Line::Solid
        };
        let arrow = full.ends_with('>') || full.ends_with('o') || full.ends_with('x');
        Some((label.filter(|l| !l.is_empty()), line, arrow))
    }
}

fn parse(src: &str) -> Result<(Graph, Dir), String> {
    let mut g = Graph::default();
    let mut dir = Dir::Down;
    for raw in src.lines().flat_map(|l| l.split(';')) {
        let line = raw.trim();
        let lower = line.to_ascii_lowercase();
        if line.is_empty() || line.starts_with("%%") {
            continue;
        }
        if lower.starts_with("graph") || lower.starts_with("flowchart") {
            dir = match lower.split_whitespace().nth(1).unwrap_or("td") {
                "lr" => Dir::Right,
                "rl" => Dir::Left,
                "bt" => Dir::Up,
                _ => Dir::Down,
            };
            continue;
        }
        if ["subgraph", "end", "classdef", "class ", "style ", "linkstyle", "click ", "direction"].iter().any(|k| lower.starts_with(k)) {
            continue;
        }
        let mut sc = Scanner { s: line, i: 0 };
        let mut prev: Vec<usize> = Vec::new();
        let mut pending: Option<(Option<String>, Line, bool)> = None;
        loop {
            sc.ws();
            // One or more nodes joined with `&`.
            let mut group = Vec::new();
            loop {
                sc.ws();
                let Some(id) = sc.ident() else { break };
                let shaped = sc.shape();
                let (label, shape) = shaped.map_or((None, None), |(l, s)| (Some(l), Some(s)));
                group.push(g.node(id, label, shape));
                sc.ws();
                // Mermaid's `:::class` styling suffix.
                if sc.eat(":::") {
                    sc.ident();
                }
                sc.ws();
                if !sc.eat("&") {
                    break;
                }
            }
            if group.is_empty() {
                break;
            }
            if let Some((label, line, arrow)) = pending.take() {
                for &a in &prev {
                    for &b in &group {
                        g.edges.push(DEdge { from: a, to: b, label: label.clone(), line, arrow });
                    }
                }
            }
            prev = group;
            sc.ws();
            match sc.link() {
                Some(l) => pending = Some(l),
                None => break,
            }
        }
    }
    if g.nodes.is_empty() {
        return Err("No nodes found. Use Mermaid flowchart syntax, e.g. \"graph TD; A[Start] --> B{OK?}; B -->|yes| C[Done]\"".into());
    }
    Ok((g, dir))
}

const FONT: f32 = 14.0;
const RANK_GAP: f32 = 64.0;
const NODE_GAP: f32 = 48.0;
const MARGIN: f32 = 40.0;

fn size_of(n: &DNode) -> (f32, f32) {
    let lines: Vec<&str> = n.label.lines().collect();
    let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(1) as f32;
    let text_w = longest * FONT * 0.56;
    let text_h = lines.len().max(1) as f32 * FONT * 1.4;
    let w = (text_w + 40.0).clamp(96.0, 280.0);
    let h = (text_h + 24.0).max(44.0);
    match n.shape {
        Shape::Diamond => ((w * 1.35).max(120.0), (h * 1.6).max(72.0)),
        Shape::Circle => {
            let d = w.max(h).max(72.0);
            (d, d)
        }
        _ => (w, h),
    }
}

/// Ranks by longest path from the sources, ignoring edges that close a cycle.
fn ranks(g: &Graph) -> (Vec<usize>, Vec<bool>) {
    let n = g.nodes.len();
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, e) in g.edges.iter().enumerate() {
        out[e.from].push(k);
    }
    // DFS for back edges.
    let mut back = vec![false; g.edges.len()];
    let mut state = vec![0u8; n];
    fn dfs(v: usize, g: &Graph, out: &[Vec<usize>], state: &mut [u8], back: &mut [bool]) {
        state[v] = 1;
        for &k in &out[v] {
            let t = g.edges[k].to;
            match state[t] {
                0 => dfs(t, g, out, state, back),
                1 => back[k] = true,
                _ => {}
            }
        }
        state[v] = 2;
    }
    for v in 0..n {
        if state[v] == 0 {
            dfs(v, g, &out, &mut state, &mut back);
        }
    }
    let mut rank = vec![0usize; n];
    // Relax forward edges until stable (n passes bound the longest path).
    for _ in 0..n {
        let mut changed = false;
        for (k, e) in g.edges.iter().enumerate() {
            if !back[k] && e.from != e.to && rank[e.to] < rank[e.from] + 1 {
                rank[e.to] = rank[e.from] + 1;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    (rank, back)
}

/// The diagram as one frame spec. `name` names the frame.
pub fn mermaid_spec(src: &str, name: &str) -> Result<NodeSpec, String> {
    let (g, dir) = parse(src)?;
    let (rank, back) = ranks(&g);
    let levels = rank.iter().copied().max().unwrap_or(0) + 1;
    let mut rows: Vec<Vec<usize>> = vec![Vec::new(); levels];
    for (v, r) in rank.iter().enumerate() {
        rows[*r].push(v);
    }
    // Barycentre sweeps: order each rank by the mean position of its neighbours.
    let mut pos = vec![0.0f32; g.nodes.len()];
    let place = |rows: &Vec<Vec<usize>>, pos: &mut Vec<f32>| {
        for row in rows {
            for (i, v) in row.iter().enumerate() {
                pos[*v] = i as f32;
            }
        }
    };
    place(&rows, &mut pos);
    for sweep in 0..4 {
        let down = sweep % 2 == 0;
        let order: Vec<usize> = if down { (1..levels).collect() } else { (0..levels.saturating_sub(1)).rev().collect() };
        for r in order {
            let mut keyed: Vec<(f32, usize)> = rows[r]
                .iter()
                .map(|&v| {
                    let ns: Vec<f32> = g
                        .edges
                        .iter()
                        .filter_map(|e| {
                            if down && e.to == v && rank[e.from] + 1 == r {
                                Some(pos[e.from])
                            } else if !down && e.from == v && rank[e.to] == r + 1 {
                                Some(pos[e.to])
                            } else {
                                None
                            }
                        })
                        .collect();
                    let key = if ns.is_empty() { pos[v] } else { ns.iter().sum::<f32>() / ns.len() as f32 };
                    (key, v)
                })
                .collect();
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
            rows[r] = keyed.into_iter().map(|k| k.1).collect();
            place(&rows, &mut pos);
        }
    }
    let sizes: Vec<(f32, f32)> = g.nodes.iter().map(size_of).collect();
    let vertical = matches!(dir, Dir::Down | Dir::Up);
    // Main axis: ranks; cross axis: order inside a rank (centred on the widest rank).
    let thick = |v: usize| if vertical { sizes[v].1 } else { sizes[v].0 };
    let wide = |v: usize| if vertical { sizes[v].0 } else { sizes[v].1 };
    let rank_size: Vec<f32> = rows.iter().map(|r| r.iter().map(|&v| thick(v)).fold(0.0, f32::max)).collect();
    let row_len: Vec<f32> = rows.iter().map(|r| r.iter().map(|&v| wide(v)).sum::<f32>() + NODE_GAP * r.len().saturating_sub(1) as f32).collect();
    let span = row_len.iter().copied().fold(0.0, f32::max);
    let mut centre = vec![(0.0f32, 0.0f32); g.nodes.len()];
    let mut main = 0.0;
    for (r, row) in rows.iter().enumerate() {
        let mut cross = (span - row_len[r]) / 2.0;
        for &v in row {
            let c_main = main + rank_size[r] / 2.0;
            let c_cross = cross + wide(v) / 2.0;
            centre[v] = if vertical { (c_cross, c_main) } else { (c_main, c_cross) };
            cross += wide(v) + NODE_GAP;
        }
        main += rank_size[r] + RANK_GAP;
    }
    let total_main = main - RANK_GAP;
    if matches!(dir, Dir::Up | Dir::Left) {
        for c in &mut centre {
            if vertical {
                c.1 = total_main - c.1;
            } else {
                c.0 = total_main - c.0;
            }
        }
    }
    let (w, h) = if vertical { (span, total_main) } else { (total_main, span) };
    let (ox, oy) = (MARGIN, MARGIN);
    let boxes: Vec<(f32, f32, f32, f32)> =
        (0..g.nodes.len()).map(|v| (ox + centre[v].0 - sizes[v].0 / 2.0, oy + centre[v].1 - sizes[v].1 / 2.0, sizes[v].0, sizes[v].1)).collect();

    let stroke = "#1F2937";
    let mut children: Vec<Value> = Vec::new();
    // Edges first, so they run under the shapes.
    let mut detour = 0.0;
    for (k, e) in g.edges.iter().enumerate() {
        let (a, b) = (boxes[e.from], boxes[e.to]);
        let (acx, acy, bcx, bcy) = (a.0 + a.2 / 2.0, a.1 + a.3 / 2.0, b.0 + b.2 / 2.0, b.1 + b.3 / 2.0);
        let forward = !back[k] && e.from != e.to;
        let width = if e.line == Line::Thick { 3.0 } else { 1.5 };
        // Pull the end back so the triangle's tip touches the shape instead of crossing it.
        let pull = if e.arrow { (width * 3.0f32).max(6.0) * 0.4 } else { 0.0 };
        // A path that would run through another shape goes around the left (or top) instead.
        let blocked = |pts: &[[f32; 2]]| {
            pts.windows(2).any(|w| {
                let (x0, x1) = (w[0][0].min(w[1][0]), w[0][0].max(w[1][0]));
                let (y0, y1) = (w[0][1].min(w[1][1]), w[0][1].max(w[1][1]));
                boxes.iter().enumerate().any(|(i, bx)| {
                    i != e.from && i != e.to && x1 > bx.0 - 4.0 && x0 < bx.0 + bx.2 + 4.0 && y1 > bx.1 - 4.0 && y0 < bx.1 + bx.3 + 4.0
                })
            })
        };
        let mut points: Vec<[f32; 2]> = if forward {
            let flip = matches!(dir, Dir::Up | Dir::Left);
            if vertical {
                let (sy, ty) = if flip { (a.1, b.1 + b.3 + pull) } else { (a.1 + a.3, b.1 - pull) };
                if (acx - bcx).abs() < 1.0 {
                    vec![[acx, sy], [bcx, ty]]
                } else {
                    let my = (sy + ty) / 2.0;
                    vec![[acx, sy], [acx, my], [bcx, my], [bcx, ty]]
                }
            } else {
                let (sx, tx) = if flip { (a.0, b.0 + b.2 + pull) } else { (a.0 + a.2, b.0 - pull) };
                if (acy - bcy).abs() < 1.0 {
                    vec![[sx, acy], [tx, bcy]]
                } else {
                    let mx = (sx + tx) / 2.0;
                    vec![[sx, acy], [mx, acy], [mx, bcy], [tx, bcy]]
                }
            }
        } else {
            // Loops back: around the outside, right of (or below) both shapes.
            detour += 16.0;
            if vertical {
                let x = a.0.max(b.0) + a.2.max(b.2) + 24.0 + detour;
                let x = x.max(a.0 + a.2 + 24.0).max(b.0 + b.2 + 24.0);
                vec![[a.0 + a.2, acy], [x, acy], [x, bcy], [b.0 + b.2 + pull, bcy]]
            } else {
                let y = (a.1 + a.3).max(b.1 + b.3) + 24.0 + detour;
                vec![[acx, a.1 + a.3], [acx, y], [bcx, y], [bcx, b.1 + b.3 + pull]]
            }
        };
        if forward && blocked(&points) {
            detour += 16.0;
            let (lo, hi) = if vertical { (a.1.min(b.1), (a.1 + a.3).max(b.1 + b.3)) } else { (a.0.min(b.0), (a.0 + a.2).max(b.0 + b.2)) };
            // The outermost edge of the shapes in between, plus a margin.
            let side = boxes
                .iter()
                .filter(|bx| if vertical { bx.1 + bx.3 > lo && bx.1 < hi } else { bx.0 + bx.2 > lo && bx.0 < hi })
                .map(|bx| if vertical { bx.0 } else { bx.1 })
                .fold(f32::MAX, f32::min)
                - 24.0
                - detour;
            points = if vertical {
                vec![[a.0, acy], [side, acy], [side, bcy], [b.0 - pull, bcy]]
            } else {
                vec![[acx, a.1], [acx, side], [bcx, side], [bcx, b.1 - pull]]
            };
        }
        let mut arrow = json!({
            "type": "line",
            "name": format!("{} → {}", g.nodes[e.from].label.lines().next().unwrap_or(""), g.nodes[e.to].label.lines().next().unwrap_or("")),
            "points": points,
            "stroke": { "color": stroke, "width": width },
            "endPoint": if e.arrow { "triangle-arrow" } else { "none" },
        });
        if e.line == Line::Dotted {
            arrow["stroke"]["dash"] = json!([4, 4]);
        }
        children.push(arrow);
        if let Some(label) = &e.label {
            // On the middle of the path.
            let mid = points.len() / 2;
            let (p, q) = (points[mid - 1], points[mid]);
            let (mx, my) = ((p[0] + q[0]) / 2.0, (p[1] + q[1]) / 2.0);
            let lw = label.chars().count() as f32 * 12.0 * 0.56 + 12.0;
            children.push(json!({
                "type": "frame", "name": format!("Label: {label}"), "x": mx - lw / 2.0, "y": my - 11.0,
                "fill": "#FFFFFF", "radius": 4,
                "layout": { "direction": "horizontal", "padding": [3, 6], "justify": "center", "align": "center" },
                "children": [{ "type": "text", "text": label, "fontSize": 12, "fill": "#4B5563" }],
            }));
        }
    }
    for (v, n) in g.nodes.iter().enumerate() {
        let (x, y, bw, bh) = boxes[v];
        let text = json!({ "type": "text", "text": n.label, "fontSize": FONT, "fontWeight": 500, "fill": "#111827", "textAlign": "center" });
        let layout = json!({ "direction": "horizontal", "justify": "center", "align": "center" });
        let spec = match n.shape {
            Shape::Diamond => json!({
                "type": "frame", "name": n.label.lines().next().unwrap_or(&n.id), "x": x, "y": y, "width": bw, "height": bh,
                "fill": "none", "clip": false, "layout": layout,
                "children": [
                    { "type": "vector", "name": "Diamond", "absolute": true, "closed": true,
                      "points": [[bw / 2.0, 0.0], [bw, bh / 2.0], [bw / 2.0, bh], [0.0, bh / 2.0]],
                      "fill": "#FFF7E6", "stroke": { "color": stroke, "width": 1.5, "align": "center" } },
                    text,
                ],
            }),
            shape => json!({
                "type": if shape == Shape::Circle { "ellipse" } else { "frame" },
                "name": n.label.lines().next().unwrap_or(&n.id), "x": x, "y": y, "width": bw, "height": bh,
                "fill": if shape == Shape::Circle { "#ECFDF5" } else { "#FFFFFF" },
                "radius": match shape { Shape::Round => 12.0, Shape::Stadium => bh / 2.0, _ => 4.0 },
                "stroke": { "color": stroke, "width": 1.5 },
                "layout": layout,
                "children": [text],
            }),
        };
        // Ellipses can't hold children: a frame with a full radius stands in for a circle.
        let spec = if n.shape == Shape::Circle {
            let mut s = spec;
            s["type"] = json!("frame");
            s["radius"] = json!(bw / 2.0);
            s
        } else {
            spec
        };
        children.push(spec);
    }
    // Fit the frame around everything, detours included.
    let corners = |c: &Value| -> Vec<(f32, f32)> {
        if let Some(p) = c.get("points") {
            return serde_json::from_value::<Vec<[f32; 2]>>(p.clone()).unwrap_or_default().into_iter().map(|q| (q[0], q[1])).collect();
        }
        let num = |k: &str, d: f64| c.get(k).and_then(Value::as_f64).unwrap_or(d) as f32;
        let (x, y) = (num("x", 0.0), num("y", 0.0));
        vec![(x, y), (x + num("width", 40.0), y + num("height", 22.0))]
    };
    let (mut lo, mut hi) = ((f32::MAX, f32::MAX), (f32::MIN, f32::MIN));
    for (x, y) in children.iter().flat_map(corners) {
        lo = (lo.0.min(x), lo.1.min(y));
        hi = (hi.0.max(x), hi.1.max(y));
    }
    let (dx, dy) = (MARGIN - lo.0, MARGIN - lo.1);
    for c in &mut children {
        if let Some(p) = c.get_mut("points").and_then(Value::as_array_mut) {
            for q in p.iter_mut() {
                *q = json!([q[0].as_f64().unwrap_or(0.0) as f32 + dx, q[1].as_f64().unwrap_or(0.0) as f32 + dy]);
            }
        } else {
            c["x"] = json!(c["x"].as_f64().unwrap_or(0.0) as f32 + dx);
            c["y"] = json!(c["y"].as_f64().unwrap_or(0.0) as f32 + dy);
        }
    }
    let _ = (w, h);
    let root = json!({
        "type": "frame", "name": name, "width": hi.0 - lo.0 + 2.0 * MARGIN, "height": hi.1 - lo.1 + 2.0 * MARGIN,
        "fill": "#FAFAFA", "radius": 12, "clip": false, "children": children,
        "note": format!("Generated from Mermaid ({} nodes, {} links)", g.nodes.len(), g.edges.len()),
    });
    serde_json::from_value(root).map_err(|e| format!("Diagram spec: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mermaid_flowcharts() {
        let (g, dir) = parse("flowchart LR\n  A[Start] --> B{Is it?}\n  B -->|Yes| C(Do it)\n  B -- No --> D([Stop])\n  C -.-> A\n  E & F --- C").unwrap();
        assert_eq!(dir, Dir::Right);
        let labels: Vec<&str> = g.nodes.iter().map(|n| n.label.as_str()).collect();
        assert_eq!(labels, ["Start", "Is it?", "Do it", "Stop", "E", "F"]);
        assert_eq!(g.nodes[1].shape, Shape::Diamond);
        assert_eq!(g.nodes[3].shape, Shape::Stadium);
        assert_eq!(g.edges.len(), 6);
        assert_eq!(g.edges[1].label.as_deref(), Some("Yes"));
        assert_eq!(g.edges[2].label.as_deref(), Some("No"));
        assert_eq!(g.edges[3].line, Line::Dotted);
        assert!(!g.edges[4].arrow, "--- has no arrowhead");
    }

    #[test]
    fn ranks_follow_the_flow_and_ignore_cycles() {
        let (g, _) = parse("graph TD; a --> b; b --> c; c --> a; a --> c").unwrap();
        let (rank, back) = ranks(&g);
        assert_eq!(rank, vec![0, 1, 2]);
        assert_eq!(back, vec![false, false, true, false]);
        let spec = mermaid_spec("graph TD; a[One] --> b[Two]", "Flow").unwrap();
        assert_eq!(spec.children.len(), 3, "one arrow and two shapes");
    }
}
