use serde::{Deserialize, Serialize};

/// Stable ID from the rasterizer's frozen, append-only entity registry.
/// IDs are explicit and never reused, so a dataset change never renumbers
/// existing entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GeoEntityId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeoEntityKind {
    Continent,
    Country,
    Province,
    City,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoEntity {
    pub id: GeoEntityId,
    pub kind: GeoEntityKind,
    /// Per-kind Natural Earth identity code: continent code ("AF") for
    /// continents, `ADM0_A3` for countries. Guaranteed unique within kind.
    pub canonical_code: String,
    pub iso_a3_eh: Option<String>,
    pub name_key: String,
    pub parent_id: Option<GeoEntityId>,
    /// Pre-computed total area in m², from rasterizer.
    pub total_area_m2: u64,
}

/// Tile-level classification for the geo lookup table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TileMembership {
    /// Entire tile belongs to one entity.
    Single(GeoEntityId),
    /// Tile straddles borders — drill to block level.
    Border,
    /// Ocean / uninhabited — no entity.
    None,
}
