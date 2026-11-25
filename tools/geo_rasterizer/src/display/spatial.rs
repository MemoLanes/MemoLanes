use geo_types::Rect;
use rstar::{
    primitives::{GeomWithData, Rectangle},
    Envelope, RTree, AABB,
};

use super::Point;

#[derive(Clone, Copy)]
pub(super) struct Bounds {
    pub(super) min: Point,
    pub(super) max: Point,
}

impl Bounds {
    pub(super) fn from_rect(rect: Rect<f64>) -> Self {
        Self {
            min: rect.min(),
            max: rect.max(),
        }
    }

    pub(super) fn new(a: Point, b: Point) -> Self {
        Self::from_rect(Rect::new(a, b))
    }

    pub(super) fn expanded(self, by: f64) -> Self {
        Self::new(
            (self.min.x - by, self.min.y - by).into(),
            (self.max.x + by, self.max.y + by).into(),
        )
    }

    pub(super) fn union(self, b: Self) -> Self {
        Self::new(
            (self.min.x.min(b.min.x), self.min.y.min(b.min.y)).into(),
            (self.max.x.max(b.max.x), self.max.y.max(b.max.y)).into(),
        )
    }

    pub(super) fn has_area_in_common(self, b: Self) -> bool {
        self.min.x.max(b.min.x) < self.max.x.min(b.max.x)
            && self.min.y.max(b.min.y) < self.max.y.min(b.max.y)
    }

    pub(super) fn intersects(self, b: Self) -> bool {
        self.envelope().intersects(&b.envelope())
    }

    fn envelope(self) -> AABB<[f64; 2]> {
        AABB::from_corners([self.min.x, self.min.y], [self.max.x, self.max.y])
    }
}

/// Unordered, inclusive bounding-box queries for source points and emitted edges.
pub(super) struct SpatialIndex(RTree<GeomWithData<Rectangle<[f64; 2]>, usize>>);

impl SpatialIndex {
    pub(super) fn new(bounds: impl IntoIterator<Item = Bounds>) -> Self {
        Self(RTree::bulk_load(
            bounds
                .into_iter()
                .enumerate()
                .map(|(id, bounds)| GeomWithData::new(Rectangle::from_aabb(bounds.envelope()), id))
                .collect(),
        ))
    }

    pub(super) fn query(&self, bounds: Bounds, mut visit: impl FnMut(usize)) {
        for entry in self.0.locate_in_envelope_intersecting(&bounds.envelope()) {
            visit(entry.data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_includes_point_and_edge_contacts_without_requiring_area() {
        let bounds = |a: (f64, f64), b: (f64, f64)| Bounds::new(a.into(), b.into());
        let query = bounds((0.0, 0.0), (2.0, 2.0));
        let point = bounds((2.0, 2.0), (2.0, 2.0));
        let vertical = bounds((2.0, 0.5), (2.0, 1.5));
        let horizontal = bounds((0.5, 2.0), (1.5, 3.0));
        let overlap = bounds((1.0, 1.0), (3.0, 3.0));
        let outside = bounds((2.5, 0.0), (3.0, 1.0));

        let mut ids = Vec::new();
        SpatialIndex::new([point, outside, vertical, horizontal, overlap])
            .query(query, |id| ids.push(id));
        ids.sort_unstable();
        assert_eq!(ids, [0, 2, 3, 4]);
        for contact in [point, vertical, horizontal] {
            assert!(!query.has_area_in_common(contact));
        }
        assert!(query.has_area_in_common(overlap));
    }
}
