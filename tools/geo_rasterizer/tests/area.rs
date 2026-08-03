use std::collections::BTreeMap;
use std::path::Path;

use geo_data_format::{GeoEntity, GeoEntityId, GeoEntityKind, TileMembership, NO_ENTITY};
use geo_rasterizer::admin0::parse_admin0;
use geo_rasterizer::area::populate_total_areas;
use geo_rasterizer::entities::{assemble_entities, EntityModel};
use geo_rasterizer::projection::block_area_m2;
use geo_rasterizer::rasterize::rasterize;
use geo_rasterizer::registry::Registry;

#[test]
fn synthetic_areas_are_positive_and_sum_consistently() {
    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new("tests/fixtures/synthetic_registry.toml")).unwrap();
    let mut model = assemble_entities(&features, &registry).unwrap();
    let country_ids: BTreeMap<String, GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let (tile_lookup, block_lookup) = rasterize(&model.geometry_for_country, &country_ids);
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
        let children = model
            .entities
            .iter()
            .filter(|e| e.parent_id == Some(cont.id))
            .count();
        let diff = cont.total_area_m2.abs_diff(sum);
        assert!(
            diff <= children as u64,
            "continent {} is {} but its children sum to {sum} — differs by {diff}, beyond the \
             one-m²-per-child rounding slack",
            cont.canonical_code,
            cont.total_area_m2,
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
        geometry_for_province: BTreeMap::new(),
        province_ids: BTreeMap::new(),
        province_declared_adm0: BTreeMap::new(),
    };

    let mut tile_lookup = vec![TileMembership::None; MAP_WIDTH * MAP_WIDTH];
    tile_lookup[tx * MAP_WIDTH + ty] = TileMembership::Border; // x-major tile index

    let mut cells = vec![NO_ENTITY; TILE_WIDTH * TILE_WIDTH];
    cells[lx * TILE_WIDTH + ly] = id.0; // x-major block index = x*128 + y
    let mut block_lookup: BTreeMap<(u16, u16), Vec<u32>> = BTreeMap::new();
    block_lookup.insert((tx as u16, ty as u16), cells);

    populate_total_areas(&mut model, &tile_lookup, &block_lookup);

    let expected = block_area_m2(0, (ty * TILE_WIDTH + ly) as i64).round() as u64;
    assert_eq!(
        model.entities[0].total_area_m2, expected,
        "border cell area must use its y row, not x"
    );
}

#[test]
fn country_area_is_the_sum_of_its_leaf_blocks_at_any_depth() {
    use geo_data_format::GeoEntityId;
    use geo_rasterizer::admin1::parse_admin1;
    use geo_rasterizer::entities::{
        attach_province_entities, collect_provinces, resolve_province_parents,
    };
    use geo_rasterizer::refine::{measure_coverage, refine_raster};
    use std::collections::BTreeMap;

    let features = parse_admin0(Path::new("tests/fixtures/synthetic.geojson"), "iso").unwrap();
    let registry = Registry::load(Path::new("tests/fixtures/synthetic_registry.toml")).unwrap();
    let mut model = assemble_entities(&features, &registry).unwrap();

    let country_ids: BTreeMap<String, GeoEntityId> = model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
        .map(|e| (e.canonical_code.clone(), e.id))
        .collect();
    let country_raster = rasterize(&model.geometry_for_country, &country_ids);

    let admin1 = parse_admin1(
        Path::new("tests/fixtures/synthetic_admin1.geojson"),
        geo_data_format::Worldview::Iso,
    )
    .unwrap();
    collect_provinces(&mut model, &admin1, &registry).unwrap();
    let province_raster = rasterize(&model.geometry_for_province, &model.province_ids);

    let tally = measure_coverage(
        (&country_raster.0, &country_raster.1),
        (&province_raster.0, &province_raster.1),
    );
    let parents =
        resolve_province_parents(&model, &tally, geo_data_format::Worldview::Iso).unwrap();
    attach_province_entities(&mut model, &parents);
    let (tile_lookup, block_lookup) = refine_raster(
        (&country_raster.0, &country_raster.1),
        (&province_raster.0, &province_raster.1),
        &parents,
    );
    populate_total_areas(&mut model, &tile_lookup, &block_lookup);

    let area_of = |id: GeoEntityId| {
        model
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .total_area_m2
    };
    for country in model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Country))
    {
        let children: Vec<u64> = model
            .entities
            .iter()
            .filter(|e| e.parent_id == Some(country.id))
            .map(|e| area_of(e.id))
            .collect();
        let child_sum: u64 = children.iter().sum();
        assert!(
            country.total_area_m2 >= child_sum,
            "{}: {} < sum of provinces {}",
            country.canonical_code,
            country.total_area_m2,
            child_sum
        );
        // Rounding is applied once per entity, so a parent may differ from the
        // sum of its already-rounded children by at most 1 m² per child.
        //
        // That tightness is a property of this fixture, not of the algorithm,
        // and the same caveat applies to the continent check below: the
        // fixture's entities are whole tiles, so almost every block credits
        // parent and child the same f64 in the same order. Splitting a
        // `Single` tile among provinces re-associates the sum, and on the real
        // sources the continent gap reaches 817 m² against a `children.len()`
        // bound of 52. Do not generalise this bound off the fixture.
        assert!(
            country.total_area_m2 - child_sum <= children.len() as u64 || children.is_empty(),
            "{}: gap {} exceeds rounding slack",
            country.canonical_code,
            country.total_area_m2 - child_sum
        );
    }

    for province in model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Province))
    {
        assert!(
            province.total_area_m2 > 0,
            "{} should have nonzero area",
            province.canonical_code
        );
    }

    // The roll-up must not stop at the first hop: a continent's total has to
    // reflect the same province-refined blocks as its countries, not just
    // whatever a one-hop credit would leave behind.
    for continent in model
        .entities
        .iter()
        .filter(|e| matches!(e.kind, GeoEntityKind::Continent))
    {
        let children: Vec<u64> = model
            .entities
            .iter()
            .filter(|e| e.parent_id == Some(continent.id))
            .map(|e| area_of(e.id))
            .collect();
        let child_sum: u64 = children.iter().sum();
        let diff = continent.total_area_m2.abs_diff(child_sum);
        assert!(
            diff <= children.len() as u64,
            "{}: {} but children sum to {child_sum} — differs by {diff}, beyond the \
             one-m²-per-child rounding slack",
            continent.canonical_code,
            continent.total_area_m2,
        );
    }
}
