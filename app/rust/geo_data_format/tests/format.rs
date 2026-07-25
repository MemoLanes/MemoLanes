use std::collections::BTreeMap;

use geo_data_format::{
    read_geo_data, write_geo_data, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile, TileEntry,
    TileMembership, CELLS_PER_TILE, TILE_COUNT,
};

fn entity(id: u32, iso: &str) -> GeoEntity {
    GeoEntity {
        id: GeoEntityId(id),
        kind: GeoEntityKind::Country,
        canonical_code: iso.into(),
        iso_a3_eh: Some(iso.into()),
        name_key: format!("c.{iso}"),
        parent_id: None,
        total_area_m2: 1,
    }
}

#[test]
fn round_trip_single_border_none() {
    let mut tl = vec![TileMembership::None; TILE_COUNT];
    tl[0] = TileMembership::Single(GeoEntityId(7));
    tl[1] = TileMembership::Border; // x-major: tile idx 1 → tx=0, ty=1
    let mut cells = vec![None; CELLS_PER_TILE];
    cells[5] = Some(GeoEntityId(7));
    let mut bl: BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>> = BTreeMap::new();
    bl.insert((0, 1), cells);

    let bytes = write_geo_data(&[entity(7, "AAA")], "iso", &tl, &bl, [3u8; 32]).unwrap();
    let gd = read_geo_data(&bytes).unwrap();

    assert_eq!(gd.provenance_hash, [3u8; 32]);
    assert_eq!(gd.worldview_id, "iso");
    assert_eq!(gd.entities.len(), 1);
    assert_eq!(gd.entities[0].canonical_code, "AAA");
    assert_eq!(gd.tile_index[0], TileEntry::Single(GeoEntityId(7)));
    assert_eq!(gd.tile_index[1], TileEntry::Border(0));
    assert!(matches!(gd.tile_index[2], TileEntry::None));
    assert_eq!(gd.border_blobs.len(), 1);
    let pt = PackedTile::from_compressed_bytes(&gd.border_blobs[0]);
    assert_eq!(pt.lookup(5), Some(GeoEntityId(7)));
    assert_eq!(pt.lookup(6), None);
}

#[test]
fn rejects_bad_magic() {
    let tl = vec![TileMembership::None; TILE_COUNT];
    let mut b = write_geo_data(&[], "iso", &tl, &BTreeMap::new(), [0u8; 32]).unwrap();
    b[0] = b'X';
    assert!(read_geo_data(&b).is_err());
}

#[test]
fn rejects_wrong_tile_count() {
    let tl = vec![TileMembership::None; 10];
    assert!(write_geo_data(&[], "iso", &tl, &BTreeMap::new(), [0u8; 32]).is_err());
}
