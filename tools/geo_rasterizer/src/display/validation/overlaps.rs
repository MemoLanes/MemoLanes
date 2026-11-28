//! Check where every boundary ring lies with respect to every nearby ring.

use geo::{coordinate_position::coord_pos_relative_to_ring, BoundingRect};
use geo_types::LineString;
use rayon::prelude::*;

use crate::display::{
    rings,
    source::Prepared,
    spatial::{Bounds, SpatialIndex},
    topology::{Graph, RingId, Shape, Vertex},
    Point,
};

struct Ring<'a> {
    id: RingId,
    before: &'a LineString<f64>,
    after: LineString<f64>,
    source_anchor: Point,
    bounds: Bounds,
}

pub(super) struct NestingChange {
    pub(super) rings: [RingId; 2],
    pub(super) at: Point,
}

// Crossing order and junction incidence determine adjacent faces. For
// disconnected boundaries (e.g. an island inside a hole), one corresponding
// vertex per ring also fixes which face contains the whole boundary.
pub(super) fn changed_nesting(
    graph: &Graph,
    sources: &[Prepared],
    shapes: &[Shape],
) -> Vec<NestingChange> {
    let mut boundaries = Vec::new();
    for (owner, source) in sources.iter().enumerate() {
        for ((original, candidate), ids) in source
            .geometry
            .0
            .iter()
            .zip(&shapes[owner])
            .zip(&graph.shapes[owner])
        {
            for ((before, after), &id) in rings(original).zip(candidate).zip(ids) {
                let (first, forward) = graph.rings[id].arcs[0];
                let path = &graph.arcs[first].nodes;
                let node = if forward {
                    path[0]
                } else {
                    *path.last().unwrap()
                };
                let after = LineString(after.iter().copied().map(Vertex::degrees).collect());
                let bounds = Bounds::from_rect(before.bounding_rect().unwrap())
                    .union(Bounds::from_rect(after.bounding_rect().unwrap()));
                boundaries.push(Ring {
                    id,
                    before,
                    after,
                    source_anchor: graph.points[node],
                    bounds,
                });
            }
        }
    }
    let index = SpatialIndex::new(boundaries.iter().map(|r| r.bounds));
    let mut conflicts = boundaries
        .par_iter()
        .enumerate()
        .fold(Vec::new, |mut conflicts, (i, a)| {
            let output_anchor = a.after.0[0];
            index.query(Bounds::new(a.source_anchor, output_anchor), |j| {
                if i == j {
                    return;
                }
                let b = &boundaries[j];
                if coord_pos_relative_to_ring(a.source_anchor, b.before)
                    != coord_pos_relative_to_ring(output_anchor, &b.after)
                {
                    conflicts.push(NestingChange {
                        rings: [a.id, b.id],
                        at: a.source_anchor,
                    });
                }
            });
            conflicts
        })
        .reduce(Vec::new, |mut a, b| {
            a.extend(b);
            a
        });
    conflicts.sort_unstable_by_key(|c| c.rings);
    conflicts
}
