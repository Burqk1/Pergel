//! Editable form of vector outlines: anchors with in/out handles, grouped in sub-paths. Used by
//! the pen tool and vector edit mode; converted to and from SVG path data (M/L/Q/C/Z).

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub p: (f32, f32),
    /// Handle shaping the curve that arrives at this anchor.
    pub cin: Option<(f32, f32)>,
    /// Handle shaping the curve that leaves it.
    pub cout: Option<(f32, f32)>,
}

impl Anchor {
    pub fn corner(x: f32, y: f32) -> Self {
        Self { p: (x, y), cin: None, cout: None }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct SubPath {
    pub anchors: Vec<Anchor>,
    pub closed: bool,
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

/// Parses absolute path data (M, L, H, V, C, S, Q, T, Z) into sub-paths. Quadratic curves become
/// cubic ones.
pub fn parse_path(d: &str) -> Vec<SubPath> {
    let mut tokens: Vec<Tok> = Vec::new();
    let mut num = String::new();
    let flush = |num: &mut String, tokens: &mut Vec<Tok>| {
        if !num.is_empty() {
            if let Ok(v) = num.parse::<f32>() {
                tokens.push(Tok::Num(v));
            }
            num.clear();
        }
    };
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() && ch != 'e' && ch != 'E' {
            flush(&mut num, &mut tokens);
            tokens.push(Tok::Cmd(ch));
        } else if ch == ',' || ch.is_whitespace() {
            flush(&mut num, &mut tokens);
        } else if (ch == '-' && !num.is_empty() && !num.ends_with(['e', 'E']))
            || (ch == '.' && num.contains('.') && !num.contains(['e', 'E']))
        {
            // A sign or a second decimal point starts the next number.
            flush(&mut num, &mut tokens);
            num.push(ch);
        } else {
            num.push(ch);
        }
    }
    flush(&mut num, &mut tokens);

    let mut paths: Vec<SubPath> = Vec::new();
    let mut cur = SubPath::default();
    let mut cmd = 'M';
    let mut i = 0;
    let mut pos = (0.0f32, 0.0f32);
    let mut start = (0.0f32, 0.0f32);
    let mut last_ctrl: Option<(f32, f32)> = None;
    let take = |i: &mut usize, n: usize| -> Option<Vec<f32>> {
        let mut out = Vec::with_capacity(n);
        for _ in 0..n {
            match tokens.get(*i) {
                Some(Tok::Num(v)) => out.push(*v),
                _ => return None,
            }
            *i += 1;
        }
        Some(out)
    };
    let close_into = |cur: &mut SubPath, paths: &mut Vec<SubPath>| {
        if !cur.anchors.is_empty() {
            paths.push(std::mem::take(cur));
        }
    };
    while i < tokens.len() {
        if let Tok::Cmd(c) = tokens[i] {
            cmd = c;
            i += 1;
            if c == 'Z' || c == 'z' {
                // A closing segment back onto the first anchor merges into it.
                if cur.anchors.len() > 1 && near(cur.anchors[cur.anchors.len() - 1].p, cur.anchors[0].p) {
                    let last = cur.anchors.pop().unwrap_or(Anchor::corner(0.0, 0.0));
                    cur.anchors[0].cin = last.cin;
                }
                cur.closed = true;
                pos = start;
                close_into(&mut cur, &mut paths);
                last_ctrl = None;
                continue;
            }
        }
        let rel = cmd.is_ascii_lowercase();
        let off = |x: f32, y: f32, pos: (f32, f32)| if rel { (x + pos.0, y + pos.1) } else { (x, y) };
        match cmd.to_ascii_uppercase() {
            'M' => {
                let Some(v) = take(&mut i, 2) else { break };
                close_into(&mut cur, &mut paths);
                pos = off(v[0], v[1], pos);
                start = pos;
                cur.anchors.push(Anchor::corner(pos.0, pos.1));
                // Further pairs after M are line-tos.
                cmd = if rel { 'l' } else { 'L' };
                last_ctrl = None;
            }
            'L' => {
                let Some(v) = take(&mut i, 2) else { break };
                pos = off(v[0], v[1], pos);
                cur.anchors.push(Anchor::corner(pos.0, pos.1));
                last_ctrl = None;
            }
            'H' => {
                let Some(v) = take(&mut i, 1) else { break };
                pos = (if rel { pos.0 + v[0] } else { v[0] }, pos.1);
                cur.anchors.push(Anchor::corner(pos.0, pos.1));
                last_ctrl = None;
            }
            'V' => {
                let Some(v) = take(&mut i, 1) else { break };
                pos = (pos.0, if rel { pos.1 + v[0] } else { v[0] });
                cur.anchors.push(Anchor::corner(pos.0, pos.1));
                last_ctrl = None;
            }
            'C' | 'S' => {
                let smooth = cmd.eq_ignore_ascii_case(&'S');
                let Some(v) = take(&mut i, if smooth { 4 } else { 6 }) else { break };
                let c1 = if smooth {
                    last_ctrl.map_or(pos, |c| (2.0 * pos.0 - c.0, 2.0 * pos.1 - c.1))
                } else {
                    off(v[0], v[1], pos)
                };
                let k = if smooth { 0 } else { 2 };
                let c2 = off(v[k], v[k + 1], pos);
                let end = off(v[k + 2], v[k + 3], pos);
                if let Some(a) = cur.anchors.last_mut() {
                    a.cout = Some(c1);
                }
                cur.anchors.push(Anchor { p: end, cin: Some(c2), cout: None });
                pos = end;
                last_ctrl = Some(c2);
            }
            'Q' | 'T' => {
                let smooth = cmd.eq_ignore_ascii_case(&'T');
                let Some(v) = take(&mut i, if smooth { 2 } else { 4 }) else { break };
                let q = if smooth { last_ctrl.map_or(pos, |c| (2.0 * pos.0 - c.0, 2.0 * pos.1 - c.1)) } else { off(v[0], v[1], pos) };
                let k = if smooth { 0 } else { 2 };
                let end = off(v[k], v[k + 1], pos);
                let c1 = (pos.0 + 2.0 / 3.0 * (q.0 - pos.0), pos.1 + 2.0 / 3.0 * (q.1 - pos.1));
                let c2 = (end.0 + 2.0 / 3.0 * (q.0 - end.0), end.1 + 2.0 / 3.0 * (q.1 - end.1));
                if let Some(a) = cur.anchors.last_mut() {
                    a.cout = Some(c1);
                }
                cur.anchors.push(Anchor { p: end, cin: Some(c2), cout: None });
                pos = end;
                last_ctrl = Some(q);
            }
            _ => {
                // Unsupported command (arcs): skip its numbers.
                i += 1;
            }
        }
    }
    close_into(&mut cur, &mut paths);
    paths
}

#[derive(Debug, Clone, Copy)]
enum Tok {
    Cmd(char),
    Num(f32),
}

fn n(v: f32) -> String {
    crate::paint::fmt(v)
}

/// Absolute path data for the sub-paths (straight segments as L, curves as C).
pub fn to_path(paths: &[SubPath]) -> String {
    let mut d = String::new();
    let seg = |d: &mut String, a: &Anchor, b: &Anchor| {
        if a.cout.is_none() && b.cin.is_none() {
            d.push_str(&format!("L{} {}", n(b.p.0), n(b.p.1)));
        } else {
            let c1 = a.cout.unwrap_or(a.p);
            let c2 = b.cin.unwrap_or(b.p);
            d.push_str(&format!("C{} {} {} {} {} {}", n(c1.0), n(c1.1), n(c2.0), n(c2.1), n(b.p.0), n(b.p.1)));
        }
    };
    for p in paths {
        let Some(first) = p.anchors.first() else { continue };
        d.push_str(&format!("M{} {}", n(first.p.0), n(first.p.1)));
        for w in p.anchors.windows(2) {
            seg(&mut d, &w[0], &w[1]);
        }
        if p.closed && p.anchors.len() > 1 {
            let last = &p.anchors[p.anchors.len() - 1];
            if last.cout.is_some() || first.cin.is_some() {
                seg(&mut d, last, first);
            }
            d.push('Z');
        }
    }
    d
}

/// Applies `f` to every anchor and handle position.
pub fn map_points(paths: &mut [SubPath], f: impl Fn((f32, f32)) -> (f32, f32)) {
    for p in paths {
        for a in &mut p.anchors {
            a.p = f(a.p);
            a.cin = a.cin.map(&f);
            a.cout = a.cout.map(&f);
        }
    }
}

/// Freehand stroke to a smooth path: Ramer–Douglas–Peucker simplification with `tolerance`,
/// then Catmull-Rom tangents as cubic handles.
pub fn smooth_stroke(points: &[(f32, f32)], tolerance: f32) -> SubPath {
    fn rdp(p: &[(f32, f32)], tol: f32, out: &mut Vec<(f32, f32)>) {
        if p.len() < 3 {
            out.extend_from_slice(&p[..p.len().saturating_sub(1)]);
            return;
        }
        let (a, b) = (p[0], p[p.len() - 1]);
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        let (mut far, mut idx) = (0.0f32, 0);
        for (i, q) in p.iter().enumerate().skip(1).take(p.len() - 2) {
            let d = ((q.0 - a.0) * dy - (q.1 - a.1) * dx).abs() / len;
            if d > far {
                far = d;
                idx = i;
            }
        }
        if far > tol {
            rdp(&p[..=idx], tol, out);
            rdp(&p[idx..], tol, out);
        } else {
            out.push(a);
        }
    }
    let mut pts = Vec::new();
    if points.len() > 1 {
        rdp(points, tolerance, &mut pts);
    }
    if let Some(last) = points.last() {
        pts.push(*last);
    }
    let n = pts.len();
    let mut anchors: Vec<Anchor> = pts.iter().map(|p| Anchor::corner(p.0, p.1)).collect();
    if n > 2 {
        for i in 0..n {
            let prev = pts[i.saturating_sub(1)];
            let next = pts[(i + 1).min(n - 1)];
            let (tx, ty) = ((next.0 - prev.0) / 6.0, (next.1 - prev.1) / 6.0);
            let p = pts[i];
            if i > 0 {
                anchors[i].cin = Some((p.0 - tx, p.1 - ty));
            }
            if i < n - 1 {
                anchors[i].cout = Some((p.0 + tx, p.1 + ty));
            }
        }
    }
    SubPath { anchors, closed: false }
}

/// The point on a cubic segment at `t`.
pub fn cubic_at(a: (f32, f32), c1: (f32, f32), c2: (f32, f32), b: (f32, f32), t: f32) -> (f32, f32) {
    let u = 1.0 - t;
    let w = [u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t];
    (w[0] * a.0 + w[1] * c1.0 + w[2] * c2.0 + w[3] * b.0, w[0] * a.1 + w[1] * c1.1 + w[2] * c2.1 + w[3] * b.1)
}

/// Splits the segment after anchor `i` of `path` at `t` (de Casteljau), inserting an anchor.
pub fn split_segment(path: &mut SubPath, i: usize, t: f32) -> usize {
    let n = path.anchors.len();
    let j = (i + 1) % n;
    let (a, b) = (path.anchors[i], path.anchors[j]);
    let lerp = |p: (f32, f32), q: (f32, f32)| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t);
    if a.cout.is_none() && b.cin.is_none() {
        let mid = lerp(a.p, b.p);
        path.anchors.insert(i + 1, Anchor::corner(mid.0, mid.1));
        return i + 1;
    }
    let (c1, c2) = (a.cout.unwrap_or(a.p), b.cin.unwrap_or(b.p));
    let (p01, p12, p23) = (lerp(a.p, c1), lerp(c1, c2), lerp(c2, b.p));
    let (p012, p123) = (lerp(p01, p12), lerp(p12, p23));
    let mid = lerp(p012, p123);
    path.anchors[i].cout = Some(p01);
    path.anchors[j].cin = Some(p23);
    path.anchors.insert(i + 1, Anchor { p: mid, cin: Some(p012), cout: Some(p123) });
    i + 1
}

/// The nearest point to `q` on any segment: (path, segment start anchor, t, distance).
pub fn nearest_segment(paths: &[SubPath], q: (f32, f32)) -> Option<(usize, usize, f32, f32)> {
    let mut best: Option<(usize, usize, f32, f32)> = None;
    for (pi, p) in paths.iter().enumerate() {
        let n = p.anchors.len();
        let segs = if p.closed { n } else { n.saturating_sub(1) };
        for i in 0..segs {
            let (a, b) = (p.anchors[i], p.anchors[(i + 1) % n]);
            let (c1, c2) = (a.cout.unwrap_or(a.p), b.cin.unwrap_or(b.p));
            for k in 0..=24 {
                let t = k as f32 / 24.0;
                let pt = cubic_at(a.p, c1, c2, b.p, t);
                let dist = ((pt.0 - q.0).powi(2) + (pt.1 - q.1).powi(2)).sqrt();
                if best.is_none_or(|b| dist < b.3) {
                    best = Some((pi, i, t, dist));
                }
            }
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_data_round_trips_through_anchors() {
        let d = "M0 0L10 0C15 0 20 5 20 10L0 10Z";
        let paths = parse_path(d);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].closed);
        assert_eq!(paths[0].anchors.len(), 4);
        assert_eq!(paths[0].anchors[1].cout, Some((15.0, 0.0)));
        assert_eq!(to_path(&paths), d);
        // Quadratics become cubics; relative and shorthand commands are understood.
        let q = parse_path("m10 10 h10 v10 q0 10 -10 10z");
        assert_eq!(q[0].anchors.len(), 4);
        assert_eq!(q[0].anchors[2].p, (20.0, 20.0));
        assert_eq!(q[0].anchors[3].p, (10.0, 30.0));
        // A closing segment that ends on the first anchor merges into it.
        let c = parse_path("M0 0C5 0 10 5 10 10C10 15 5 20 0 20C-5 20 -10 15 0 0Z");
        assert_eq!(c[0].anchors.len(), 3);
        assert!(c[0].anchors[0].cin.is_some());
    }

    #[test]
    fn segments_split_where_clicked() {
        let mut p = parse_path("M0 0L100 0")[0].clone();
        let (pi, i, t, dist) = nearest_segment(std::slice::from_ref(&p), (50.0, 2.0)).unwrap();
        assert_eq!((pi, i), (0, 0));
        assert!((t - 0.5).abs() < 0.05 && dist < 2.5);
        let k = split_segment(&mut p, i, 0.5);
        assert_eq!(k, 1);
        assert_eq!(p.anchors[1].p, (50.0, 0.0));
    }
}
