use crate::geom::Rect;

pub const MIN_ZOOM: f32 = 0.01;
pub const MAX_ZOOM: f32 = 256.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    pub x: f32,
    pub y: f32,
    pub zoom: f32,
    pub width: f32,
    pub height: f32,
}

impl Viewport {
    pub fn new(width: f32, height: f32) -> Self {
        Self { x: 0.0, y: 0.0, zoom: 1.0, width, height }
    }

    pub fn resize(&mut self, width: f32, height: f32) {
        self.width = width;
        self.height = height;
    }

    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.x -= dx / self.zoom;
        self.y -= dy / self.zoom;
    }

    pub fn zoom_at(&mut self, factor: f32, sx: f32, sy: f32) {
        let (wx, wy) = self.to_world(sx, sy);
        self.zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        self.x = wx - sx / self.zoom;
        self.y = wy - sy / self.zoom;
    }

    pub fn fit(&mut self, area: Rect, margin: f32) {
        if area.w <= 0.0 || area.h <= 0.0 {
            return;
        }
        let zoom = ((self.width - 2.0 * margin) / area.w).min((self.height - 2.0 * margin) / area.h);
        self.zoom = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        self.x = area.x - (self.width / self.zoom - area.w) / 2.0;
        self.y = area.y - (self.height / self.zoom - area.h) / 2.0;
    }

    pub fn to_world(&self, sx: f32, sy: f32) -> (f32, f32) {
        (self.x + sx / self.zoom, self.y + sy / self.zoom)
    }

    pub fn to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        ((wx - self.x) * self.zoom, (wy - self.y) * self.zoom)
    }

    pub fn world_area(&self) -> Rect {
        Rect::new(self.x, self.y, self.width / self.zoom, self.height / self.zoom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_keeps_the_point_under_the_cursor() {
        let mut v = Viewport::new(800.0, 600.0);
        v.pan(-120.0, -40.0);
        let before = v.to_world(300.0, 200.0);
        v.zoom_at(2.5, 300.0, 200.0);
        let after = v.to_world(300.0, 200.0);
        assert!((before.0 - after.0).abs() < 1e-3 && (before.1 - after.1).abs() < 1e-3);
    }

    #[test]
    fn fit_shows_the_whole_area() {
        let mut v = Viewport::new(800.0, 600.0);
        v.fit(Rect::new(0.0, 0.0, 4000.0, 1000.0), 0.0);
        let area = v.world_area();
        assert!(area.w >= 4000.0 - 0.1 && area.h >= 1000.0 - 0.1);
    }
}
