use serde::{Deserialize, Serialize};

/// Stable ID from the rasterizer's frozen, append-only entity registry.
/// IDs are explicit and never reused, so a dataset change never renumbers
/// existing entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct GeoEntityId(pub u32);

/// The administrative tier. Named for the level, not for what the units are —
/// what a unit *is* lives in its display name and flags.
///
/// Tiers are POSITIONAL: an entity's tier is the deepest position any
/// worldview's tree requires, never a cross-country administrative class
/// (US counties and Chinese counties land at different tiers because their
/// ladders differ). A tree may skip tiers below that; class-like semantics
/// belong in flags.
///
/// NOTE: the serialized name-key prefixes stay `country.` / `province.` — they
/// encode the source namespace, are baked into shipped bins and name JSON, and
/// deliberately do not track these variant names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum GeoEntityKind {
    Continent,
    Admin0,
    Admin1,
    /// Empty until ADM2 data ships.
    Admin2,
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
