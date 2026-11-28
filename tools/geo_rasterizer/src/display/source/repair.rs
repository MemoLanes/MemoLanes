use super::{restore_overlay_vertices, rings, Bounds, CoordPos, Dimensions, SpatialIndex};
use anyhow::{ensure, Context, Result};
use geo::{
    line_intersection::{line_intersection, LineIntersection},
    Relate,
};
use geo_types::{Line, MultiPolygon, Polygon};
use num_rational::BigRational;
use num_traits::ToPrimitive;

pub(super) fn union(original: &[&Polygon<f64>]) -> Result<MultiPolygon<f64>> {
    let mut inputs = Vec::new();
    let mut untouched: Vec<Polygon<f64>> = Vec::new();
    for (i, polygon) in original.iter().enumerate() {
        let mut interiors = Vec::new();
        for ring in polygon.interiors() {
            let hole = Polygon::new(ring.clone(), vec![]);
            let covered = original.iter().enumerate().any(|(k, p)| {
                k != i
                    && p.relate(&hole).get(CoordPos::Inside, CoordPos::Inside)
                        == Dimensions::TwoDimensional
            });
            if !covered {
                // The union cannot change this hole. Carry its exact ring past
                // the overlay, whose integer grid can collapse small holes.
                if !untouched.iter().any(|h| h.relate(&hole).is_equal_topo()) {
                    untouched.push(hole);
                }
            } else {
                interiors.push(ring.clone());
            }
        }
        inputs.push(Polygon::new(polygon.exterior().clone(), interiors));
    }
    let mut fixed = geo::algorithm::bool_ops::unary_union(&inputs);
    restore_overlay_vertices(&mut fixed, &inputs.iter().collect::<Vec<_>>());
    for hole in untouched {
        let shell = fixed
            .0
            .iter_mut()
            .find(|p| p.relate(&hole).is_covers())
            .context("component union displaces an uncovered source hole")?;
        shell.interiors_push(hole.into_inner().0);
    }
    verify_coverage(original, &fixed, |w| w != 0)
        .context("component union changes source coverage")?;
    Ok(fixed)
}

struct Edge {
    line: Line<f64>,
    slope: BigRational,
    intercept: BigRational,
    upward: bool,
    result: bool,
}

fn exact(f: f64) -> BigRational {
    BigRational::from_float(f).expect("validated finite coordinate")
}

// Between consecutive vertices and proper crossings, edges have a fixed
// order. Compare the two unions on each open band. Exact cross-sections can
// distinguish an enclosed channel even when it is narrower than the overlay grid.
pub(super) fn verify_coverage(
    original: &[&Polygon<f64>],
    fixed: &MultiPolygon<f64>,
    input_filled: impl Fn(i32) -> bool,
) -> Result<()> {
    let mut edges = Vec::new();
    let mut levels = Vec::new();
    for (polygon, result) in original
        .iter()
        .map(|p| (*p, false))
        .chain(fixed.0.iter().map(|p| (p, true)))
    {
        for ring in rings(polygon) {
            for line in ring.lines() {
                if line.start.y == line.end.y {
                    continue;
                }
                let y0 = exact(line.start.y);
                let y1 = exact(line.end.y);
                let x0 = exact(line.start.x);
                let slope = (exact(line.end.x) - &x0) / (&y1 - &y0);
                let intercept = x0 - &slope * &y0;
                levels.extend([y0, y1]);
                edges.push(Edge {
                    line,
                    slope,
                    intercept,
                    upward: line.end.y > line.start.y,
                    result,
                });
            }
        }
    }
    let index = SpatialIndex::new(edges.iter().map(|e| Bounds::new(e.line.start, e.line.end)));
    for (i, a) in edges.iter().enumerate() {
        index.query(Bounds::new(a.line.start, a.line.end), |j| {
            let b = &edges[j];
            if i < j
                && matches!(
                    line_intersection(a.line, b.line),
                    Some(LineIntersection::SinglePoint {
                        is_proper: true,
                        ..
                    })
                )
            {
                levels.push((&b.intercept - &a.intercept) / (&a.slope - &b.slope));
            }
        });
    }
    levels.sort_unstable();
    levels.dedup();
    let two = exact(2.);
    let west = edges
        .iter()
        .map(|e| e.line.start.x.min(e.line.end.x))
        .fold(f64::INFINITY, f64::min);
    let east = edges
        .iter()
        .map(|e| e.line.start.x.max(e.line.end.x))
        .fold(f64::NEG_INFINITY, f64::max);
    for band in levels.windows(2) {
        let y = (&band[0] + &band[1]) / &two;
        let latitude = y.to_f64().unwrap();
        let mut hits = Vec::new();
        index.query(
            Bounds::new((west, latitude).into(), (east, latitude).into()),
            |i| {
                let e = &edges[i];
                if y > exact(e.line.start.y.min(e.line.end.y))
                    && y < exact(e.line.start.y.max(e.line.end.y))
                {
                    hits.push((
                        &e.slope * &y + &e.intercept,
                        e.result as usize,
                        if e.upward { 1 } else { -1 },
                    ));
                }
            },
        );
        hits.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        let mut winding = [0, 0];
        let mut spans = [Vec::new(), Vec::new()];
        for group in hits.chunk_by(|a, b| a.0 == b.0) {
            let previous = winding;
            for (_, region, direction) in group {
                winding[*region] += direction;
            }
            if input_filled(winding[0]) != input_filled(previous[0]) {
                spans[0].push(&group[0].0);
            }
            // The valid output components have disjoint interiors. A hole can
            // still wind like its shell until preparation normalizes the rings.
            if (winding[1] % 2 == 0) != (previous[1] % 2 == 0) {
                spans[1].push(&group[0].0);
            }
        }
        ensure!(
            spans[0] == spans[1],
            "different covered intervals near latitude {latitude}"
        );
    }
    Ok(())
}
