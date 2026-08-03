use std::collections::BTreeMap;

use geo_data_format::{
    write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, NO_ENTITY, TILE_COUNT, TILE_GRID_WIDTH,
};
use memolanes_core::{
    geo::{GeoIndex, GeoLookup},
    journey_bitmap::{BlockKey, TileKey},
};

fn entity(id: u32, kind: GeoEntityKind, iso: &str, parent: Option<u32>) -> GeoEntity {
    GeoEntity {
        id: GeoEntityId(id),
        kind,
        canonical_code: iso.into(),
        iso_a3_eh: None,
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: 1,
    }
}

fn synthetic_geo() -> GeoIndex {
    synthetic_geo_with_singles_at(&[])
}

/// Build a tiny worldview asset: continent EU(1) ⊃ {FR(2), DE(3)}, a `Single(FR)`
/// tile at (0,0), and a `Border` tile at (1,0) whose blocks are filled x-major
/// (`bx*128 + by`, the BlockKey convention) so block coords pass straight
/// through `GeoLookup` with no transpose. `extra_singles` puts `Single(FR)` at
/// further raw tile-grid indices.
fn synthetic_geo_with_singles_at(extra_singles: &[usize]) -> GeoIndex {
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Country, "FR", Some(1)),
        entity(3, GeoEntityKind::Country, "DE", Some(1)),
    ];

    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[0] = TileMembership::Single(GeoEntityId(2)); // (tx,ty)=(0,0) → idx 0
    tiles[512] = TileMembership::Border; // (tx,ty)=(1,0) → x-major idx tx*512+ty
    for idx in extra_singles {
        tiles[*idx] = TileMembership::Single(GeoEntityId(2));
    }

    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(2, 3).index()] = 3; // DE
    cells[BlockKey::from_x_y(5, 5).index()] = 2; // FR
    let mut blocks: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    blocks.insert((1, 0), cells);

    let bytes = write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &blocks,
        [0u8; 32],
    )
    .unwrap();
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

/// Tile keys real `add_line` calls produce past the Mercator latitude limit
/// (±85.0511°): `journey_bitmap` masks tile x to the map width but never masks
/// tile y, and nothing clamps latitude. South of the limit y runs off the
/// 512-tile grid; north of it the Mercator y goes negative and `as u16` wraps.
/// x*512 + y lands past the grid only for a big enough x, so the rest alias
/// onto an unrelated in-grid tile instead of panicking.
const OUT_OF_GRID_PAST_END: [(u16, u16); 2] = [
    (511, 529),   // lng 179.5°, lat -86.0° → raw index 262161
    (426, 65462), // lng 120°, lat 88° → raw index 283574
];
const OUT_OF_GRID_ALIASING: [(u16, u16); 2] = [
    (256, 552),   // lng 0°, lat -87° → raw index 131624
    (256, 65461), // lng 0°, lat 88° → raw index 196533
];

#[test]
fn out_of_grid_tile_key_does_not_index_past_the_tile_grid() {
    let geo = synthetic_geo();
    for (x, y) in OUT_OF_GRID_PAST_END {
        let key = TileKey::new(x, y);
        assert_eq!(
            geo.entity_of_block(key, BlockKey::from_x_y(0, 0)),
            None,
            "tile ({x},{y})"
        );
        assert_eq!(
            geo.tile_membership(key),
            TileMembership::None,
            "tile ({x},{y})"
        );
    }
}

#[test]
fn out_of_grid_tile_key_is_not_aliased_onto_an_in_grid_tile() {
    // Seed the tiles those raw indices land on, so an unguarded lookup reports
    // their owner instead of quietly reading an empty tile.
    let aliased: Vec<usize> = OUT_OF_GRID_ALIASING
        .iter()
        .map(|(x, y)| *x as usize * TILE_GRID_WIDTH + *y as usize)
        .collect();
    let geo = synthetic_geo_with_singles_at(&aliased);

    for (x, y) in OUT_OF_GRID_ALIASING {
        let key = TileKey::new(x, y);
        assert_eq!(
            geo.entity_of_block(key, BlockKey::from_x_y(0, 0)),
            None,
            "tile ({x},{y})"
        );
        assert_eq!(
            geo.tile_membership(key),
            TileMembership::None,
            "tile ({x},{y})"
        );
    }
}

#[test]
fn out_of_grid_tile_x_resolves_to_none() {
    // `of_tile_bytes_without_validation` takes tile keys verbatim from a FoW
    // import file, so x can be out of the grid too — no projection involved.
    let geo = synthetic_geo();
    for (x, y) in [(TILE_GRID_WIDTH as u16, 0), (u16::MAX, u16::MAX)] {
        let key = TileKey::new(x, y);
        assert_eq!(
            geo.entity_of_block(key, BlockKey::from_x_y(0, 0)),
            None,
            "tile ({x},{y})"
        );
        assert_eq!(
            geo.tile_membership(key),
            TileMembership::None,
            "tile ({x},{y})"
        );
    }
}

#[test]
fn entity_metadata_kinds_and_ancestors() {
    let geo = synthetic_geo();

    assert_eq!(geo.entity(GeoEntityId(2)).unwrap().canonical_code, "FR");
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

#[test]
fn asset_declares_its_worldview_id() {
    let entities = [entity(1, GeoEntityKind::Continent, "EU", None)];
    let tiles = vec![TileMembership::None; TILE_COUNT];
    let bytes = write_geo_data(
        &entities,
        Worldview::Chn.spec().id,
        &tiles,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap();
    let geo = GeoIndex::from_bytes(&bytes).unwrap();
    assert_eq!(geo.worldview_id(), "chn");
}

#[test]
fn interleaved_tile_lookups_do_not_leak_across_tiles() {
    // Build a fixture with two border tiles at different indices holding
    // different entities for the same block coordinate.
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None),
        entity(2, GeoEntityKind::Country, "FR", Some(1)),
        entity(3, GeoEntityKind::Country, "DE", Some(1)),
    ];

    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[512] = TileMembership::Border; // (tx,ty)=(1,0) → idx 1*512+0=512
    tiles[1024] = TileMembership::Border; // (tx,ty)=(2,0) → idx 2*512+0=1024

    let block_idx = BlockKey::from_x_y(3, 5).index();
    let mut cells1 = vec![NO_ENTITY; CELLS_PER_TILE];
    cells1[block_idx] = 2; // FR in first tile
    let mut cells2 = vec![NO_ENTITY; CELLS_PER_TILE];
    cells2[block_idx] = 3; // DE in second tile

    let mut blocks: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    blocks.insert((1, 0), cells1);
    blocks.insert((2, 0), cells2);

    let bytes = write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &blocks,
        [0u8; 32],
    )
    .unwrap();
    let geo = GeoIndex::from_bytes(&bytes).unwrap();

    let border_tile_a = TileKey::new(1, 0);
    let border_tile_b = TileKey::new(2, 0);
    let block = BlockKey::from_x_y(3, 5);

    let from_a = geo.entity_of_block(border_tile_a, block);
    let from_b = geo.entity_of_block(border_tile_b, block);
    assert_ne!(
        from_a, from_b,
        "fixture must distinguish the two tiles for this test to mean anything"
    );

    // Alternate repeatedly: a one-entry memo is evicted on every switch, so a
    // stale hit would show up immediately.
    for _ in 0..8 {
        assert_eq!(geo.entity_of_block(border_tile_a, block), from_a);
        assert_eq!(geo.entity_of_block(border_tile_b, block), from_b);
        assert_eq!(geo.entity_of_block(border_tile_a, block), from_a);
    }
}
