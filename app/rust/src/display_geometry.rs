use anyhow::Result;
use geo_data_format::{DisplayGeometry, GeoEntityId};

use crate::api::achievement::{RegionGeometry, Worldview};
use crate::utils::MapBounds;

pub fn feature(
    id: GeoEntityId,
    worldview: Worldview,
    geometry: DisplayGeometry,
) -> Result<RegionGeometry> {
    let polygons: Vec<Vec<Vec<[f64; 2]>>> = geometry
        .polygons
        .into_iter()
        .map(|polygon| {
            polygon
                .into_iter()
                .map(|ring| {
                    ring.into_iter()
                        .map(|[x, y]| [x as f64 / 100_000.0, y as f64 / 100_000.0])
                        .collect()
                })
                .collect()
        })
        .collect();
    let feature = match polygons.as_slice() {
        [polygon] => geojson_feature(id, "Polygon", polygon),
        polygons => geojson_feature(id, "MultiPolygon", polygons),
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

fn geojson_feature(
    id: GeoEntityId,
    geometry_type: &str,
    coordinates: impl serde::Serialize,
) -> serde_json::Value {
    serde_json::json!({
        "type": "Feature",
        "id": id.0,
        "properties": { "entity_id": id.0 },
        "geometry": { "type": geometry_type, "coordinates": coordinates }
    })
}
