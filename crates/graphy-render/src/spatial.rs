//! R-tree spatial index for nodes, comments and wires.
//!
//! Moved from the Blueprint plugin's `core/spatial_index.rs` and made
//! document-agnostic: entries are `(index, bounds)` pairs and the owner
//! ([`crate::Scene`]) decides what an index means. Queries return indices in
//! draw order (or reverse, for top-most-first picking).

use rstar::{RTree, RTreeObject, AABB};

use crate::geom::Rect;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub index: usize,
    pub rect: Rect,
}

impl RTreeObject for Entry {
    type Envelope = AABB<[f32; 2]>;
    fn envelope(&self) -> Self::Envelope {
        AABB::from_corners(
            [self.rect.min.x, self.rect.min.y],
            [self.rect.max.x, self.rect.max.y],
        )
    }
}

impl rstar::PointDistance for Entry {
    fn distance_2(&self, p: &[f32; 2]) -> f32 {
        let d = self.rect.distance_to(crate::geom::Vec2::new(p[0], p[1]));
        d * d
    }
}

#[derive(Default)]
pub struct Layer {
    tree: RTree<Entry>,
}

impl Layer {
    pub fn rebuild(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.tree = RTree::bulk_load(entries.into_iter().collect());
    }

    pub fn insert(&mut self, entry: Entry) {
        self.tree.insert(entry);
    }

    pub fn remove(&mut self, entry: &Entry) -> bool {
        self.tree.remove(entry).is_some()
    }

    /// Moves an entry; a no-op when the bounds did not change.
    pub fn update(&mut self, old: Entry, new_rect: Rect) {
        if old.rect == new_rect {
            return;
        }
        self.tree.remove(&old);
        self.tree.insert(Entry {
            index: old.index,
            rect: new_rect,
        });
    }

    pub fn len(&self) -> usize {
        self.tree.size()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.size() == 0
    }

    /// Indices whose bounds intersect `rect`, ascending (draw order) or
    /// descending when `top_first`.
    pub fn query(&self, rect: Rect, top_first: bool) -> Vec<usize> {
        let env = AABB::from_corners([rect.min.x, rect.min.y], [rect.max.x, rect.max.y]);
        let mut out: Vec<usize> = self
            .tree
            .locate_in_envelope_intersecting(&env)
            .map(|e| e.index)
            .collect();
        out.sort_unstable();
        if top_first {
            out.reverse();
        }
        out
    }
}

#[derive(Default)]
pub struct SpatialIndex {
    pub nodes: Layer,
    pub comments: Layer,
    pub wires: Layer,
    pub ties: Layer,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::Vec2;

    fn e(i: usize, x: f32, y: f32) -> Entry {
        Entry {
            index: i,
            rect: Rect::from_pos_size(Vec2::new(x, y), Vec2::new(10.0, 10.0)),
        }
    }

    #[test]
    fn query_is_ordered_and_culls() {
        let mut l = Layer::default();
        l.rebuild([e(2, 0.0, 0.0), e(0, 5.0, 5.0), e(1, 500.0, 500.0)]);
        let r = Rect::from_pos_size(Vec2::ZERO, Vec2::new(20.0, 20.0));
        assert_eq!(l.query(r, false), vec![0, 2]);
        assert_eq!(l.query(r, true), vec![2, 0]);
    }

    #[test]
    fn update_moves_entry() {
        let mut l = Layer::default();
        let old = e(0, 0.0, 0.0);
        l.insert(old);
        l.update(old, e(0, 300.0, 300.0).rect);
        assert!(l
            .query(
                Rect::from_pos_size(Vec2::ZERO, Vec2::new(20.0, 20.0)),
                false
            )
            .is_empty());
        assert_eq!(
            l.query(
                Rect::from_pos_size(Vec2::new(290.0, 290.0), Vec2::new(30.0, 30.0)),
                false
            ),
            vec![0]
        );
    }
}
