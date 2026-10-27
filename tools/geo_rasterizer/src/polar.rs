//! Polar polygon preparation. GeoJSON edges are linear in longitude/latitude,
//! not great-circle arcs. Densify those edges before LAEA projection; in
//! particular a constant-latitude cap closure must become a circular arc.
//! Natural Earth's dateline cuts and pole-closure edges are retained: opposite
//! sides of a dateline cut project onto the same radial line.

use std::collections::BTreeMap;

use bitmap_geometry::{project_polar_laea, BitmapPlane, BITMAP_WIDTH, MERCATOR_MAX_LATITUDE};
use geo::{BooleanOps, BoundingRect};
use geo_types::{Coord, LineString, MultiPolygon, Polygon, Rect};

/// Run after province policy and geometry merges, so dropped whole-country
/// placeholders (such as Antarctica's) do not trigger this guard.
pub fn validate_no_polar_provinces(
    geometries: &BTreeMap<String, MultiPolygon<f64>>,
) -> anyhow::Result<()> {
    let unsupported: Vec<_> = geometries
        .iter()
        .filter(|(_, geometry)| {
            geometry.bounding_rect().is_some_and(|bounds| {
                bounds.min().y < -MERCATOR_MAX_LATITUDE || bounds.max().y > MERCATOR_MAX_LATITUDE
            })
        })
        .map(|(code, _)| code.as_str())
        .collect();
    anyhow::ensure!(
        unsupported.is_empty(),
        "retained provinces extend beyond the Mercator latitude limit: {}; \
         polar province refinement is not supported",
        unsupported.join(", ")
    );
    Ok(())
}

pub fn project_geometries(
    geometries: &BTreeMap<String, MultiPolygon<f64>>,
    plane: BitmapPlane,
) -> BTreeMap<String, MultiPolygon<f64>> {
    assert_ne!(plane, BitmapPlane::Mercator);
    // Clip away the opposite hemisphere before projection to avoid the LAEA
    // antipode singularity. This artificial boundary is far outside the cap;
    // cap clipping is done on the raster, preserving partial boundary blocks.
    let (lo, hi) = if plane == BitmapPlane::NorthPolar {
        (0.0, 90.0)
    } else {
        (-90.0, 0.0)
    };
    let hemisphere = Rect::new(Coord { x: -180.0, y: lo }, Coord { x: 180.0, y: hi }).to_polygon();
    geometries
        .iter()
        .filter_map(|(code, geometry)| {
            let relevant: Vec<_> = geometry
                .0
                .iter()
                .filter(|polygon| {
                    polygon.bounding_rect().is_some_and(|b| match plane {
                        BitmapPlane::NorthPolar => b.max().y > MERCATOR_MAX_LATITUDE,
                        BitmapPlane::SouthPolar => b.min().y < -MERCATOR_MAX_LATITUDE,
                        BitmapPlane::Mercator => unreachable!(),
                    })
                })
                .cloned()
                .collect();
            if relevant.is_empty() {
                return None;
            }
            let clipped = MultiPolygon(relevant).intersection(&hemisphere);
            let projected = MultiPolygon(
                clipped
                    .0
                    .iter()
                    .map(|polygon| {
                        Polygon::new(
                            project_ring(polygon.exterior(), plane),
                            polygon
                                .interiors()
                                .iter()
                                .map(|r| project_ring(r, plane))
                                .collect(),
                        )
                    })
                    .collect(),
            );
            Some((code.clone(), projected))
        })
        .collect()
}

fn project(lnglat: Coord<f64>, plane: BitmapPlane) -> Coord<f64> {
    let (x, y) = project_polar_laea(lnglat.x, lnglat.y, plane);
    Coord {
        x: x / BITMAP_WIDTH as f64,
        y: y / BITMAP_WIDTH as f64,
    }
}

fn project_ring(ring: &LineString<f64>, plane: BitmapPlane) -> LineString<f64> {
    let mut points = Vec::new();
    if let Some(&first) = ring.0.first() {
        points.push(project(first, plane));
    }
    for edge in ring.0.windows(2) {
        subdivide(edge[0], edge[1], plane, 0, &mut points);
    }
    LineString(points)
}

fn subdivide(
    a: Coord<f64>,
    b: Coord<f64>,
    plane: BitmapPlane,
    depth: u8,
    out: &mut Vec<Coord<f64>>,
) {
    let mid = Coord {
        x: (a.x + b.x) * 0.5,
        y: (a.y + b.y) * 0.5,
    };
    let pa = project(a, plane);
    let pb = project(b, plane);
    let pm = project(mid, plane);
    let error = (pm.x - (pa.x + pb.x) * 0.5).hypot(pm.y - (pa.y + pb.y) * 0.5);
    // Angular bounds also catch inflections whose midpoint lies on the chord.
    if depth < 24 && (error > 0.05 || (a.x - b.x).abs() > 1.0 || (a.y - b.y).abs() > 1.0) {
        subdivide(a, mid, plane, depth + 1, out);
        subdivide(mid, b, plane, depth + 1, out);
    } else {
        out.push(pb);
    }
}
