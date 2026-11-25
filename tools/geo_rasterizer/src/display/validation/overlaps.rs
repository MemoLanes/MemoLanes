//! Compare filled interiors, including containment with no boundary crossing.

use std::collections::HashMap;

use anyhow::Result;
use geo::{
    coordinate_position::{CoordPos, CoordinatePosition},
    dimensions::Dimensions,
    BoundingRect, Relate,
};
use geo_types::Polygon;
use rayon::prelude::*;

use crate::display::{
    source::Prepared,
    spatial::{Bounds, SpatialIndex},
    topology::{polygon_from_grid, Shape},
};

fn interior_intersection(a: &Polygon<f64>, b: &Polygon<f64>) -> Dimensions {
    a.relate(b).get(CoordPos::Inside, CoordPos::Inside)
}

// The owner index is stable (prepared sources are sorted by entity ID),
// as is the component's index within its prepared source.
type ComponentId = (usize, usize);

struct OverlapComponent<'a> {
    id: ComponentId,
    before: &'a Polygon<f64>,
    after: Polygon<f64>,
    bounds: Bounds,
    unchanged: bool,
}

#[derive(Default)]
pub(super) struct InteriorOverlapCache {
    shapes: Vec<Option<Shape>>,
    pairs: HashMap<[ComponentId; 2], bool>,
}

// A new overlap needn't cross any boundary: for example, a hole can
// shrink inside an island. Compare the filled interiors, including holes.
pub(super) fn new_interior_overlaps(
    sources: &[Prepared],
    shapes: &[Option<Result<Shape>>],
    cache: &mut InteriorOverlapCache,
) -> Vec<[usize; 2]> {
    let valid_shapes = || {
        shapes
            .iter()
            .map(|shape| shape.as_ref().and_then(|s| s.as_ref().ok()))
    };
    let mut components = Vec::new();
    for (owner, shape) in valid_shapes().enumerate() {
        let Some(shape) = shape else { continue };
        let previous = cache.shapes.get(owner).and_then(Option::as_ref);
        for (part, (before, grid)) in sources[owner].geometry.0.iter().zip(shape).enumerate() {
            // Use exact storage units: converting snapped vertices
            // to degrees can turn a tangent into a sliver of area.
            let after = polygon_from_grid(grid, |[x, y]| (x.into(), y.into()).into());
            components.push(OverlapComponent {
                id: (owner, part),
                before,
                bounds: Bounds::from_rect(after.bounding_rect().unwrap()),
                after,
                unchanged: previous.and_then(|shape| shape.get(part)) == Some(grid),
            });
        }
    }
    let index = SpatialIndex::new(components.iter().map(|c| c.bounds));
    let pairs = &cache.pairs;
    let checked: Vec<_> = components
        .par_iter()
        .flat_map_iter(|a| {
            let mut checked = Vec::new();
            index.query(a.bounds, |j| {
                let b = &components[j];
                if a.id >= b.id || a.id.0 == b.id.0 || !a.bounds.has_area_in_common(b.bounds) {
                    return;
                }
                let key = [a.id, b.id];
                // Cache only one snapshot. A candidate is reusable only if
                // both full grid components match, including shared endpoints.
                if a.unchanged && b.unchanged {
                    if let Some(&overlap) = pairs.get(&key) {
                        checked.push((key, overlap));
                        return;
                    }
                }
                // A boundary vertex strictly inside a valid source polygon
                // proves that their interiors already overlap nearby.
                if a.before.coordinate_position(&b.before.exterior().0[0]) == CoordPos::Inside
                    || b.before.coordinate_position(&a.before.exterior().0[0]) == CoordPos::Inside
                {
                    return;
                }
                let overlap = interior_intersection(&a.after, &b.after)
                    == Dimensions::TwoDimensional
                    && interior_intersection(a.before, b.before) == Dimensions::Empty;
                checked.push((key, overlap));
            });
            checked
        })
        .collect();
    let mut changed: Vec<_> = checked
        .iter()
        .filter_map(|&([(a, _), (b, _)], overlap)| overlap.then_some([a, b]))
        .collect();
    cache.shapes = valid_shapes().map(|shape| shape.cloned()).collect();
    cache.pairs = checked.into_iter().collect();
    changed.sort_unstable();
    changed.dedup();
    changed
}
