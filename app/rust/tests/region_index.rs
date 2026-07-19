use std::collections::BTreeMap;

use geo_data_format::{
    tile_index, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, TILE_COUNT,
};
use memolanes_core::{
    achievement::compute::region_state::compute_region_states,
    achievement::layer::AchievementLayer,
    geo::GeoIndex,
    journey_area_utils::compute_journey_bitmap_area,
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
    let mut cells = vec![None; CELLS_PER_TILE];
    cells[BlockKey::from_x_y(7, 7).index()] = Some(DE);
    let mut blocks: BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>> = BTreeMap::new();
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
fn attributes_area_with_rollup_and_layers() {
    let geo = synthetic_geo();

    // Per-layer coverage as cache_db would supply it: Default holds France,
    // Flight holds Germany, All is their union. Both blocks share a bit pattern.
    let fr_block = one_block(TileKey::new(0, 0), BlockKey::from_x_y(3, 4), 20);
    let de_block = one_block(TileKey::new(1, 0), BlockKey::from_x_y(7, 7), 20);
    let fr_area = compute_journey_bitmap_area(&fr_block, None);
    let de_area = compute_journey_bitmap_area(&de_block, None);
    // EU's area is the union (disjoint blocks), summed as f64 then rounded —
    // not round(fr)+round(de), which can differ by a metre from rounding.
    let mut union = fr_block.clone();
    union.merge(de_block.clone());
    let eu_area = compute_journey_bitmap_area(&union, None);

    let states = compute_region_states(
        [
            (AchievementLayer::Default, fr_block),
            (AchievementLayer::Flight, de_block),
            (AchievementLayer::All, union),
        ],
        &geo,
    );

    // France: Default layer only, area == the block oracle.
    let fr = &states[&(AchievementLayer::Default, FR)];
    assert_eq!(fr.visited_area_m2, fr_area);
    assert!(!states.contains_key(&(AchievementLayer::Flight, FR)));

    // Germany: Flight layer only.
    let de = &states[&(AchievementLayer::Flight, DE)];
    assert_eq!(de.visited_area_m2, de_area);
    assert!(!states.contains_key(&(AchievementLayer::Default, DE)));

    // EU rollup: All layer is the union of both kinds.
    let eu_all = &states[&(AchievementLayer::All, EU)];
    assert_eq!(eu_all.visited_area_m2, eu_area);
    // Per-layer EU: Default sees only FR, Flight only DE.
    assert_eq!(
        states[&(AchievementLayer::Default, EU)].visited_area_m2,
        fr_area
    );
    assert_eq!(
        states[&(AchievementLayer::Flight, EU)].visited_area_m2,
        de_area
    );
}

#[test]
fn ocean_blocks_are_ignored() {
    let geo = synthetic_geo();
    // Border tile (1,0), but a block with no geo owner.
    let states = compute_region_states(
        [(
            AchievementLayer::Default,
            one_block(TileKey::new(1, 0), BlockKey::from_x_y(0, 0), 10),
        )],
        &geo,
    );
    assert!(states.is_empty());
}
