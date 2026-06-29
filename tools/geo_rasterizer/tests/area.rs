use std::path::Path;

use geo_data_format::GeoEntityKind;
use geo_rasterizer::area::populate_total_areas;
use geo_rasterizer::entities::assemble_entities;
use geo_rasterizer::parse::parse_geojson;
use geo_rasterizer::rasterize::rasterize;
use geo_rasterizer::registry::Registry;

#[test]
fn synthetic_areas_are_positive_and_sum_consistently() {
    let features = parse_geojson(Path::new("tests/fixtures/synthetic.geojson")).unwrap();
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
                e.iso_code
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
            cont.iso_code
        );
    }
}
