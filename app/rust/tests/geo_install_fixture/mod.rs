#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use chrono::NaiveDate;
use geo_data_format::{
    tile_index, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, Worldview,
    CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};
use memolanes_core::achievement::layer::AchievementLayer;
use memolanes_core::journey_bitmap::{Block, BlockKey, JourneyBitmap, TileKey};
use memolanes_core::journey_data::JourneyData;
use memolanes_core::journey_header::JourneyKind;
use memolanes_core::storage::Storage;
use tempdir::TempDir;

pub const HASH_A: [u8; 32] = [0xA5; 32];
pub const HASH_B: [u8; 32] = [0x5A; 32];
pub const EU: GeoEntityId = GeoEntityId(1);
pub const FR: GeoEntityId = GeoEntityId(2);
pub const BORDER_TILE: TileKey = TileKey { x: 1, y: 0 };

pub fn fr_block() -> BlockKey {
    BlockKey::from_x_y(5, 5)
}

/// EU ⊃ FR; tile (0,0) is all FR, tile (1,0) is a border tile whose only
/// owned block is `fr_block()`.
pub fn asset(provenance_hash: [u8; 32]) -> Vec<u8> {
    let entities = [
        GeoEntity {
            id: EU,
            kind: GeoEntityKind::Continent,
            canonical_code: "EU".into(),
            iso_a3_eh: None,
            name_key: "k.EU".into(),
            parent_id: None,
            total_area_m2: 1,
        },
        GeoEntity {
            id: FR,
            kind: GeoEntityKind::Admin0,
            canonical_code: "FR".into(),
            iso_a3_eh: None,
            name_key: "k.FR".into(),
            parent_id: Some(EU),
            total_area_m2: 1,
        },
    ];
    let mut tiles = vec![TileMembership::None; TILE_COUNT];
    tiles[tile_index(0, 0)] = TileMembership::Single(FR);
    tiles[tile_index(BORDER_TILE.x, BORDER_TILE.y)] = TileMembership::Border;
    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[fr_block().index()] = FR.0;
    let mut blocks = BTreeMap::new();
    blocks.insert((BORDER_TILE.x, BORDER_TILE.y), cells);
    write_geo_data(
        &entities,
        Worldview::Iso.spec().id,
        &tiles,
        &blocks,
        provenance_hash,
    )
    .unwrap()
}

pub fn sub(dir: &TempDir, name: &str) -> String {
    let p = dir.path().join(name);
    fs::create_dir_all(&p).unwrap();
    p.into_os_string().into_string().unwrap()
}

pub fn storage(dir: &TempDir) -> Storage {
    Storage::init(sub(dir, "t"), sub(dir, "d"), sub(dir, "s"), sub(dir, "c")).unwrap()
}

/// Provenance hash of the geo lookup the achievement reader currently sees.
pub fn active_hash(storage: &Storage) -> Option<[u8; 32]> {
    storage
        .with_achievement_read(|reader| Ok(reader.geo().ok().map(|geo| geo.provenance_hash())))
        .unwrap()
}

/// One journey covering `bits` points of `FR_BLOCK` in the border tile.
pub fn insert_border_journey(storage: &Storage, day: u32, bits: u32) -> anyhow::Result<String> {
    let mut block = Block::new();
    for i in 0..bits {
        block.set_point((i % 64) as u8, (i / 64) as u8, true);
    }
    let mut bitmap = JourneyBitmap::new();
    bitmap
        .get_tile_mut_or_insert_empty(&BORDER_TILE)
        .set(&fr_block(), block);
    storage.with_db_txn(|txn| {
        txn.create_and_insert_journey(
            NaiveDate::from_ymd_opt(2025, 1, day).unwrap(),
            None,
            None,
            None,
            JourneyKind::DefaultKind,
            None,
            JourneyData::Bitmap(bitmap),
        )
    })
}

pub fn fr_area_m2(storage: &Storage) -> anyhow::Result<u64> {
    storage.with_achievement_read(|reader| {
        Ok(reader
            .region_areas(AchievementLayer::Default, &[FR])?
            .get(&FR)
            .copied()
            .unwrap_or(0))
    })
}

/// Flip the tail of the file: same size, header and tile index intact, the
/// border blob region no longer decodes.
pub fn corrupt_blob_region(path: &Path) {
    let mut bytes = fs::read(path).unwrap();
    let n = bytes.len();
    for b in &mut bytes[n - 16..] {
        *b ^= 0xFF;
    }
    fs::write(path, &bytes).unwrap();
}
