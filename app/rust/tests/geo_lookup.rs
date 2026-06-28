use std::collections::BTreeMap;

use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, CELLS_PER_TILE,
    TILE_COUNT,
};
use memolanes_core::{
    geo::{GeoIndex, GeoLookup},
    journey_bitmap::{BlockKey, TileKey},
};

fn entity(id: u32, kind: GeoEntityKind, iso: &str, parent: Option<u32>) -> GeoEntity {
    GeoEntity {
        id: GeoEntityId(id),
        kind,
        iso_code: iso.into(),
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: 1,
    }
}

/// Build a tiny POV asset: continent EU(1) ⊃ {FR(2), DE(3)}, a `Single(FR)`
/// tile at (0,0), and a `Border` tile at (1,0) whose blocks are filled x-major
/// (`bx*128 + by`, the BlockKey convention) so block coords pass straight
/// through `GeoLookup` with no transpose.
fn synthetic_geo() -> GeoIndex {
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Country, "FR", Some(1)),
        entity(3, GeoEntityKind::Country, "DE", Some(1)),
    ];

    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[0] = TileMembership::Single(GeoEntityId(2)); // (tx,ty)=(0,0) → idx 0
    tiles[512] = TileMembership::Border; // (tx,ty)=(1,0) → x-major idx tx*512+ty

    let mut cells = vec![None; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(2, 3).index()] = Some(GeoEntityId(3)); // DE
    cells[BlockKey::from_x_y(5, 5).index()] = Some(GeoEntityId(2)); // FR
    let mut blocks: BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>> = BTreeMap::new();
    blocks.insert((1, 0), cells);

    let bytes = write_geo_data(&entities, &[], &tiles, &blocks, [0u8; 32]).unwrap();
    GeoIndex::from_bytes(&bytes).unwrap()
}

#[test]
fn entity_of_block_resolves_single_border_and_none() {
    let geo = synthetic_geo();

    // Single tile: every block resolves to the tile's one owner.
    let single = TileKey::new(0, 0);
    assert_eq!(
        geo.entity_of_block(single, BlockKey::from_x_y(0, 0)),
        Some(GeoEntityId(2))
    );
    assert_eq!(
        geo.entity_of_block(single, BlockKey::from_x_y(99, 7)),
        Some(GeoEntityId(2))
    );

    // Border tile: per-block owners, x-major; unfilled cells are ocean.
    let border = TileKey::new(1, 0);
    assert_eq!(
        geo.entity_of_block(border, BlockKey::from_x_y(2, 3)),
        Some(GeoEntityId(3))
    );
    assert_eq!(
        geo.entity_of_block(border, BlockKey::from_x_y(5, 5)),
        Some(GeoEntityId(2))
    );
    assert_eq!(geo.entity_of_block(border, BlockKey::from_x_y(0, 0)), None);

    // None tile: nothing.
    assert_eq!(
        geo.entity_of_block(TileKey::new(2, 0), BlockKey::from_x_y(1, 1)),
        None
    );
}

#[test]
fn tile_membership_does_not_decode() {
    let geo = synthetic_geo();
    assert_eq!(
        geo.tile_membership(TileKey::new(0, 0)),
        TileMembership::Single(GeoEntityId(2))
    );
    assert_eq!(
        geo.tile_membership(TileKey::new(1, 0)),
        TileMembership::Border
    );
    assert_eq!(
        geo.tile_membership(TileKey::new(2, 0)),
        TileMembership::None
    );
}

#[test]
fn entity_metadata_kinds_and_ancestors() {
    let geo = synthetic_geo();

    assert_eq!(geo.entity(GeoEntityId(2)).unwrap().iso_code, "FR");
    assert!(geo.entity(GeoEntityId(404)).is_none());

    let mut countries = geo.entities_of_kind(GeoEntityKind::Country).to_vec();
    countries.sort();
    assert_eq!(countries, vec![GeoEntityId(2), GeoEntityId(3)]);
    assert_eq!(
        geo.entities_of_kind(GeoEntityKind::Continent),
        &[GeoEntityId(1)]
    );
    assert!(geo.entities_of_kind(GeoEntityKind::City).is_empty());

    // FR → EU; continent has no parent.
    assert_eq!(geo.ancestors(GeoEntityId(2)), vec![GeoEntityId(1)]);
    assert!(geo.ancestors(GeoEntityId(1)).is_empty());
}
