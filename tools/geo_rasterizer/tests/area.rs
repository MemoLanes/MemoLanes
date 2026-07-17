use std::collections::BTreeMap;
use std::path::Path;

use geo_data_format::{GeoEntity, GeoEntityId, GeoEntityKind, TileMembership};
use geo_rasterizer::area::populate_total_areas;
use geo_rasterizer::entities::{assemble_entities, EntityModel};
use geo_rasterizer::parse::parse_geojson;
use geo_rasterizer::projection::block_area_m2;
use geo_rasterizer::rasterize::rasterize;
use geo_rasterizer::registry::Registry;

#[test]
fn synthetic_areas_are_positive_and_sum_consistently() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new("tests/fixtures/synthetic_registry.toml")).unwrap();
    let mut model = assemble_entities(&features, &registry).unwrap();
    let (tile_lookup, block_lookup) = rasterize(&features, &model);
    populate_total_areas(&mut model, &tile_lookup, &block_lookup);

    // Each country gets a positive area.
    for e in &model.entities {
        if matches!(e.kind, GeoEntityKind::Country) {
            assert!(
                e.total_area_m2 > 0,
                "{} should have nonzero area",
                e.canonical_code
            );
        }
    }
    // Each continent's area is the sum of its child countries' areas.
    for cont in model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Continent))
    {
        let sum: u64 = model
            .entities
            .iter()
            .filter(|e| e.parent_id == Some(cont.id))
            .map(|e| e.total_area_m2)
            .sum();
        assert_eq!(
            cont.total_area_m2, sum,
            "continent {} area mismatch",
            cont.canonical_code
        );
    }
}

/// A single border cell must be weighted by its *latitude row* (y), not x.
/// `block_area_m2` depends only on y, so placing one cell at (x≠y) in a
/// high-latitude tile distinguishes correct (y-row) from a transposed
/// (x-row) weighting. Guards `area.rs`'s x-major block indexing.
#[test]
fn border_cell_area_weighted_by_latitude_row() {
    const MAP_WIDTH: usize = 512;
    const TILE_WIDTH: usize = 128;
    let (tx, ty, lx, ly) = (10usize, 400usize, 10usize, 120usize); // x != y, high latitude

    let id = GeoEntityId(2);
    let mut model = EntityModel {
        entities: vec![GeoEntity {
            id,
            kind: GeoEntityKind::Country,
            canonical_code: "AAA".into(),
            iso_a3_eh: None,
            name_key: "k.AAA".into(),
            parent_id: None,
            total_area_m2: 0,
        }],
        geometry_for_country: BTreeMap::new(),
    };

    let mut tile_lookup = vec![TileMembership::None; MAP_WIDTH * MAP_WIDTH];
    tile_lookup[tx * MAP_WIDTH + ty] = TileMembership::Border; // x-major tile index

    let mut cells = vec![None; TILE_WIDTH * TILE_WIDTH];
    cells[lx * TILE_WIDTH + ly] = Some(id); // x-major block index = x*128 + y
    let mut block_lookup: BTreeMap<(u16, u16), Vec<Option<GeoEntityId>>> = BTreeMap::new();
    block_lookup.insert((tx as u16, ty as u16), cells);

    populate_total_areas(&mut model, &tile_lookup, &block_lookup);

    let expected = block_area_m2(0, (ty * TILE_WIDTH + ly) as i64).round() as u64;
    assert_eq!(
        model.entities[0].total_area_m2, expected,
        "border cell area must use its y row, not x"
    );
}
