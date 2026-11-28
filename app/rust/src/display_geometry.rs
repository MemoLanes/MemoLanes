use anyhow::Result;
use geo_data_format::{DisplayGeometry, GeoEntityId};
use serde::Serialize;

use crate::api::achievement::{RegionGeometry, Worldview};
use crate::utils::MapBounds;

#[derive(Serialize)]
#[serde(tag = "type")]
struct Feature<'a> {
    id: GeoEntityId,
    properties: Properties,
    geometry: Geometry<'a>,
}

#[derive(Serialize)]
struct Properties {
    entity_id: GeoEntityId,
}

type LinearRing = Vec<[f64; 2]>;

#[derive(Serialize)]
#[serde(tag = "type", content = "coordinates")]
enum Geometry<'a> {
    Polygon(&'a [LinearRing]),
    MultiPolygon(&'a [Vec<LinearRing>]),
}

pub fn feature(
    id: GeoEntityId,
    worldview: Worldview,
    geometry: DisplayGeometry,
) -> Result<RegionGeometry> {
    let polygons = geometry.polygons.into_degrees();
    let feature = Feature {
        id,
        properties: Properties { entity_id: id },
        geometry: match polygons.as_slice() {
            [polygon] => Geometry::Polygon(polygon),
            polygons => Geometry::MultiPolygon(polygons),
        },
    };
    let bounds = geometry.bounds;
    Ok(RegionGeometry {
        entity_id: id,
        worldview,
        bounds: MapBounds {
            west: bounds.west,
            south: bounds.south,
            east: bounds.east,
            north: bounds.north,
        },
        feature_geojson: serde_json::to_string(&feature)?,
    })
}
