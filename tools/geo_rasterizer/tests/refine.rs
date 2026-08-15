use std::collections::BTreeMap;

use geo_data_format::{
    tile_index, GeoEntityId, TileMembership, CELLS_PER_TILE, NO_ENTITY, TILE_COUNT,
};
use geo_rasterizer::rasterize::{BlockLookup, TileLookup};
use geo_rasterizer::refine::{block_majority, measure_coverage, refine_raster};

const FR: GeoEntityId = GeoEntityId(1);
const DE: GeoEntityId = GeoEntityId(2);
const IDF: GeoEntityId = GeoEntityId(10); // a province of FR
const BAV: GeoEntityId = GeoEntityId(11); // a province of DE

fn empty() -> (TileLookup, BlockLookup) {
    (vec![TileMembership::None; TILE_COUNT], BlockLookup::new())
}

/// Tile (0,0) entirely `id`.
fn set_single(lookup: &mut (TileLookup, BlockLookup), tx: u16, ty: u16, id: GeoEntityId) {
    lookup.0[tile_index(tx, ty)] = TileMembership::Single(id);
}

/// Tile (tx,ty) split down the middle: first half `a`, second half `b`.
fn set_border(lookup: &mut (TileLookup, BlockLookup), tx: u16, ty: u16, a: u32, b: u32) {
    lookup.0[tile_index(tx, ty)] = TileMembership::Border;
    let cells: Vec<u32> = (0..CELLS_PER_TILE)
        .map(|i| if i < CELLS_PER_TILE / 2 { a } else { b })
        .collect();
    lookup.1.insert((tx, ty), cells);
}

#[test]
fn the_tally_counts_each_province_against_every_country_it_touches() {
    let mut country = empty();
    // Province IDF straddles the border: 3 tiles of FR, 1 tile of DE.
    for tx in 0..3u16 {
        set_single(&mut country, tx, 0, FR);
    }
    set_single(&mut country, 3, 0, DE);

    let mut province = empty();
    for tx in 0..4u16 {
        set_single(&mut province, tx, 0, IDF);
    }

    let tally = measure_coverage((&country.0, &country.1), (&province.0, &province.1));
    assert_eq!(tally[IDF.0 as usize][&FR], 3 * CELLS_PER_TILE as u64);
    assert_eq!(tally[IDF.0 as usize][&DE], CELLS_PER_TILE as u64);
    assert_eq!(block_majority(&tally[IDF.0 as usize]), Some(FR));
}

#[test]
fn the_tally_ignores_province_blocks_no_country_covers() {
    let mut country = empty();
    set_single(&mut country, 0, 0, FR);

    let mut province = empty();
    set_single(&mut province, 0, 0, IDF);
    set_single(&mut province, 1, 0, IDF); // out over open water

    let tally = measure_coverage((&country.0, &country.1), (&province.0, &province.1));
    assert_eq!(tally[IDF.0 as usize].len(), 1);
    assert_eq!(tally[IDF.0 as usize][&FR], CELLS_PER_TILE as u64);
}

#[test]
fn a_tie_resolves_to_the_lowest_country_id() {
    let mut country = empty();
    // Country tile is split exactly in half between FR and DE: a genuine tie.
    set_border(&mut country, 0, 0, FR.0, DE.0);

    let mut province = empty();
    // Province covers the whole tile, so it overlaps FR and DE equally.
    set_single(&mut province, 0, 0, IDF);

    let tally = measure_coverage((&country.0, &country.1), (&province.0, &province.1));
    assert_eq!(
        block_majority(&tally[IDF.0 as usize]),
        Some(FR),
        "a genuine tie must resolve to the lower country id (FR=1 < DE=2)"
    );
}

#[test]
fn a_province_spilling_across_a_border_is_clipped() {
    let mut country = empty();
    set_single(&mut country, 0, 0, FR);
    set_single(&mut country, 1, 0, DE);

    let mut province = empty();
    set_single(&mut province, 0, 0, IDF);
    set_single(&mut province, 1, 0, IDF); // spills into DE

    let parents = BTreeMap::from([(IDF, FR)]);
    let (tiles, _) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );

    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Single(IDF));
    assert_eq!(
        tiles[tile_index(1, 0)],
        TileMembership::Single(DE),
        "IDF is not a province of DE, so DE's own mask must win"
    );
}

#[test]
fn a_country_border_tile_clips_the_province_per_cell() {
    let mut country = empty();
    // Country tile is itself a border: first half FR, second half DE.
    set_border(&mut country, 0, 0, FR.0, DE.0);

    let mut province = empty();
    // Province covers the whole tile.
    set_single(&mut province, 0, 0, IDF);

    let parents = BTreeMap::from([(IDF, FR)]);
    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Border);
    let cells = &blocks[&(0, 0)];
    assert_eq!(
        cells[0], IDF.0,
        "cell on FR's half takes the province, since IDF's parent is FR"
    );
    assert_eq!(
        cells[CELLS_PER_TILE / 2 - 1],
        IDF.0,
        "cell on FR's half takes the province, since IDF's parent is FR"
    );
    assert_eq!(
        cells[CELLS_PER_TILE / 2],
        DE.0,
        "cell on DE's half stays DE, since IDF is not DE's province"
    );
    assert_eq!(
        cells[CELLS_PER_TILE - 1],
        DE.0,
        "cell on DE's half stays DE, since IDF is not DE's province"
    );
}

#[test]
fn a_country_border_tile_keeps_ocean_cells_empty_under_a_province() {
    let mut country = empty();
    // Country tile is itself a border: first half FR, second half open ocean.
    set_border(&mut country, 0, 0, FR.0, NO_ENTITY);

    let mut province = empty();
    // Province polygon covers the whole tile, including the ocean half.
    set_single(&mut province, 0, 0, IDF);

    let parents = BTreeMap::from([(IDF, FR)]);
    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Border);
    let cells = &blocks[&(0, 0)];
    assert_eq!(cells[0], IDF.0, "cell on FR's half takes the province");
    assert_eq!(
        cells[CELLS_PER_TILE / 2 - 1],
        IDF.0,
        "cell on FR's half takes the province"
    );
    assert_eq!(
        cells[CELLS_PER_TILE / 2],
        NO_ENTITY,
        "ocean cell stays empty: a province polygon over water must not paint land"
    );
    assert_eq!(
        cells[CELLS_PER_TILE - 1],
        NO_ENTITY,
        "ocean cell stays empty: a province polygon over water must not paint land"
    );
}

#[test]
fn a_tile_wholly_inside_one_province_stays_single() {
    let mut country = empty();
    set_single(&mut country, 0, 0, FR);
    let mut province = empty();
    set_single(&mut province, 0, 0, IDF);

    let parents = BTreeMap::from([(IDF, FR)]);
    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Single(IDF));
    assert!(
        blocks.is_empty(),
        "a uniform tile must not become a border tile"
    );
}

#[test]
fn uncovered_country_blocks_fall_back_to_the_country() {
    let mut country = empty();
    set_single(&mut country, 0, 0, FR);
    let mut province = empty();
    // Only the first half of the tile belongs to IDF.
    set_border(&mut province, 0, 0, IDF.0, NO_ENTITY);

    let parents = BTreeMap::from([(IDF, FR)]);
    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Border);
    let cells = &blocks[&(0, 0)];
    assert_eq!(cells[0], IDF.0);
    assert_eq!(
        cells[CELLS_PER_TILE - 1],
        FR.0,
        "land inside the country but in no province stays with the country"
    );
}

/// The empty-province fast path must not lose the collapse a full merge would
/// have done: a country border tile that happens to hold one id in every cell
/// still becomes `Single`.
#[test]
fn a_uniform_country_border_tile_collapses_even_with_no_province() {
    let mut country = empty();
    set_border(&mut country, 0, 0, FR.0, FR.0);
    let province = empty();

    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &BTreeMap::new(),
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Single(FR));
    assert!(blocks.is_empty());
}

#[test]
fn a_country_border_tile_survives_untouched_with_no_province() {
    let mut country = empty();
    set_border(&mut country, 0, 0, FR.0, DE.0);
    let province = empty();

    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &BTreeMap::new(),
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::Border);
    assert_eq!(blocks[&(0, 0)], country.1[&(0, 0)]);
}

#[test]
fn ocean_stays_ocean_even_under_a_province_polygon() {
    let country = empty(); // no country anywhere
    let mut province = empty();
    set_single(&mut province, 0, 0, BAV);

    let parents = BTreeMap::from([(BAV, DE)]);
    let (tiles, blocks) = refine_raster(
        (&country.0, &country.1),
        (&province.0, &province.1),
        &parents,
    );
    assert_eq!(tiles[tile_index(0, 0)], TileMembership::None);
    assert!(blocks.is_empty());
}
