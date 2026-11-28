//! Insert exact source subdivisions before a rounded meridian intersection
//! can make two originally collinear paths appear to diverge.

use std::collections::HashMap;

use geo_data_format::GeoEntityId;
use geo_types::{LineString, Polygon};
use num_rational::BigRational;

use super::{at_seam, preserves_world_span, rings, Bounds, DisplaySource, Point, SpatialIndex};

struct Crossing {
    source: usize,
    part: usize,
    ring: usize,
    edge: usize,
    from: Point,
    to: Point,
    interior: Vec<Point>,
}

impl Crossing {
    fn bounds(&self) -> [Bounds; 2] {
        let west = self.from.x.max(self.to.x);
        let east = self.from.x.min(self.to.x);
        let (a, b) = (self.from.y, self.to.y);
        [
            Bounds::new((west, a).into(), (180., b).into()),
            Bounds::new((-180., a).into(), (east, b).into()),
        ]
    }

    fn contains_interior(&self, point: Point) -> bool {
        let (a, b) = (self.from, self.to);
        if [a, b]
            .iter()
            .any(|p| point.x == p.x || (at_seam(point.x) && at_seam(p.x)))
        {
            return false;
        }
        let exact = |v: f64| BigRational::from_float(v).expect("finite coordinate");
        let longitude = |x: f64| {
            if x < 0.0 {
                exact(x) + exact(360.)
            } else {
                exact(x)
            }
        };
        // Adding 360 in f64 can erase a low bit and turn a near miss into
        // exact collinearity. The index already bounds the candidate interval.
        let (x, y) = (longitude(a.x), exact(a.y));
        (longitude(point.x) - &x) * (exact(b.y) - &y) == (longitude(b.x) - x) * (exact(point.y) - y)
    }
}

pub(super) fn node_sources(
    sources: &[DisplaySource],
) -> HashMap<(GeoEntityId, usize), Polygon<f64>> {
    let valid = |p: Point| p.x.abs() <= 180.0 && p.y.abs() <= 90.0;
    let mut crossings = Vec::new();
    for (source, region) in sources.iter().enumerate() {
        for (part, polygon) in region.polygons.0.iter().enumerate() {
            if preserves_world_span(polygon) {
                continue;
            }
            for (ring, path) in rings(polygon).enumerate() {
                for (edge, &[from, to]) in path.0.array_windows().enumerate() {
                    if valid(from)
                        && valid(to)
                        && (from.x - to.x).abs() > 180.0
                        && !(at_seam(from.x) && at_seam(to.x))
                    {
                        crossings.push(Crossing {
                            source,
                            part,
                            ring,
                            edge,
                            from,
                            to,
                            interior: Vec::new(),
                        });
                    }
                }
            }
        }
    }
    if crossings.is_empty() {
        return HashMap::new();
    }
    let index = SpatialIndex::new(crossings.iter().flat_map(Crossing::bounds));
    for region in sources {
        for &p in region.polygons.0.iter().flat_map(rings).flat_map(|r| &r.0) {
            if valid(p) {
                let p = Point {
                    x: if at_seam(p.x) { 180. } else { p.x },
                    y: p.y,
                };
                index.query(Bounds::new(p, p), |i| {
                    let crossing = &mut crossings[i / 2];
                    if crossing.contains_interior(p) {
                        crossing.interior.push(p);
                    }
                });
            }
        }
    }

    let longitude_order = |a: &Point, b: &Point| {
        (a.x < 0.)
            .cmp(&(b.x < 0.))
            .then_with(|| a.x.total_cmp(&b.x))
    };
    crossings.retain(|edge| !edge.interior.is_empty());
    for edge in &mut crossings {
        edge.interior.sort_unstable_by(longitude_order);
        edge.interior.dedup();
        if edge.from.x < edge.to.x {
            edge.interior.reverse();
        }
    }

    let mut noded = HashMap::new();
    for part in crossings.chunk_by(|a, b| (a.source, a.part) == (b.source, b.part)) {
        let first = &part[0];
        let region = &sources[first.source];
        let mut polygon = region.polygons.0[first.part].clone();
        for edges in part.chunk_by(|a, b| a.ring == b.ring) {
            let insert = |ring: &mut LineString<f64>| {
                let original = std::mem::take(&mut ring.0);
                let mut edges = edges.iter().peekable();
                for (i, point) in original.into_iter().enumerate() {
                    ring.0.push(point);
                    if let Some(edge) = edges.next_if(|e| e.edge == i) {
                        ring.0.extend_from_slice(&edge.interior);
                    }
                }
            };
            if edges[0].ring == 0 {
                polygon.exterior_mut(insert);
            } else {
                polygon.interiors_mut(|rings| insert(&mut rings[edges[0].ring - 1]));
            }
        }
        noded.insert((region.entity_id, first.part), polygon);
    }
    noded
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo_types::{wkt, MultiPolygon};

    #[test]
    fn wrapping_cannot_make_a_nearby_vertex_collinear() {
        let on = Point { x: -120., y: 40. };
        let near = Point {
            x: on.x.next_up(),
            y: 40.,
        };
        assert_eq!(on.x + 360., near.x + 360.);
        let crossing = wkt!(POLYGON((160.0 0.0, -90.0 55.0, -90.0 70.0, 160.0 70.0, 160.0 0.0)));
        let neighbor = Polygon::new(LineString(vec![on, near, (-120., 39.).into(), on]), vec![]);
        let sources = [crossing, neighbor]
            .into_iter()
            .enumerate()
            .map(|(i, p)| DisplaySource {
                entity_id: GeoEntityId(i as u32),
                polygons: MultiPolygon(vec![p]),
            })
            .collect::<Vec<_>>();
        let result = node_sources(&sources);
        let ring = &result[&(GeoEntityId(0), 0)].exterior().0;
        assert!(ring.contains(&on));
        assert!(!ring.contains(&near));
    }
}
