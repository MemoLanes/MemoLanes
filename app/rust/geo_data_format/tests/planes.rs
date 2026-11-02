use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    plane_tile_index, read_header, write_geo_data_planes, GeoData, GeoEntityId, PlaneRaster,
    TileEntry, TileMembership, CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};
use std::collections::BTreeMap;

#[test]
fn planes_have_independent_single_border_and_empty_tiles() {
    let tiles: Vec<_> = BitmapPlane::ALL
        .into_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut t = vec![TileMembership::None; (p.grid_width() * p.grid_width()) as usize];
            t[0] = TileMembership::Single(GeoEntityId(i as u32));
            t[1] = TileMembership::Border;
            t
        })
        .collect();
    let blocks: Vec<_> = (0..3)
        .map(|i| {
            let mut cells = vec![NO_ENTITY; CELLS_PER_TILE];
            cells[7] = i;
            BTreeMap::from([((0, 1), cells)])
        })
        .collect();
    let rasters: Vec<_> = BitmapPlane::ALL
        .into_iter()
        .enumerate()
        .map(|(i, p)| PlaneRaster {
            plane: p,
            tiles: &tiles[i],
            blocks: &blocks[i],
        })
        .collect();
    let bytes = write_geo_data_planes(&[], "iso", &rasters, [0; 32]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo.bin");
    std::fs::write(&path, bytes).unwrap();
    let geo = GeoData::open(&path).unwrap();
    for (i, p) in BitmapPlane::ALL.into_iter().enumerate() {
        assert_eq!(
            geo.tile_index.get(plane_tile_index(p, 0, 0).unwrap()),
            TileEntry::Single(GeoEntityId(i as u32))
        );
        let TileEntry::Border(b) = geo.tile_index.get(plane_tile_index(p, 0, 1).unwrap()) else {
            panic!()
        };
        let tile =
            geo_data_format::PackedTile::from_compressed_bytes(&geo.border_blobs.get(b).unwrap())
                .unwrap();
        assert_eq!(tile.lookup(7), Some(GeoEntityId(i as u32)));
        assert_eq!(
            geo.tile_index.get(plane_tile_index(p, 0, 2).unwrap()),
            TileEntry::None
        );
        assert!(plane_tile_index(p, p.grid_width() as u16, 0).is_none());
    }
}

#[test]
fn rejects_legacy_mercator_only_index_and_duplicate_planes() {
    let tiles = vec![TileMembership::None; TILE_COUNT];
    let blocks = BTreeMap::new();
    let raster = || PlaneRaster {
        plane: BitmapPlane::Mercator,
        tiles: &tiles,
        blocks: &blocks,
    };
    assert!(write_geo_data_planes(&[], "iso", &[raster(), raster()], [0; 32]).is_err());
    let mut bytes = write_geo_data_planes(&[], "iso", &[raster()], [0; 32]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("geo.bin");
    std::fs::write(&path, &bytes).unwrap();
    let h = read_header(&path).unwrap().unwrap();
    let legacy = zstd::encode_all(vec![0u8; TILE_COUNT * 5].as_slice(), 3).unwrap();
    let delta = legacy.len() as i64 - h.tile_index.len as i64;
    bytes.splice(
        h.tile_index.off..h.tile_index.off + h.tile_index.len,
        legacy.iter().copied(),
    );
    // Update section lengths/offsets so only the legacy index size is invalid.
    bytes[48..52].copy_from_slice(&(legacy.len() as u32).to_le_bytes());
    for pos in [52, 60] {
        let old = u32::from_le_bytes(bytes[pos..pos + 4].try_into().unwrap());
        bytes[pos..pos + 4].copy_from_slice(&((old as i64 + delta) as u32).to_le_bytes());
    }
    std::fs::write(&path, &bytes).unwrap();
    let err = GeoData::open(&path).unwrap_err();
    assert!(err.to_string().contains("tile index"), "{err}");
}
