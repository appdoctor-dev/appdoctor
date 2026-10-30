//! Editing geometry: moving with snapping, resizing from corners, and text
//! boxes that grow with their lines. Plain functions, so the editor's view
//! only routes input and draws.

use crate::{LINE_HEIGHT, Layer, LayerKind, Rect};

/// Where a snap line is drawn while dragging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Guide {
    /// A vertical line at this x.
    Vertical(f32),
    /// A horizontal line at this y.
    Horizontal(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub const MIN_SIZE: f32 = 40.;

impl Layer {
    pub fn rect(&self) -> Rect {
        Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
        }
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.x = rect.x;
        self.y = rect.y;
        self.width = rect.width;
        self.height = rect.height;
        self.fit_text();
    }

    /// Text boxes are as tall as their wrapped lines.
    pub fn fit_text(&mut self) {
        if let LayerKind::Text { size, .. } = &self.kind {
            let lines = self.lines().len().max(1) as f32;
            self.height = (lines * size * LINE_HEIGHT).ceil();
        }
    }
}

/// Moves `rect` by (dx, dy), snapping its edges and centre to the edges and
/// centre of the `canvas` (width, height) and to `others`' edges and
/// centres within `threshold`.
pub fn snap_move(
    rect: Rect,
    dx: f32,
    dy: f32,
    canvas: (f32, f32),
    others: &[Rect],
    threshold: f32,
) -> (Rect, Vec<Guide>) {
    let mut moved = Rect {
        x: rect.x + dx,
        y: rect.y + dy,
        ..rect
    };
    let mut guides = Vec::new();

    let (width, height) = canvas;
    let mut xs = vec![0., width / 2., width];
    let mut ys = vec![0., height / 2., height];
    for other in others {
        xs.extend([other.x, other.x + other.width / 2., other.x + other.width]);
        ys.extend([other.y, other.y + other.height / 2., other.y + other.height]);
    }
    let edges = |start: f32, length: f32| [start, start + length / 2., start + length];

    if let Some((shift, at)) = best_snap(&edges(moved.x, moved.width), &xs, threshold) {
        moved.x += shift;
        guides.push(Guide::Vertical(at));
    }
    if let Some((shift, at)) = best_snap(&edges(moved.y, moved.height), &ys, threshold) {
        moved.y += shift;
        guides.push(Guide::Horizontal(at));
    }
    (moved, guides)
}

/// The smallest shift that lines one of `points` up with one of `targets`.
fn best_snap(points: &[f32; 3], targets: &[f32], threshold: f32) -> Option<(f32, f32)> {
    let mut best: Option<(f32, f32)> = None;
    for point in points {
        for target in targets {
            let shift = target - point;
            if shift.abs() <= threshold && best.is_none_or(|(current, _)| shift.abs() < current.abs()) {
                best = Some((shift, *target));
            }
        }
    }
    best
}

/// Resizes from `corner` by (dx, dy). With `keep_aspect` the width decides
/// and the height follows the starting ratio; the opposite corner stays put.
pub fn resize(start: Rect, corner: Corner, dx: f32, dy: f32, keep_aspect: bool) -> Rect {
    let left = matches!(corner, Corner::TopLeft | Corner::BottomLeft);
    let top = matches!(corner, Corner::TopLeft | Corner::TopRight);
    let mut width = (if left { start.width - dx } else { start.width + dx }).max(MIN_SIZE);
    let mut height = (if top { start.height - dy } else { start.height + dy }).max(MIN_SIZE);
    if keep_aspect && start.width > 0. {
        let ratio = start.height / start.width;
        height = (width * ratio).max(MIN_SIZE);
        width = height / ratio;
    }
    Rect {
        x: if left { start.x + start.width - width } else { start.x },
        y: if top { start.y + start.height - height } else { start.y },
        width,
        height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Target;

    const CANVAS: (f32, f32) = (1320., 2868.);

    fn rect(x: f32, y: f32, width: f32, height: f32) -> Rect {
        Rect { x, y, width, height }
    }

    #[test]
    fn snaps_to_canvas_centre_within_threshold() {
        // A 200-wide box whose centre lands at 655: within 8px of 660.
        let (moved, guides) = snap_move(rect(0., 300., 200., 100.), 555., 0., CANVAS, &[], 8.);
        assert_eq!(moved.x, 560.);
        assert_eq!(guides, vec![Guide::Vertical(660.)]);
        // Too far: no snap.
        let (moved, guides) = snap_move(rect(0., 300., 200., 100.), 540., 0., CANVAS, &[], 8.);
        assert_eq!(moved.x, 540.);
        assert!(guides.is_empty());
    }

    #[test]
    fn snaps_edges_to_other_layers() {
        let other = rect(100., 1000., 300., 300.);
        let (moved, guides) = snap_move(rect(500., 500., 100., 100.), 0., 497., CANVAS, &[other], 6.);
        assert_eq!(moved.y, 1000.);
        assert!(guides.contains(&Guide::Horizontal(1000.)));
    }

    #[test]
    fn resizes_from_each_corner_keeping_the_opposite_one() {
        let start = rect(100., 100., 400., 200.);
        let bigger = resize(start, Corner::BottomRight, 100., 50., false);
        assert_eq!(bigger, rect(100., 100., 500., 250.));
        let from_top_left = resize(start, Corner::TopLeft, 100., 50., false);
        assert_eq!(from_top_left, rect(200., 150., 300., 150.));
        let tiny = resize(start, Corner::BottomRight, -1000., -1000., false);
        assert_eq!((tiny.width, tiny.height), (MIN_SIZE, MIN_SIZE));
    }

    #[test]
    fn aspect_locked_resize_follows_width() {
        let start = rect(0., 0., 100., 200.);
        let resized = resize(start, Corner::BottomRight, 50., 0., true);
        assert_eq!((resized.width, resized.height), (150., 300.));
        let from_top_left = resize(start, Corner::TopLeft, 50., 0., true);
        assert_eq!(from_top_left, rect(50., 100., 50., 100.));
    }

    #[test]
    fn text_boxes_grow_with_their_lines() {
        let mut layer = Layer::text(Target::Iphone, 1, "Plan your whole day in seconds", 0., 100., 700, "#fff");
        layer.set_rect(rect(0., 0., 2000., 10.));
        assert_eq!(layer.height, (100. * LINE_HEIGHT).ceil());
        layer.set_rect(rect(0., 0., 1000., 10.));
        assert_eq!(layer.height, (2. * 100. * LINE_HEIGHT).ceil());
    }
}
