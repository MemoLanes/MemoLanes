use std::collections::BTreeMap;

use geo_data_format::{
    tile_xy, write_geo_data, GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile, TileEntry,
    TileMembership, CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};

fn entity(id: u32, iso: &str) -> GeoEntity {
    GeoEntity {
        id: GeoEntityId(id),
        kind: GeoEntityKind::Admin0,
        canonical_code: iso.into(),
        iso_a3_eh: Some(iso.into()),
        name_key: format!("c.{iso}"),
        parent_id: None,
        total_area_m2: 1,
    }
}

/// Write `bytes` to a file and open it; the dir must outlive the `GeoData`.
fn open(bytes: &[u8]) -> (tempfile::TempDir, anyhow::Result<GeoData>) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo_data_iso.bin");
    std::fs::write(&path, bytes).unwrap();
    let data = GeoData::open(&path);
    (dir, data)
}

#[test]
fn round_trip_single_border_none() {
    let mut tl = vec![TileMembership::None; TILE_COUNT];
    tl[0] = TileMembership::Single(GeoEntityId(7));
    tl[1] = TileMembership::Border; // x-major: tile idx 1 → tx=0, ty=1
    let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
    cells[5] = 7;
    let mut bl: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    bl.insert((0, 1), cells);

    let bytes = write_geo_data(&[entity(7, "AAA")], "iso", &tl, &bl, [3u8; 32]).unwrap();
    let (_dir, gd) = open(&bytes);
    let gd = gd.unwrap();

    assert_eq!(gd.provenance_hash, [3u8; 32]);
    assert_eq!(gd.worldview_id, "iso");
    let entities = gd.entities().unwrap();
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].canonical_code, "AAA");
    assert_eq!(gd.tile_index.get(0), TileEntry::Single(GeoEntityId(7)));
    assert_eq!(gd.tile_index.get(1), TileEntry::Border(0));
    assert!(matches!(gd.tile_index.get(2), TileEntry::None));
    let pt = PackedTile::from_compressed_bytes(&gd.border_blobs.get(0).unwrap());
    assert_eq!(pt.lookup(5), Some(GeoEntityId(7)));
    assert_eq!(pt.lookup(6), None);
    assert!(gd.border_blobs.get(1).is_err());
}

/// Three border tiles at raw indices 1, 600 and 4000 owned by AAA/BBB/CCC.
fn three_border_tiles() -> Vec<u8> {
    let mut tl = vec![TileMembership::None; TILE_COUNT];
    let mut bl: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    for (idx, id) in [(1usize, 7u32), (600, 8), (4000, 9)] {
        tl[idx] = TileMembership::Border;
        let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
        cells[0] = id;
        bl.insert(tile_xy(idx), cells);
    }
    tl[2] = TileMembership::Single(GeoEntityId(8));
    let entities = [entity(7, "AAA"), entity(8, "BBB"), entity(9, "CCC")];
    write_geo_data(&entities, "iso", &tl, &bl, [5u8; 32]).unwrap()
}

#[test]
fn border_blobs_keep_their_tile_order() {
    let (_dir, gd) = open(&three_border_tiles());
    let gd = gd.unwrap();

    for (i, id) in [7u32, 8, 9].into_iter().enumerate() {
        let TileEntry::Border(blob) = gd.tile_index.get([1usize, 600, 4000][i]) else {
            panic!("tile {i} should be Border");
        };
        assert_eq!(blob as usize, i);
        let pt = PackedTile::from_compressed_bytes(&gd.border_blobs.get(blob).unwrap());
        assert_eq!(pt.lookup(0), Some(GeoEntityId(id)));
    }
    assert!(gd.border_blobs.get(3).is_err());
}

#[test]
fn rejects_bad_magic() {
    let tl = vec![TileMembership::None; TILE_COUNT];
    let mut b = write_geo_data(&[], "iso", &tl, &BTreeMap::new(), [0u8; 32]).unwrap();
    b[0] = b'X';
    let (_dir, gd) = open(&b);
    assert!(gd.is_err());
}

#[test]
fn rejects_wrong_tile_count() {
    let tl = vec![TileMembership::None; 10];
    assert!(write_geo_data(&[], "iso", &tl, &BTreeMap::new(), [0u8; 32]).is_err());
}

#[test]
fn rejects_truncated_file() {
    let bytes = three_border_tiles();
    let (_dir, gd) = open(&bytes[..bytes.len() - 1]);
    assert!(gd.is_err());
}

#[test]
fn rejects_missing_file() {
    let dir = tempfile::tempdir().unwrap();
    assert!(GeoData::open(&dir.path().join("absent.bin")).is_err());
}

#[test]
fn tile_index_resolves_positions_around_word_boundaries() {
    let mut tl = vec![TileMembership::None; TILE_COUNT];
    let singles = [0usize, 1, 63, 64, 65, 127, 128, 4095, 4096, TILE_COUNT - 1];
    for (n, idx) in singles.iter().enumerate() {
        tl[*idx] = TileMembership::Single(GeoEntityId(100 + n as u32));
    }
    let mut bl: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    for idx in [62usize, 66, TILE_COUNT - 2] {
        tl[idx] = TileMembership::Border;
        bl.insert(tile_xy(idx), vec![NO_ENTITY; CELLS_PER_TILE]);
    }
    let entities: Vec<GeoEntity> = (0..singles.len())
        .map(|n| entity(100 + n as u32, "AAA"))
        .collect();
    let bytes = write_geo_data(&entities, "iso", &tl, &bl, [0u8; 32]).unwrap();
    let (_dir, gd) = open(&bytes);
    let gd = gd.unwrap();

    for (n, idx) in singles.iter().enumerate() {
        assert_eq!(
            gd.tile_index.get(*idx),
            TileEntry::Single(GeoEntityId(100 + n as u32)),
            "tile {idx}"
        );
    }
    assert_eq!(gd.tile_index.get(62), TileEntry::Border(0));
    assert_eq!(gd.tile_index.get(66), TileEntry::Border(1));
    assert_eq!(gd.tile_index.get(TILE_COUNT - 2), TileEntry::Border(2));
    for idx in [2usize, 61, 67, 129, 4094, 4097, 100_000] {
        assert_eq!(gd.tile_index.get(idx), TileEntry::None, "tile {idx}");
    }
}

#[test]
fn rejects_entity_id_beyond_tile_index_range() {
    let mut tl = vec![TileMembership::None; TILE_COUNT];
    tl[0] = TileMembership::Single(GeoEntityId(1 << 31));
    let err = write_geo_data(
        &[entity(1 << 31, "AAA")],
        "iso",
        &tl,
        &BTreeMap::new(),
        [0u8; 32],
    )
    .unwrap_err();
    assert!(err.to_string().contains("entity id"), "{err}");
}
