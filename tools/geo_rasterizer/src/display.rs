//! The display graph is independent of rasterization. Node and arc identity
//! come from the source, never from the storage grid.

mod source;
mod spatial;
mod topology;
mod validation;

use anyhow::Result;
use geo_data_format::{DisplayRecord, GeoEntityId, Worldview};
use geo_types::{Coord, LineString, MultiPolygon, Polygon};

// Only compensates for floating-point arithmetic on source coordinates.
// More than six orders of magnitude smaller than the fine storage grid.
const ROUNDOFF: f64 = 2e-12;

type Point = Coord<f64>;

pub struct DisplaySource {
    pub entity_id: GeoEntityId,
    pub polygons: MultiPolygon<f64>,
}

fn rings(polygon: &Polygon<f64>) -> impl Iterator<Item = &LineString<f64>> {
    std::iter::once(polygon.exterior()).chain(polygon.interiors())
}

fn at_seam(x: f64) -> bool {
    x.abs() == 180.0
}

fn point_key(p: Point) -> (u64, u64) {
    let bits = |x: f64| if x == 0.0 { 0 } else { x.to_bits() };
    (bits(p.x), bits(p.y))
}

fn parameter(a: Point, b: Point, p: Point) -> f64 {
    let d = b - a;
    let v = p - a;
    (d.x * v.x + d.y * v.y) / (d.x * d.x + d.y * d.y)
}

pub fn build_display_records(
    worldview: Worldview,
    sources: &[DisplaySource],
) -> Result<Vec<DisplayRecord>> {
    let prefix = format!("[display:{}]", worldview.spec().id);
    let prepared = source::prepare_sources(sources, &prefix)?;
    Ok(validation::validated_records(
        worldview,
        &prepared,
        sources.len(),
        &prefix,
    ))
}
