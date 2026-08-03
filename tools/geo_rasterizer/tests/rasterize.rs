use std::collections::BTreeMap;
use std::path::Path;

use geo_data_format::TileMembership;
use geo_rasterizer::{
    admin0::parse_admin0, entities::assemble_entities, projection::lng_lat_to_block_xy,
    rasterize::rasterize, registry::Registry,
};

const SYNTHETIC_REGISTRY: &str = "tests/fixtures/synthetic_registry.toml";

#[test]
fn synthetic_polygons_classify_correctly() {
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model = assemble_entities(&features, &registry).unwrap();
    let country_ids: BTreeMap<String, geo_data_format::GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, geo_data_format::GeoEntityKind::Country))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let (tile_lookup, block_lookup) = rasterize(&model.geometry_for_country, &country_ids);

    // Pick a point inside AAA's box (lng 0..1, lat 0..1) — 0.5, 0.5.
    let (bx, by) = lng_lat_to_block_xy(0.5, 0.5);
    let tx = (bx / 128) as u16;
    let ty = (by / 128) as u16;
    let tile_idx = tx as usize * 512 + ty as usize;
    let blkx = (bx % 128) as u8;
    let blky = (by % 128) as u8;

    // The tile should be Border (AAA is 1°×1°, smaller than a tile),
    // and the block at (blkx, blky) should resolve to AAA's GeoEntityId.
    match &tile_lookup[tile_idx] {
        TileMembership::Border => {
            let blocks = block_lookup
                .get(&(tx, ty))
                .expect("border tile must have a block array");
            // Block lookup is x-major (`bxo * 128 + byo`), matching BlockKey::index().
            let block_idx = blkx as usize * 128 + blky as usize;
            let entity_id = model
                .entities
                .iter()
                .find(|e| e.canonical_code == "AAA")
                .unwrap()
                .id;
            assert_eq!(blocks[block_idx], entity_id.0);
        }
        other => panic!("expected Border for AAA tile, got {other:?}"),
    }
}

#[test]
fn deep_ocean_block_resolves_to_none() {
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new(SYNTHETIC_REGISTRY)).unwrap();
    let model = assemble_entities(&features, &registry).unwrap();
    let country_ids: BTreeMap<String, geo_data_format::GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, geo_data_format::GeoEntityKind::Country))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let (tile_lookup, _) = rasterize(&model.geometry_for_country, &country_ids);
    let (bx, by) = lng_lat_to_block_xy(-150.0, 0.0);
    let tx = (bx / 128) as u16;
    let ty = (by / 128) as u16;
    let tile_idx = tx as usize * 512 + ty as usize;
    assert!(matches!(tile_lookup[tile_idx], TileMembership::None));
}
