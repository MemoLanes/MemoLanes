//! Re-exports of the geo-entity types from the `geo_data_format` shared
//! crate. Definitions live there because the offline rasterizer also
//! needs to construct them.

pub use geo_data_format::{GeoEntity, GeoEntityId, GeoEntityKind, Worldview};
