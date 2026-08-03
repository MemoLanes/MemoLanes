use std::collections::BTreeMap;

use geo_data_format::{
    tile_index, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};
use memolanes_core::{
    achievement::attribution,
    geo::GeoIndex,
    journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey},
};

const FR: GeoEntityId = GeoEntityId(2);
const DE: GeoEntityId = GeoEntityId(3);
const EU: GeoEntityId = GeoEntityId(1);

fn entity(id: u32, kind: GeoEntityKind, iso: &str, parent: Option<u32>, area: u64) -> GeoEntity {
    GeoEntity {
        id: GeoEntityId(id),
        kind,
        canonical_code: iso.into(),
        iso_a3_eh: None,
        name_key: format!("k.{iso}"),
        parent_id: parent.map(GeoEntityId),
        total_area_m2: area,
    }
}

/// Geo where the whole tile (0,0) is France and tile (1,0) is a border tile
/// owned block-by-block by Germany. EU is their parent continent. Entity total
/// areas are deliberately tiny so a single visited block completes them.
fn synthetic_geo() -> GeoIndex {
    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None, 1),
        entity(2, GeoEntityKind::Country, "FR", Some(1), 1),
        entity(3, GeoEntityKind::Country, "DE", Some(1), 1),
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[tile_index(0, 0)] = TileMembership::Single(FR);
    tiles[tile_index(1, 0)] = TileMembership::Border;
    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(7, 7).index()] = DE.0;
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

/// A bitmap with `bits` pixels set in one block of one tile.
fn one_block(tile: TileKey, block: BlockKey, bits: u32) -> JourneyBitmap {
    let mut bm = JourneyBitmap::new();
    let mut b = Block::new();
    for i in 0..bits {
        b.set_point((i % 64) as u8, (i / 64) as u8, true);
    }
    bm.get_tile_mut_or_insert_empty(&tile).set(&block, b);
    bm
}

#[test]
fn attributes_area_with_rollup() {
    let geo = synthetic_geo();
    let fr_block = one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 20);
    let de_block = one_block(TileKey::new(1, 0), BlockKey::from_x_y(7, 7), 20);

    let fr_only = attribution::attribute(&fr_block, &geo);
    assert!(fr_only.contains_key(&FR));
    assert!(!fr_only.contains_key(&DE));
    // Continent rollup: EU carries France's area.
    assert_eq!(fr_only[&EU], fr_only[&FR]);

    let mut union = fr_block.clone();
    union.merge(de_block.clone());
    let both = attribution::attribute(&union, &geo);

    // Subtree-inclusive: EU is credited each block alongside its owner, so its
    // total is the sum of the two addends on the right. Exact because the areas
    // are integer cm2.
    assert_eq!(both[&EU], both[&FR] + both[&DE]);
    assert_eq!(both[&FR], fr_only[&FR]);
}

/// Continent EU(1) ⊃ country FR(2) ⊃ province IDF(4). Tile (0,0) is entirely
/// IDF, so a block there must credit all three.
#[test]
fn a_province_block_credits_its_country_and_continent() {
    const IDF: GeoEntityId = GeoEntityId(4);

    let entities = [
        entity(1, GeoEntityKind::Continent, "EU", None, 100),
        entity(2, GeoEntityKind::Country, "FR", Some(1), 50),
        entity(4, GeoEntityKind::Province, "FR-IDF", Some(2), 10),
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[tile_index(0, 0)] = TileMembership::Single(IDF);
    let bytes = write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap();
    let geo = GeoIndex::from_bytes(&bytes).unwrap();

    let by_entity = attribution::attribute(
        &one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 20),
        &geo,
    );
    let province_area = by_entity[&IDF];
    assert!(province_area > 0);
    assert_eq!(
        by_entity[&FR], province_area,
        "country must inherit the province's area"
    );
    assert_eq!(
        by_entity[&EU], province_area,
        "continent must inherit it too"
    );
}

#[test]
fn ocean_blocks_are_ignored() {
    let geo = synthetic_geo();
    // Border tile (1,0), but a block with no geo owner.
    let states = attribution::attribute(
        &one_block(TileKey::new(1, 0), BlockKey::from_x_y(0, 0), 10),
        &geo,
    );
    assert!(states.is_empty());
}
