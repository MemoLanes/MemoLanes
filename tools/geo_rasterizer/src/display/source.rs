//! Prepare and validate the source polygons before building shared arcs.

use std::collections::HashMap;

use anyhow::{ensure, Result};
use geo::{
    coordinate_position::CoordPos, dimensions::Dimensions, Area, BooleanOps, BoundingRect, Orient,
    Relate, Validation,
};
use geo_data_format::{DisplayBounds, GeoEntityId};
use geo_types::{Line, LineString, MultiPolygon, Polygon, Rect};
use rayon::prelude::*;

use super::{
    at_seam, parameter, point_key, rings,
    spatial::{Bounds, SpatialIndex},
    DisplaySource, Point, ROUNDOFF,
};

const VIEW_WIDTH: f64 = 640.0;
const VIEW_HEIGHT: f64 = 480.0;
// Match the Explorer's maximum fit: a 512 CSS-pixel MapLibre world at zoom 14.
const MIN_PROJECTED_DEGREES_PER_PIXEL: f64 = 360.0 / (512.0 * (1_u32 << 14) as f64);

pub(super) struct Prepared {
    pub(super) id: GeoEntityId,
    pub(super) geometry: MultiPolygon<f64>,
    pub(super) bounds: DisplayBounds,
    pub(super) budget: f64,
    pub(super) repaired: bool,
}

fn unwrap_ring(ring: &LineString<f64>, preserve_world_span: bool) -> Result<LineString<f64>> {
    let mut out: Vec<Point> = Vec::new();
    for &point in &ring.0 {
        ensure!(
            point.x.is_finite()
                && point.y.is_finite()
                && point.x.abs() <= 180.0
                && point.y.abs() <= 90.0,
            "coordinates outside longitude/latitude domain"
        );
        let mut p = point;
        if let Some(prev) = out.last() {
            if !preserve_world_span {
                while p.x - prev.x > 180.0 {
                    p.x -= 360.0;
                }
                while p.x - prev.x < -180.0 {
                    p.x += 360.0;
                }
            }
            if p == *prev {
                continue;
            }
        }
        out.push(p);
    }
    ensure!(out.len() >= 4, "collapsed source ring");
    let first = out[0];
    let last = *out.last().unwrap();
    if first != last {
        // A cap described by a coastline that winds around the pole needs a
        // closure at the pole, not a chord across the opposite hemisphere.
        ensure!(
            ring.0.first() == ring.0.last() && (first.x - last.x).abs() == 360.0,
            "source ring is not closed"
        );
        let pole = if out.iter().all(|p| p.y > 0.0) {
            90.0
        } else if out.iter().all(|p| p.y < 0.0) {
            -90.0
        } else {
            anyhow::bail!("longitude winding is ambiguous outside a polar cap");
        };
        out.extend([
            Point { x: last.x, y: pole },
            Point {
                x: first.x,
                y: pole,
            },
            first,
        ]);
    }
    Ok(LineString(out))
}

fn component_conflicts(geometry: &MultiPolygon<f64>) -> impl Iterator<Item = (usize, usize)> + '_ {
    let bounds: Vec<_> = geometry
        .0
        .iter()
        .map(|p| Bounds::from_rect(p.bounding_rect().unwrap()))
        .collect();
    geometry.0.iter().enumerate().flat_map(move |(i, p)| {
        let bounds = &bounds;
        ((i + 1)..geometry.0.len())
            .filter_map(move |j| {
                if !bounds[i].intersects(bounds[j]) {
                    return None;
                }
                let relation = p.relate(&geometry.0[j]);
                (relation.get(CoordPos::Inside, CoordPos::Inside) == Dimensions::TwoDimensional
                    || relation.get(CoordPos::OnBoundary, CoordPos::OnBoundary)
                        == Dimensions::OneDimensional)
                    .then_some((i, j))
            })
            .collect::<Vec<_>>()
    })
}

pub(super) fn valid_geometry(geometry: &MultiPolygon<f64>) -> Result<()> {
    for polygon in &geometry.0 {
        ensure!(polygon.unsigned_area() > 0.0, "zero-area polygon");
        polygon.check_validation()?;
    }
    ensure!(
        component_conflicts(geometry).next().is_none(),
        "polygon components overlap or share an edge"
    );
    Ok(())
}

fn join_components(geometry: &mut MultiPolygon<f64>) -> Result<bool> {
    let mut affected = vec![false; geometry.0.len()];
    for (a, b) in component_conflicts(geometry) {
        affected[a] = true;
        affected[b] = true;
    }
    if !affected.contains(&true) {
        return Ok(false);
    }
    let original: Vec<_> = geometry
        .0
        .iter()
        .zip(&affected)
        .filter_map(|(polygon, &affected)| affected.then_some(polygon))
        .collect();
    // An attributed area can be added to a country as a separate polygon even
    // though it shares an edge with an existing component. Union only the
    // affected components; leave all detached islands bit-for-bit intact.
    let mut fixed = geo::algorithm::bool_ops::unary_union(original.iter().copied());
    restore_overlay_vertices(&mut fixed, &original);
    valid_geometry(&fixed)?;
    for polygon in &original {
        ensure!(
            polygon.difference(&fixed).unsigned_area() <= polygon.unsigned_area() * 1e-6,
            "component union changes source coverage"
        );
    }
    let mut i = 0;
    geometry.0.retain(|_| {
        let keep = !affected[i];
        i += 1;
        keep
    });
    geometry.0.extend(fixed.0);
    Ok(true)
}

/// FloatOverlay uses a centered binary integer grid internally. Replace its
/// round-tripped coordinates with exact input vertices only for unambiguous
/// grid keys, so repair does not turn coincident borders into near misses.
fn restore_overlay_vertices(fixed: &mut MultiPolygon<f64>, original: &[&Polygon<f64>]) {
    let points = || {
        original
            .iter()
            .flat_map(|p| rings(p))
            .flat_map(|r| r.0.iter().copied())
    };
    let bounds = points()
        .map(|p| Bounds::new(p, p))
        .reduce(Bounds::union)
        .unwrap();
    let half = (bounds.max - bounds.min) / 2.0;
    let step = 2.0_f64.powi(half.x.max(half.y).log2().round() as i32 - 29);
    let center = bounds.min + half;
    let round_trip = |p: Point| {
        let q = p - center;
        let round = |x: f64| (x / step).round() * step;
        Point {
            x: round(q.x),
            y: round(q.y),
        } + center
    };
    let mut input = HashMap::new();
    for point in points() {
        input
            .entry(point_key(round_trip(point)))
            .and_modify(|p| {
                if *p != Some(point) {
                    *p = None;
                }
            })
            .or_insert(Some(point));
    }
    let mut vertices: Vec<_> = input
        .values()
        .filter_map(|p| *p)
        .map(|p| (round_trip(p), p))
        .collect();
    vertices.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.0.y.total_cmp(&b.0.y)));
    let index = SpatialIndex::new(vertices.iter().map(|&(p, _)| Bounds::new(p, p)));
    for polygon in &mut fixed.0 {
        let restore = |ring: &mut LineString<f64>| {
            let original = std::mem::take(&mut ring.0);
            let mut restored = Vec::new();
            for pair in original.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                restored.push(input.get(&point_key(a)).and_then(|v| *v).unwrap_or(a));
                // The overlay can omit a collinear input vertex. Recover it
                // on the overlay's own integer segment before restoring its
                // exact source position. No other entity's points are used.
                let integer = |p: Point| (p - center) / step;
                let mut interior = Vec::new();
                index.query(Bounds::new(a, b), |i| {
                    let (p, source) = vertices[i];
                    if p == a || p == b {
                        return;
                    }
                    let (a, b, p) = (integer(a), integer(b), integer(p));
                    if geo::Intersects::intersects(&Line::new(a, b), &geo_types::Point(p)) {
                        interior.push((parameter(a, b, p), source));
                    }
                });
                interior.sort_by(|a, b| a.0.total_cmp(&b.0));
                restored.extend(interior.into_iter().map(|(_, p)| p));
            }
            if let Some(&first) = restored.first() {
                restored.push(first);
            }
            ring.0 = restored;
        };
        polygon.exterior_mut(restore);
        polygon.interiors_mut(|rings| rings.iter_mut().for_each(restore));
    }
}

fn prepare(source: &DisplaySource) -> Result<Prepared> {
    let mut polygons = Vec::new();
    let mut repaired = false;
    for p in &source.polygons.0 {
        // A ring cut open on both dateline meridians (e.g. Antarctica)
        // represents the full width. Do not unwrap its explicit pole closure.
        let preserve_world_span = p.exterior().0.windows(2).any(|e| {
            at_seam(e[0].x) && e[0].x == -e[1].x && e[0].y == e[1].y && e[0].y.abs() == 90.0
        });
        let exterior = unwrap_ring(p.exterior(), preserve_world_span)?;
        let b = exterior.bounding_rect().unwrap();
        let holes = p
            .interiors()
            .iter()
            .map(|ring| {
                let mut ring = unwrap_ring(ring, preserve_world_span)?;
                let shift = ((b.center().x - ring.bounding_rect().unwrap().center().x) / 360.0)
                    .round()
                    * 360.0;
                if shift != 0.0 {
                    for point in &mut ring.0 {
                        point.x += shift;
                    }
                }
                Ok(ring)
            })
            .collect::<Result<_>>()?;
        let p = Polygon::new(exterior, holes);
        ensure!(
            b.width() <= 360.0 + ROUNDOFF,
            "polygon winds more than once around the globe"
        );
        let pieces = if let Err(error) = p.check_validation() {
            // Natural Earth has some self-touching rings. Polygonize their
            // even/odd interior; accept only area-preserving, valid output.
            let mut fixed = p.union(&MultiPolygon(vec![]));
            restore_overlay_vertices(&mut fixed, &[&p]);
            let before = p.unsigned_area();
            ensure!(
                before > 0.0 && (fixed.unsigned_area() - before).abs() <= before * 1e-6,
                "source invalid ({error}); polygonization changes area"
            );
            valid_geometry(&fixed)?;
            repaired = true;
            fixed.0
        } else {
            vec![p]
        };
        for p in pieces {
            let b = p.bounding_rect().unwrap();
            if b.min().x >= -180.0 && b.max().x <= 180.0 {
                polygons.push(p);
                continue;
            }
            // GeoJSON cuts at the dateline. Keep the source on either side
            // instead of publishing a line from +179° to -179°.
            let lo = ((b.min().x + 180.0) / 360.0).floor() as i32;
            let hi = ((b.max().x + 180.0) / 360.0).floor() as i32;
            let mut total = 0.0;
            for world in lo..=hi {
                let shift = f64::from(world) * 360.0;
                let slab = Rect::new((-180.0 + shift, -90.0), (180.0 + shift, 90.0));
                for piece in p.intersection(&slab.to_polygon()).0 {
                    total += piece.unsigned_area();
                    let shift_ring = |ring: &LineString<f64>| {
                        LineString(
                            ring.0
                                .iter()
                                .map(|p| Point {
                                    x: (p.x - shift).clamp(-180.0, 180.0),
                                    y: p.y.clamp(-90.0, 90.0),
                                })
                                .collect(),
                        )
                    };
                    polygons.push(Polygon::new(
                        shift_ring(piece.exterior()),
                        piece.interiors().iter().map(shift_ring).collect(),
                    ));
                }
            }
            let area = p.unsigned_area();
            ensure!(
                (total - area).abs() <= area * 1e-6,
                "dateline cut changes source area"
            );
        }
    }
    ensure!(!polygons.is_empty(), "empty source");
    let mut geometry = MultiPolygon(polygons).orient(geo::orient::Direction::Default);
    repaired |= join_components(&mut geometry)?;
    valid_geometry(&geometry)?;
    let bounds = source_bounds(&geometry);
    Ok(Prepared {
        id: source.entity_id,
        geometry,
        bounds,
        budget: fit_budget(bounds),
        repaired,
    })
}

fn source_bounds(geometry: &MultiPolygon<f64>) -> DisplayBounds {
    let mut extents: Vec<_> = geometry
        .0
        .iter()
        .filter_map(BoundingRect::bounding_rect)
        .collect();
    extents.sort_by(|a, b| a.min().x.total_cmp(&b.min().x));
    let south = extents.iter().map(|r| r.min().y).fold(90.0, f64::min);
    let north = extents.iter().map(|r| r.max().y).fold(-90.0, f64::max);
    let first = extents[0].min().x;
    let mut reach = extents[0].max().x;
    let mut gap = (0.0, first, first + 360.0);
    // Complement the largest gap in the union of *intervals*, not the set
    // of vertices. A broad polygon occupies all longitudes in its interval.
    for r in &extents[1..] {
        if r.min().x - reach > gap.0 {
            gap = (r.min().x - reach, r.min().x, reach + 360.0);
        }
        reach = reach.max(r.max().x);
    }
    if first + 360.0 - reach >= gap.0 {
        gap = (first + 360.0 - reach, first, reach);
    }
    DisplayBounds {
        west: gap.1,
        south,
        east: gap.2,
        north,
    }
}

fn fit_budget(b: DisplayBounds) -> f64 {
    use bitmap_geometry::MERCATOR_MAX_LATITUDE as LIMIT;
    let scale_lat = |lat: f64| lat.clamp(-LIMIT, LIMIT).to_radians();
    let mercator_y = |lat| scale_lat(lat).tan().asinh().to_degrees();
    let fit = ((b.east - b.west) / VIEW_WIDTH)
        .max((mercator_y(b.north) - mercator_y(b.south)) / VIEW_HEIGHT)
        .max(MIN_PROJECTED_DEGREES_PER_PIXEL);
    // Convert the projected pixel to a conservative angular budget using
    // the worst local Mercator scale anywhere in the entity's latitude span.
    fit * scale_lat(b.north.abs().max(b.south.abs())).cos()
        + (b.north - b.south).abs() * f64::EPSILON
}

pub(super) fn prepare_sources(sources: &[DisplaySource], prefix: &str) -> Result<Vec<Prepared>> {
    let mut sorted: Vec<_> = sources.iter().collect();
    sorted.sort_by_key(|s| s.entity_id);
    ensure!(
        sorted
            .windows(2)
            .all(|pair| pair[0].entity_id != pair[1].entity_id),
        "duplicate display source ID"
    );
    let mut prepared = vec![];
    for (source, result) in sorted
        .par_iter()
        .map(|s| (*s, prepare(s)))
        .collect::<Vec<_>>()
    {
        match result {
            Ok(source) => {
                if source.repaired {
                    eprintln!(
                        "{prefix} id={} repaired source by area-preserving polygonization",
                        source.id.0
                    );
                }
                prepared.push(source);
            }
            Err(error) => eprintln!("{prefix} id={} unavailable: {error:#}", source.entity_id.0),
        }
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_budget_floors_subpixel_extents_before_latitude_scale() {
        let bounds = DisplayBounds {
            west: 12.0,
            east: 12.00002,
            south: 40.0,
            north: 40.00002,
        };
        let expected = 360.0 / (512.0 * 2.0_f64.powi(14)) * bounds.north.to_radians().cos()
            + (bounds.north - bounds.south) * f64::EPSILON;
        assert_eq!(fit_budget(bounds), expected);
    }

    #[test]
    fn fit_budget_preserves_larger_projected_extents() {
        for (east, north) in [(20.0, 41.0), (13.0, 50.0)] {
            let bounds = DisplayBounds {
                west: 12.0,
                east,
                south: 40.0,
                north,
            };
            let mercator_y = |lat: f64| lat.to_radians().tan().asinh().to_degrees();
            let previous = ((east - bounds.west) / VIEW_WIDTH)
                .max((mercator_y(north) - mercator_y(bounds.south)) / VIEW_HEIGHT)
                * north.to_radians().cos()
                + (north - bounds.south) * f64::EPSILON;
            assert_eq!(fit_budget(bounds), previous);
        }
    }
}
