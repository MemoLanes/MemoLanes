use std::collections::BTreeMap;

use bitmap_geometry::BitmapPlane;
use geo_data_format::{
    write_geo_data_planes, GeoEntity, GeoEntityId, GeoEntityKind, PlaneRaster, TileMembership,
    CELLS_PER_TILE, NO_ENTITY,
};
use memolanes_core::{
    geo::{GeoIndex, GeoLookup},
    journey_bitmap::{BlockKey, TileKey},
};

#[test]
fn runtime_looks_up_all_planes_without_aliasing() {
    let entities: Vec<_> = (0..3)
        .map(|i| GeoEntity {
            id: GeoEntityId(i),
            kind: GeoEntityKind::Admin0,
            canonical_code: format!("C{i}"),
            iso_a3_eh: None,
            name_key: format!("c.{i}"),
            parent_id: None,
            total_area_m2: 1,
        })
        .collect();
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
            let mut c = vec![NO_ENTITY; CELLS_PER_TILE];
            c[BlockKey::from_x_y(3, 7).index()] = i;
            BTreeMap::from([((0, 1), c)])
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
    let bytes = write_geo_data_planes(&entities, "iso", &rasters, [0; 32]).unwrap();
    let dir = tempdir::TempDir::new("geo_polar").unwrap();
    let path = dir.path().join("geo.bin");
    std::fs::write(&path, bytes).unwrap();
    let geo = GeoIndex::open(&path).unwrap();
    for (i, p) in BitmapPlane::ALL.into_iter().enumerate() {
        assert_eq!(
            geo.entity_of_block(TileKey::new(p, 0, 0), BlockKey::from_x_y(0, 0))
                .unwrap(),
            Some(GeoEntityId(i as u32))
        );
        assert_eq!(
            geo.entity_of_block(TileKey::new(p, 0, 1), BlockKey::from_x_y(3, 7))
                .unwrap(),
            Some(GeoEntityId(i as u32))
        );
        assert_eq!(
            geo.entity_of_block(TileKey::new(p, 0, 1), BlockKey::from_x_y(7, 3))
                .unwrap(),
            None
        );
        assert_eq!(
            geo.tile_membership(TileKey::new(p, p.grid_width() as u16, 0)),
            TileMembership::None
        );
        assert_eq!(
            geo.tile_membership(TileKey::new(p, 0, u16::MAX)),
            TileMembership::None
        );
    }
}
