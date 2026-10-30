//! End-to-end admin-2 refinement over the synthetic fixture world: the tier
//! nests inside the province mask, and the leaves it wins were the province's.

use std::path::Path;
use std::process::Command;

use geo_data_format::{
    GeoData, GeoEntity, GeoEntityId, GeoEntityKind, PackedTile, TileEntry, CELLS_PER_TILE,
    TILE_COUNT,
};

fn rasterize(admin2_dir: &str, out: &Path) -> GeoData {
    let status = Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--worldview",
            "iso",
            "--countries",
            "tests/fixtures/synthetic.geojson",
            "--admin1",
            "tests/fixtures/synthetic_admin1.geojson",
            "--registry",
            "tests/fixtures/synthetic_registry",
            "--admin2-dir",
            admin2_dir,
            "--output",
        ])
        .arg(out)
        .status()
        .expect("run rasterizer");
    assert!(
        status.success(),
        "rasterizer failed with --admin2-dir {admin2_dir}"
    );
    GeoData::open(out).unwrap()
}

/// One tile's leaf ids, uniform tiles included. The synthetic world is far
/// smaller than a tile (each is 0.7° wide), so every populated tile is a
/// border tile and the refinement can only be seen per block.
enum Leaves {
    Empty,
    Uniform(GeoEntityId),
    Packed(PackedTile),
}

impl Leaves {
    fn at(&self, cell: usize) -> Option<GeoEntityId> {
        match self {
            Leaves::Empty => None,
            Leaves::Uniform(id) => Some(*id),
            Leaves::Packed(tile) => tile.lookup(cell),
        }
    }
}

fn leaves(data: &GeoData, tile: usize) -> Leaves {
    match data.tile_index.get(tile) {
        TileEntry::None => Leaves::Empty,
        TileEntry::Single(id) => Leaves::Uniform(id),
        TileEntry::Border(i) => Leaves::Packed(
            PackedTile::from_compressed_bytes(&data.border_blobs.get(i).unwrap()).unwrap(),
        ),
    }
}

fn find<'a>(entities: &'a [GeoEntity], code: &str) -> &'a GeoEntity {
    entities
        .iter()
        .find(|e| e.canonical_code == code)
        .unwrap_or_else(|| panic!("no entity `{code}` in the emitted bin"))
}

#[test]
fn admin2_refines_the_province_leaf_and_nests_under_it() {
    let dir = tempfile::tempdir().unwrap();
    let without = rasterize(
        "tests/fixtures/synthetic_admin2_empty",
        &dir.path().join("without_admin2.bin"),
    );
    let with = rasterize(
        "tests/fixtures/synthetic_admin2",
        &dir.path().join("with_admin2.bin"),
    );

    assert!(
        !without
            .entities()
            .unwrap()
            .iter()
            .any(|e| matches!(e.kind, GeoEntityKind::Admin2)),
        "an empty admin-2 snapshot must ship no admin-2 entity"
    );

    let with_entities = with.entities().unwrap();
    let city = find(&with_entities, "CN-4401");
    assert!(matches!(city.kind, GeoEntityKind::Admin2));
    assert_eq!(city.name_key, "admin2.CN-4401");
    assert_eq!(city.iso_a3_eh, None);

    let province = find(&with_entities, "AAA-001");
    assert!(matches!(province.kind, GeoEntityKind::Admin1));
    assert_eq!(city.parent_id, Some(province.id));

    // The tier nests: every block the admin-2 unit wins was already the
    // province's, so the province's total can only be the larger.
    assert!(city.total_area_m2 > 0, "the admin-2 unit won no block");
    assert!(province.total_area_m2 >= city.total_area_m2);

    // …and every block it won was its parent's: the coarse mask stays
    // authoritative, so an admin-2 unit can never take land off a sibling.
    let (mut from_parent, mut from_elsewhere) = (0u64, 0u64);
    for tile in 0..TILE_COUNT {
        let (before, after) = (leaves(&without, tile), leaves(&with, tile));
        if matches!(before, Leaves::Empty) && matches!(after, Leaves::Empty) {
            continue;
        }
        for cell in 0..CELLS_PER_TILE {
            if after.at(cell) != Some(city.id) {
                continue;
            }
            if before.at(cell) == Some(province.id) {
                from_parent += 1;
            } else {
                from_elsewhere += 1;
            }
        }
    }
    assert!(
        from_parent > 0,
        "no block flipped from the province to its admin-2 child"
    );
    assert_eq!(
        from_elsewhere, 0,
        "the admin-2 unit won blocks its parent province did not hold"
    );
}

/// A tier only ships where its parent does; an admin-2 unit whose declared
/// province this worldview drops is a build failure, not a silent orphan.
#[test]
fn an_admin2_unit_whose_parent_does_not_ship_fails_the_build() {
    let dir = tempfile::tempdir().unwrap();
    let admin2_dir = dir.path().join("admin2");
    std::fs::create_dir(&admin2_dir).unwrap();
    std::fs::copy(
        "tests/fixtures/synthetic_admin2/chn_admin2.geojson.zst",
        admin2_dir.join("chn_admin2.geojson.zst"),
    )
    .unwrap();
    // `CCC+99?` is Cland's sole admin-1 unit, so the coextensive-unit rule
    // drops it and nothing at admin-1 remains for prefix `44` to parent to.
    std::fs::write(
        admin2_dir.join("chn_province_map.toml"),
        "schema = 1\n\nprovince = [\n  { prefix = \"44\", adm1_code = \"CCC+99?\" },\n  \
         { prefix = \"46\", adm1_code = \"AAA-002\" },\n]\n",
    )
    .unwrap();

    let out = Command::new(env!("CARGO_BIN_EXE_geo_rasterizer"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "--worldview",
            "iso",
            "--countries",
            "tests/fixtures/synthetic.geojson",
            "--admin1",
            "tests/fixtures/synthetic_admin1.geojson",
            "--registry",
            "tests/fixtures/synthetic_registry",
            "--admin2-dir",
        ])
        .arg(&admin2_dir)
        .args(["--output"])
        .arg(dir.path().join("orphan.bin"))
        .output()
        .expect("run rasterizer");
    assert!(!out.status.success(), "expected the orphan parent to fail");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("CN-4401") && stderr.contains("CCC+99?"),
        "expected a failure naming the unit and its missing parent, got: {stderr}"
    );
}
