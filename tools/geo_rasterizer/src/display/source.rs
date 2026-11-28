//! Prepare and validate the source polygons before building shared arcs.

mod crossings;
mod dateline;
mod repair;

use std::collections::HashMap;

use anyhow::{ensure, Context, Result};
use geo::{
    coordinate_position::CoordPos, dimensions::Dimensions, Area, BooleanOps, BoundingRect, Orient,
    Relate, Validation,
};
use geo_data_format::{DisplayBounds, GeoEntityId};
use geo_types::{Line, LineString, MultiPolygon, Polygon};
use i_float::{adapter::FloatPointAdapter, float::rect::FloatRect};
use rayon::prelude::*;

use super::{
    at_seam,
    bounds::{fit_budget, source_bounds},
    parameter, point_key, rings,
    spatial::{Bounds, SpatialIndex},
    DisplaySource, Point, ROUNDOFF,
};

pub(super) struct Prepared {
    pub(super) id: GeoEntityId,
    pub(super) geometry: MultiPolygon<f64>,
    pub(super) bounds: DisplayBounds,
    pub(super) budget: f64,
    pub(super) repaired: bool,
}

type SourcePositions = HashMap<(u64, u64), Point>;

fn unwrap_ring(
    ring: &LineString<f64>,
    preserve_world_span: bool,
    align_to: Option<f64>,
    source_positions: &mut SourcePositions,
) -> Result<LineString<f64>> {
    let mut worlds = Vec::with_capacity(ring.0.len());
    let mut world = 0_i32;
    let (mut west, mut east) = (f64::INFINITY, f64::NEG_INFINITY);
    for (i, point) in ring.0.iter().enumerate() {
        ensure!(
            point.x.is_finite()
                && point.y.is_finite()
                && point.x.abs() <= 180.0
                && point.y.abs() <= 90.0,
            "point ({}, {}) outside longitude/latitude domain",
            point.x,
            point.y
        );
        if i > 0 && !preserve_world_span {
            let delta = point.x - ring.0[i - 1].x;
            if delta > 180.0 {
                world -= 1;
            }
            if delta < -180.0 {
                world += 1;
            }
        }
        worlds.push(world);
        let x = point.x + 360.0 * f64::from(world);
        west = west.min(x);
        east = east.max(x);
    }
    let shift = align_to.map_or(0, |center| {
        ((center - west.midpoint(east)) / 360.0).round() as i32
    });
    let mut out = Vec::with_capacity(worlds.len());
    for (&original, turn) in ring.0.iter().zip(worlds) {
        let offset = 360.0 * f64::from(turn + shift);
        let p = Point {
            x: original.x + offset,
            y: original.y,
        };
        if offset != 0.0 && !at_seam(original.x) {
            ensure!(
                p.x != offset - 180.0 && p.x != offset + 180.0,
                "source vertex {original:?} coincides with the dateline after unwrapping"
            );
            ensure!(
                source_positions
                    .insert(point_key(p), original)
                    .is_none_or(|old| old == original),
                "distinct source vertices coincide after unwrapping at {original:?}"
            );
        }
        if out.last() != Some(&p) {
            out.push(p);
        }
    }
    ensure!(out.len() >= 4, "collapsed source ring");
    let first = out[0];
    let last = *out.last().unwrap();
    if first != last {
        // A winding polar coastline needs a closure at the pole.
        ensure!(
            ring.0.first() == ring.0.last() && world.abs() == 1,
            "source ring is not closed"
        );
        let pole = if out.iter().all(|p| p.y > 0.0) {
            90.0
        } else if out.iter().all(|p| p.y < 0.0) {
            -90.0
        } else {
            anyhow::bail!("longitude winding is ambiguous outside a polar cap");
        };
        let original_pole = Point {
            x: ring.0[0].x,
            y: pole,
        };
        for x in [first.x, last.x] {
            if x != original_pole.x && !at_seam(original_pole.x) {
                ensure!(
                    source_positions
                        .insert(point_key(Point { x, y: pole }), original_pole)
                        .is_none_or(|old| old == original_pole),
                    "ambiguous polar closure at {original_pole:?}"
                );
            }
        }
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
    let mut neighbors = vec![Vec::new(); geometry.0.len()];
    for (a, b) in component_conflicts(geometry) {
        neighbors[a].push(b);
        neighbors[b].push(a);
    }
    if neighbors.iter().all(Vec::is_empty) {
        return Ok(false);
    }
    // Each connected group needs its own overlay scale. A distant mainland
    // can otherwise push an entire group of small islands below grid precision.
    let mut joined = Vec::new();
    let mut consumed = vec![false; geometry.0.len()];
    for start in 0..neighbors.len() {
        if consumed[start] || neighbors[start].is_empty() {
            continue;
        }
        let mut pending = vec![start];
        let mut group = Vec::new();
        while let Some(i) = pending.pop() {
            if !std::mem::replace(&mut consumed[i], true) {
                group.push(i);
                pending.extend(&neighbors[i]);
            }
        }
        group.sort_unstable();
        let originals: Vec<_> = group.iter().map(|&i| &geometry.0[i]).collect();
        let fixed = repair::union(&originals)
            .with_context(|| format!("joining source polygons {group:?}"))?;
        ensure!(
            fixed.0.len() == 1,
            "union of connected source polygons {group:?} has {} parts",
            fixed.0.len()
        );
        valid_geometry(&fixed)?;
        joined.extend(fixed.0);
    }
    let mut consumed = consumed.into_iter();
    geometry.0.retain(|_| !consumed.next().unwrap());
    geometry.0.extend(joined);
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
    let adapter = FloatPointAdapter::<[f64; 2], f64>::new(FloatRect::new(
        bounds.min.x,
        bounds.max.x,
        bounds.min.y,
        bounds.max.y,
    ));
    let round_trip = |p: Point| {
        let [x, y] = adapter.int_to_float(&adapter.float_to_int(&[p.x, p.y]));
        Point { x, y }
    };
    let integer = |p: Point| {
        let i = adapter.float_to_int(&[p.x, p.y]);
        Point {
            x: f64::from(i.x),
            y: f64::from(i.y),
        }
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

// A polygon explicitly closed across both dateline meridians at a pole
// represents the full width of the map.
fn preserves_world_span(polygon: &Polygon<f64>) -> bool {
    polygon
        .exterior()
        .0
        .windows(2)
        .any(|e| at_seam(e[0].x) && e[0].x == -e[1].x && e[0].y == e[1].y && e[0].y.abs() == 90.0)
}

fn prepare(
    source: &DisplaySource,
    noded: &HashMap<(GeoEntityId, usize), Polygon<f64>>,
) -> Result<Prepared> {
    let mut polygons = Vec::new();
    let mut repaired = false;
    for (part, original) in source.polygons.0.iter().enumerate() {
        let original = noded.get(&(source.entity_id, part)).unwrap_or(original);
        let preserve_world_span = preserves_world_span(original);
        let mut source_positions = SourcePositions::new();
        let exterior = unwrap_ring(
            original.exterior(),
            preserve_world_span,
            None,
            &mut source_positions,
        )
        .with_context(|| format!("polygon {part} exterior"))?;
        let b = exterior.bounding_rect().unwrap();
        let holes = original
            .interiors()
            .iter()
            .enumerate()
            .map(|(i, ring)| {
                unwrap_ring(
                    ring,
                    preserve_world_span,
                    Some(b.center().x),
                    &mut source_positions,
                )
                .with_context(|| format!("polygon {part} hole {i}"))
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
            repair::verify_coverage(&[&p], &fixed, |w| w % 2 != 0).with_context(|| {
                format!("polygonizing polygon {part} changes its even/odd interior")
            })?;
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
            polygons.extend(
                dateline::cut(&p, &source_positions)
                    .with_context(|| format!("cutting polygon {part} at the dateline"))?,
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

pub(super) fn prepare_sources(sources: &[DisplaySource], prefix: &str) -> Result<Vec<Prepared>> {
    let mut sorted: Vec<_> = sources.iter().collect();
    sorted.sort_by_key(|s| s.entity_id);
    if let Some(pair) = sorted.windows(2).find(|p| p[0].entity_id == p[1].entity_id) {
        anyhow::bail!(
            "{prefix} duplicate display source ID {}",
            pair[0].entity_id.0
        );
    }
    let noded = crossings::node_sources(sources);
    let mut prepared = vec![];
    for (source, result) in sorted
        .par_iter()
        .map(|s| (*s, prepare(s, &noded)))
        .collect::<Vec<_>>()
    {
        let source = result.with_context(|| {
            format!("{prefix} id={} invalid source geometry", source.entity_id.0)
        })?;
        if source.repaired {
            eprintln!(
                "{prefix} id={} repaired source by area-preserving polygonization",
                source.id.0
            );
        }
        prepared.push(source);
    }
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use geo::Contains;
    use geo_types::Rect;

    fn prepare(source: &DisplaySource) -> Result<Prepared> {
        super::prepare(source, &HashMap::new())
    }

    #[test]
    fn dateline_cut_preserves_original_shared_vertices_and_exact_seams() {
        let (west, east) = (179.123456789, -125.12345678901235);
        let (bottom, top) = (1.123456789, 2.123456789);
        let shape = |a, b| {
            Polygon::new(
                LineString::from(vec![
                    (a, bottom),
                    (b, bottom),
                    (b, top),
                    (a, top),
                    (a, bottom),
                ]),
                vec![],
            )
        };
        let prepare_polygon = |polygon| {
            prepare(&DisplaySource {
                entity_id: GeoEntityId(42),
                polygons: MultiPolygon(vec![polygon]),
            })
            .unwrap()
            .geometry
        };
        let crossing = prepare_polygon(shape(west, east));
        for (x, other, seam) in [(west, 178.0, 180.0), (east, -120.0, -180.0)] {
            let adjacent = prepare_polygon(shape(x, other));
            let part = crossing
                .0
                .iter()
                .find(|p| p.exterior().0.iter().any(|p| p.x.signum() == x.signum()))
                .unwrap();
            for y in [bottom, top] {
                let vertex = Point { x, y };
                assert!(adjacent.0[0].exterior().0.contains(&vertex));
                assert!(
                    part.exterior().0.contains(&vertex),
                    "missing source vertex {vertex:?}"
                );
                assert!(
                    part.exterior().0.contains(&Point { x: seam, y }),
                    "missing seam vertex"
                );
            }
        }
    }

    #[test]
    fn component_union_preserves_an_uncovered_subgrid_hole() {
        let hole = Rect::new((1., 1.), (1.000000001, 1.000000001)).to_polygon();
        let west = Rect::new((0., 0.), (2., 2.)).to_polygon();
        let east = Rect::new((2., 0.), (4., 2.)).to_polygon();
        let with_hole = Polygon::new(west.exterior().clone(), vec![hole.exterior().clone()]);
        let result = prepare(&DisplaySource {
            entity_id: GeoEntityId(17),
            polygons: MultiPolygon(vec![with_hole, east]),
        })
        .unwrap();
        assert_eq!(result.geometry.0.len(), 1);
        assert_eq!(result.geometry.0[0].interiors().len(), 1);
        assert!(!result
            .geometry
            .contains(&geo_types::Point::new(1.0000000005, 1.0000000005)));
    }

    #[test]
    fn preserves_source_vertex_at_an_overlay_rounding_tie() {
        let corner = (2.0_f64.powi(-30).next_down(), 0.5);
        let polygon = Polygon::new(
            LineString::from(vec![
                (-1., -1.),
                (1., -1.),
                (1., 1.),
                corner,
                (-1., 1.),
                (-1., -1.),
            ]),
            vec![],
        );
        let mut fixed = polygon.union(&MultiPolygon(vec![]));
        restore_overlay_vertices(&mut fixed, &[&polygon]);
        assert!(fixed.0[0].exterior().0.contains(&corner.into()));
    }

    #[test]
    fn crossing_latitude_is_independent_of_collinear_segmentation() {
        let clipped = |mut bottom: Vec<(f64, f64)>| {
            bottom.extend([(-174., 0.), (150., 0.), (150., -25.)]);
            let geometry = prepare(&DisplaySource {
                entity_id: GeoEntityId(1),
                polygons: MultiPolygon(vec![Polygon::new(LineString::from(bottom), vec![])]),
            })
            .unwrap()
            .geometry;
            geometry
                .0
                .iter()
                .flat_map(rings)
                .flat_map(|r| &r.0)
                .filter(|p| p.x.abs() == 180.0 && p.y < 0.0)
                .map(|p| p.y)
                .collect::<Vec<_>>()
        };
        let expected = -50.0 / 3.0;
        for vertices in [
            vec![(150., -25.), (-174., -15.)],
            vec![(150., -25.), (168., -20.), (-174., -15.)],
        ] {
            let latitudes = clipped(vertices);
            assert!(!latitudes.is_empty());
            assert!(latitudes.iter().all(|&y| y == expected), "{latitudes:?}");
        }
    }

    #[test]
    fn joined_components_can_cover_part_or_all_of_a_source_hole() {
        for west in [2., 0.] {
            let base = Rect::new((0., 0.), (4., 4.)).to_polygon();
            let hole = Rect::new((1., 1.), (3., 3.)).to_polygon();
            let with_hole = Polygon::new(base.exterior().clone(), vec![hole.exterior().clone()]);
            let covering = Rect::new((west, 0.), (5., 4.)).to_polygon();
            let result = prepare(&DisplaySource {
                entity_id: GeoEntityId(1),
                polygons: MultiPolygon(vec![with_hole, covering]),
            })
            .unwrap();
            assert_eq!(
                result.geometry.contains(&geo_types::Point::new(1.5, 2.)),
                west == 0.
            );
            assert!(result.geometry.contains(&geo_types::Point::new(2.5, 2.)));
        }
    }

    #[test]
    fn union_detects_an_implicit_subgrid_hole_between_source_shells() {
        let gap = 1e-11;
        let components = [
            ((0., 0.), (1., 2.)),
            ((1. + gap, 0.), (2., 2.)),
            ((0., 0.), (2., 1.)),
            ((0., 1. + gap), (2., 2.)),
        ]
        .map(|(a, b)| Rect::new(a, b).to_polygon());
        let source = DisplaySource {
            entity_id: GeoEntityId(42),
            polygons: MultiPolygon(components.into()),
        };
        match prepare(&source) {
            Ok(result) => assert!(!result
                .geometry
                .contains(&geo_types::Point::new(1. + gap / 2., 1. + gap / 2.))),
            Err(error) => assert!(
                format!("{error:#}").contains("component union changes source coverage"),
                "{error:#}"
            ),
        }
    }

    #[test]
    fn repairing_a_self_touch_cannot_erase_a_tiny_hole() {
        let shell = vec![
            (0., 0.),
            (2., 0.),
            (2., 2.),
            (0., 2.),
            (0., 0.),
            (-2., 0.),
            (-2., -2.),
            (0., -2.),
            (0., 0.),
        ];
        let hole = Rect::new((1., 1.), (1.000000001, 1.000000001)).to_polygon();
        let polygon = Polygon::new(shell.into(), vec![hole.exterior().clone()]);
        let source = DisplaySource {
            entity_id: GeoEntityId(7),
            polygons: MultiPolygon(vec![polygon]),
        };
        match prepare(&source) {
            Ok(result) => assert!(!result
                .geometry
                .contains(&geo_types::Point::new(1.0000000005, 1.0000000005))),
            Err(error) => assert!(
                format!("{error:#}").contains("changes its even/odd interior"),
                "{error:#}"
            ),
        }
    }

    #[test]
    fn only_actual_unwrapped_copies_need_distinct_source_positions() {
        let coordinates = vec![
            (179., 0.),
            (179.00000000000003, 0.),
            (-179., 0.),
            (-179., 1.),
            (179., 1.),
            (179., 0.),
        ];
        let result = prepare(&DisplaySource {
            entity_id: GeoEntityId(1),
            polygons: MultiPolygon(vec![Polygon::new(coordinates.into(), vec![])]),
        })
        .unwrap();
        assert!(result
            .geometry
            .0
            .iter()
            .flat_map(rings)
            .any(|r| r.0.contains(&(179.00000000000003, 0.).into())));
    }

    #[test]
    fn winding_polar_cap_has_no_slit_at_the_original_longitude() {
        let polygon = Polygon::new(
            vec![
                (152.002, 80.),
                (-120., 81.),
                (0., 82.),
                (90., 83.),
                (152.002, 80.),
            ]
            .into(),
            vec![],
        );
        let source = DisplaySource {
            entity_id: GeoEntityId(1),
            polygons: MultiPolygon(vec![polygon]),
        };
        let record =
            super::super::build_display_records(geo_data_format::Worldview::Iso, &[source])
                .unwrap()
                .pop()
                .unwrap();
        let polygons = record.polygons.into_degrees();
        assert_eq!(polygons.len(), 1);
        assert!(
            polygons[0][0]
                .windows(2)
                .all(|edge| !(edge[0][0] == edge[1][0]
                    && edge[0][0].abs() != 180.
                    && (edge[0][1] == 90. || edge[1][1] == 90.))),
            "{polygons:?}"
        );
    }
}
