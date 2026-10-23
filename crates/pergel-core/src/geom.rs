use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn right(&self) -> f32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }

    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.right() && other.x < self.right() && self.y < other.bottom() && other.y < self.bottom()
    }

    pub fn contains(&self, other: &Rect) -> bool {
        other.x >= self.x && other.y >= self.y && other.right() <= self.right() && other.bottom() <= self.bottom()
    }

    pub fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.x && y >= self.y && x <= self.right() && y <= self.bottom()
    }

    pub fn intersection(&self, other: &Rect) -> Option<Rect> {
        let (x0, y0) = (self.x.max(other.x), self.y.max(other.y));
        let (x1, y1) = (self.right().min(other.right()), self.bottom().min(other.bottom()));
        (x1 > x0 && y1 > y0).then(|| Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn union(&self, other: &Rect) -> Rect {
        let (x0, y0) = (self.x.min(other.x), self.y.min(other.y));
        Rect::new(x0, y0, self.right().max(other.right()) - x0, self.bottom().max(other.bottom()) - y0)
    }

    pub fn union_all(rects: impl IntoIterator<Item = Rect>) -> Option<Rect> {
        rects.into_iter().reduce(|a, b| a.union(&b))
    }

    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    pub fn translated(&self, dx: f32, dy: f32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }

    /// Same size, centred on (cx, cy).
    pub fn centered(&self, cx: f32, cy: f32) -> Rect {
        Rect::new(cx - self.w / 2.0, cy - self.h / 2.0, self.w, self.h)
    }

    /// Corners clockwise from the top-left, after rotating the box about its centre.
    pub fn corners(&self, degrees: f32) -> [(f32, f32); 4] {
        let (cx, cy) = self.center();
        [(self.x, self.y), (self.right(), self.y), (self.right(), self.bottom()), (self.x, self.bottom())]
            .map(|(x, y)| rotate_point(x, y, cx, cy, degrees))
    }

    /// The axis-aligned box around this box rotated about its centre.
    pub fn rotated_aabb(&self, degrees: f32) -> Rect {
        if degrees == 0.0 {
            return *self;
        }
        let c = self.corners(degrees);
        let (mut x0, mut y0, mut x1, mut y1) = (c[0].0, c[0].1, c[0].0, c[0].1);
        for (x, y) in &c[1..] {
            x0 = x0.min(*x);
            y0 = y0.min(*y);
            x1 = x1.max(*x);
            y1 = y1.max(*y);
        }
        Rect::new(x0, y0, x1 - x0, y1 - y0)
    }

    /// Whether a world point lies inside this box rotated about its centre.
    pub fn contains_rotated(&self, x: f32, y: f32, degrees: f32) -> bool {
        let (cx, cy) = self.center();
        let (lx, ly) = rotate_point(x, y, cx, cy, -degrees);
        self.contains_point(lx, ly)
    }
}

/// Rotates (x, y) about (cx, cy). Positive degrees turn counter-clockwise on screen, as in Figma.
pub fn rotate_point(x: f32, y: f32, cx: f32, cy: f32, degrees: f32) -> (f32, f32) {
    if degrees == 0.0 {
        return (x, y);
    }
    let (s, c) = degrees.to_radians().sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    (cx + dx * c + dy * s, cy - dx * s + dy * c)
}

/// Normalises an angle to (-180, 180].
pub fn normalize_degrees(d: f32) -> f32 {
    let mut d = d % 360.0;
    if d <= -180.0 {
        d += 360.0;
    } else if d > 180.0 {
        d -= 360.0;
    }
    if d.abs() < 1e-4 { 0.0 } else { d }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Color(pub u32);

impl Color {
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self(u32::from_be_bytes([a, r, g, b]))
    }

    pub const fn argb(self) -> u32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rect_intersection_is_exclusive_at_edges() {
        let a = Rect::new(0.0, 0.0, 10.0, 10.0);
        assert!(a.intersects(&Rect::new(9.0, 9.0, 5.0, 5.0)));
        assert!(!a.intersects(&Rect::new(10.0, 0.0, 5.0, 5.0)));
    }

    #[test]
    fn color_packs_as_argb() {
        assert_eq!(Color::rgba(0x11, 0x22, 0x33, 0xff).argb(), 0xff11_2233);
    }
}
