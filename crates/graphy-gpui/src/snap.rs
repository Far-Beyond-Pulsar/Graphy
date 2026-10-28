//! Move snapping: alignment to other nodes' edges/centres and the grid.

use graphy_render::{Rect, Vec2};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SnapResult {
    /// Adjustment to add to the proposed movement.
    pub delta: Vec2,
    /// Thin rectangles (graph space) marking the alignments that were used.
    pub guides: Vec<Rect>,
}

#[derive(Clone, Copy, Debug)]
pub struct SnapOptions {
    pub grid: Option<f32>,
    pub align_to_nodes: bool,
    /// Capture distance, graph units.
    pub threshold: f32,
    /// Guide thickness, graph units (one screen pixel).
    pub guide_thickness: f32,
}

fn lines_x(r: &Rect) -> [f32; 3] {
    [r.min.x, (r.min.x + r.max.x) * 0.5, r.max.x]
}

fn lines_y(r: &Rect) -> [f32; 3] {
    [r.min.y, (r.min.y + r.max.y) * 0.5, r.max.y]
}

/// Snaps `moving` (the rectangle after the proposed move) against `others`.
pub fn snap_move(moving: Rect, others: &[Rect], o: SnapOptions) -> SnapResult {
    let mut best_x: Option<(f32, f32, usize)> = None; // (delta, line, other index)
    let mut best_y: Option<(f32, f32, usize)> = None;
    if o.align_to_nodes {
        for (i, other) in others.iter().enumerate() {
            for m in lines_x(&moving) {
                for t in lines_x(other) {
                    let d = t - m;
                    if d.abs() <= o.threshold && best_x.is_none_or(|(bd, ..)| d.abs() < bd.abs()) {
                        best_x = Some((d, t, i));
                    }
                }
            }
            for m in lines_y(&moving) {
                for t in lines_y(other) {
                    let d = t - m;
                    if d.abs() <= o.threshold && best_y.is_none_or(|(bd, ..)| d.abs() < bd.abs()) {
                        best_y = Some((d, t, i));
                    }
                }
            }
        }
    }
    let mut delta = Vec2::ZERO;
    let mut guides = Vec::new();
    match best_x {
        Some((d, line, i)) => {
            delta.x = d;
            let span = moving.translate(Vec2::new(d, 0.0)).union(&others[i]);
            guides.push(Rect::from_corners(
                Vec2::new(line - o.guide_thickness * 0.5, span.min.y - 8.0),
                Vec2::new(line + o.guide_thickness * 0.5, span.max.y + 8.0),
            ));
        }
        None => {
            if let Some(g) = o.grid {
                delta.x = (moving.min.x / g).round() * g - moving.min.x;
            }
        }
    }
    match best_y {
        Some((d, line, i)) => {
            delta.y = d;
            let span = moving.translate(Vec2::new(0.0, d)).union(&others[i]);
            guides.push(Rect::from_corners(
                Vec2::new(span.min.x - 8.0, line - o.guide_thickness * 0.5),
                Vec2::new(span.max.x + 8.0, line + o.guide_thickness * 0.5),
            ));
        }
        None => {
            if let Some(g) = o.grid {
                delta.y = (moving.min.y / g).round() * g - moving.min.y;
            }
        }
    }
    SnapResult { delta, guides }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_pos_size(Vec2::new(x, y), Vec2::new(w, h))
    }

    const OPTS: SnapOptions = SnapOptions {
        grid: Some(10.0),
        align_to_nodes: true,
        threshold: 6.0,
        guide_thickness: 1.0,
    };

    #[test]
    fn aligns_to_neighbouring_edge() {
        let s = snap_move(
            r(103.0, 40.0, 50.0, 30.0),
            &[r(100.0, 200.0, 80.0, 30.0)],
            OPTS,
        );
        assert_eq!(s.delta.x, -3.0);
        assert_eq!(s.guides.len(), 1);
        // y has no neighbour in range -> grid
        assert_eq!(s.delta.y, 0.0);
    }

    #[test]
    fn falls_back_to_grid() {
        let s = snap_move(r(103.0, 47.0, 50.0, 30.0), &[], OPTS);
        assert_eq!((s.delta.x, s.delta.y), (-3.0, 3.0));
        assert!(s.guides.is_empty());
    }

    #[test]
    fn no_snap_when_disabled() {
        let o = SnapOptions {
            grid: None,
            align_to_nodes: false,
            ..OPTS
        };
        let s = snap_move(r(103.0, 47.0, 50.0, 30.0), &[r(100.0, 40.0, 50.0, 30.0)], o);
        assert_eq!(s.delta, Vec2::ZERO);
    }
}
