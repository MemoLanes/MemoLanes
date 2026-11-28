use super::{point_key, restore_overlay_vertices, rings, Point, SourcePositions};
use anyhow::{ensure, Result};
use geo::{Area, BooleanOps, BoundingRect, MapCoords, Validation};
use geo_types::{LineString, MultiPolygon, Polygon, Rect};
use num_rational::BigRational;
use num_traits::ToPrimitive;
use std::collections::HashSet;

type PointKey = (u64, u64);

/// Split the input edges first, so the overlay only decides how to connect
/// them along the meridian. Every other emitted edge must match a source edge.
pub(super) fn cut(polygon: &Polygon<f64>, source: &SourcePositions) -> Result<Vec<Polygon<f64>>> {
    let bounds = polygon.bounding_rect().unwrap();
    let first = (bounds.min().x / 360.0).floor() as i32;
    let last = (bounds.max().x / 360.0).ceil() as i32;
    let worlds = first..=last;
    let mut result = Vec::new();
    for n in worlds {
        let shift = f64::from(n) * 360.0;
        let (west, east) = (shift - 180.0, shift + 180.0);
        if west >= bounds.max().x || east <= bounds.min().x {
            continue;
        }
        let split = |ring: &LineString<f64>| {
            let mut points = vec![ring.0[0]];
            for pair in ring.0.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (left, right) = if a.x < b.x { (a, b) } else { (b, a) };
                for seam in if a.x < b.x {
                    [west, east]
                } else {
                    [east, west]
                } {
                    if left.x < seam && seam < right.x {
                        points.push(crossing(left, right, seam, source));
                    }
                }
                points.push(b);
            }
            LineString(points)
        };
        let noded = Polygon::new(
            split(polygon.exterior()),
            polygon.interiors().iter().map(split).collect(),
        );
        // Filtering the noded rings closes each excursion along the slab wall.
        // Disconnected lobes or holes opened by the cut need polygonization.
        let clip = |ring: &LineString<f64>| {
            LineString(
                ring.0
                    .iter()
                    .copied()
                    .filter(|p| (west..=east).contains(&p.x))
                    .collect(),
            )
        };
        let clipped = Polygon::new(
            clip(noded.exterior()),
            noded
                .interiors()
                .iter()
                .map(clip)
                .filter(|r| r.0.first().is_some_and(|a| r.0.iter().any(|p| p.x != a.x)))
                .collect(),
        );
        let pieces = if clipped.is_valid() && clipped.unsigned_area() > 0.0 {
            MultiPolygon(vec![clipped])
        } else {
            let slab = Rect::new((west, -90.0), (east, 90.0)).to_polygon();
            let mut pieces = noded.intersection(&slab);
            restore_overlay_vertices(&mut pieces, &[&noded, &slab]);
            pieces
        };
        let edges = |polygons: &[Polygon<f64>]| -> HashSet<(PointKey, PointKey)> {
            polygons
                .iter()
                .flat_map(rings)
                .flat_map(|ring| ring.0.windows(2))
                .filter(|pair| {
                    let (a, b) = (pair[0].x, pair[1].x);
                    (west..=east).contains(&a)
                        && (west..=east).contains(&b)
                        && !(a == b && (a == west || a == east))
                })
                .map(|pair| {
                    let (a, b) = (point_key(pair[0]), point_key(pair[1]));
                    (a.min(b), a.max(b))
                })
                .collect()
        };
        ensure!(
            edges(&pieces.0) == edges(std::slice::from_ref(&noded)),
            "dateline cut lost or displaced a source boundary in [{west}, {east}]"
        );
        for piece in pieces.0 {
            let wrapped = piece.map_coords(|p| {
                if p.x == west || p.x == east {
                    Point {
                        x: if p.x == west { -180.0 } else { 180.0 },
                        y: p.y,
                    }
                } else {
                    source.get(&point_key(p)).copied().unwrap_or(Point {
                        x: p.x - shift,
                        y: p.y,
                    })
                }
            });
            ensure!(
                rings(&wrapped)
                    .flat_map(|r| &r.0)
                    .all(|p| p.x.abs() <= 180.0 && p.y.abs() <= 90.0),
                "dateline cut produced an out-of-domain vertex"
            );
            result.push(wrapped);
        }
    }
    let area = polygon.unsigned_area();
    let cut_area: f64 = result.iter().map(Area::unsigned_area).sum();
    ensure!(
        (cut_area - area).abs() <= area * 1e-6,
        "dateline cut changes source area"
    );
    Ok(result)
}

// The exact rational ordinate rounds once, so subdividing a collinear shared
// edge cannot change its seam node by one bit.
fn crossing(left: Point, right: Point, seam: f64, source: &SourcePositions) -> Point {
    let exact = |n: f64| BigRational::from_float(n).expect("validated finite coordinate");
    let a = source.get(&point_key(left)).map_or_else(
        || exact(seam) - exact(left.x),
        |p| exact(180.0) - exact(p.x),
    );
    let b = source.get(&point_key(right)).map_or_else(
        || exact(right.x) - exact(seam),
        |p| exact(p.x) + exact(180.0),
    );
    let y = (exact(left.y) * &b + exact(right.y) * &a) / (a + b);
    Point {
        x: seam,
        y: y.to_f64()
            .expect("latitude lies between finite source endpoints"),
    }
}
