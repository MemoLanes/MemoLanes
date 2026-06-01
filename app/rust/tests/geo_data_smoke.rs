//! End-to-end smoke: load the real `app/assets/geo_data_iso.bin` and assert
//! that landmark coordinates resolve to the expected country ADM0_A3.

use std::path::PathBuf;

use memolanes_core::achievement::geo_lookup::GeoLookupTable;

fn load_table() -> GeoLookupTable {
    let path: PathBuf = std::env::var("GEO_DATA_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            // CARGO_MANIFEST_DIR is `app/rust`; bin is at `../assets/geo_data_iso.bin`.
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("assets")
                .join("geo_data_iso.bin")
        });
    let bytes = std::fs::read(&path).unwrap_or_else(|e| {
        panic!("failed to read {}: {e}", path.display());
    });
    GeoLookupTable::load_from_bytes(&bytes).expect("geo_data_iso.bin should load")
}

/// Projection inlined here to avoid coupling the integration test to
/// `memolanes_core::utils` visibility.
fn lng_lat_to_block_xy(lng: f64, lat: f64) -> (i32, i32) {
    use std::f64::consts::PI;
    let n = f64::powi(2.0, 16);
    let lat_rad = lat.to_radians();
    let x = ((lng + 180.0) / 360.0) * n;
    let y = (1.0 - ((lat_rad.tan() + 1.0 / lat_rad.cos()).ln() / PI)) / 2.0 * n;
    (x.floor() as i32, y.floor() as i32)
}

fn iso_at(table: &GeoLookupTable, lng: f64, lat: f64) -> Option<String> {
    let (block_x, block_y) = lng_lat_to_block_xy(lng, lat);
    let tile_x = (block_x / 128) as u16;
    let tile_y = (block_y / 128) as u16;
    let block_x_in_tile = (block_x % 128) as u8;
    let block_y_in_tile = (block_y % 128) as u8;
    let entity_id = table.lookup(tile_x, tile_y, block_x_in_tile, block_y_in_tile)?;
    table.get_entity(entity_id).map(|e| e.iso_code.clone())
}

#[test]
fn eiffel_tower_resolves_to_fra() {
    let t = load_table();
    assert_eq!(iso_at(&t, 2.2945, 48.8584).as_deref(), Some("FRA"));
}

#[test]
fn times_square_resolves_to_usa() {
    let t = load_table();
    assert_eq!(iso_at(&t, -73.9855, 40.7580).as_deref(), Some("USA"));
}

#[test]
fn mount_fuji_resolves_to_jpn() {
    let t = load_table();
    assert_eq!(iso_at(&t, 138.7274, 35.3606).as_deref(), Some("JPN"));
}

#[test]
fn sydney_opera_house_resolves_to_aus() {
    let t = load_table();
    assert_eq!(iso_at(&t, 151.2153, -33.8568).as_deref(), Some("AUS"));
}

#[test]
fn russia_far_east_resolves_to_rus_on_both_sides_of_180() {
    let t = load_table();
    // Magadan area (eastern Russia, well inland on lng > 0 side).
    assert_eq!(iso_at(&t, 150.78, 59.56).as_deref(), Some("RUS"));
    // Far east of Chukotka (lng < 0 after the antimeridian split).
    assert_eq!(iso_at(&t, -173.0, 65.0).as_deref(), Some("RUS"));
}

#[test]
fn pacific_open_ocean_resolves_to_none() {
    let t = load_table();
    assert!(iso_at(&t, -150.0, 0.0).is_none());
}

#[test]
fn russia_total_area_within_5pct() {
    use memolanes_core::achievement::geo_entity::{GeoEntity, GeoEntityId};
    let t = load_table();
    let _: &GeoEntity; // type proof
    let rus = (0..u32::MAX)
        .map_while(|i| t.get_entity(GeoEntityId(i)))
        .find(|e| e.iso_code == "RUS")
        .expect("RUS entity should exist");
    let expected = 17_098_242_000_000_u64; // ≈ 17.1 M km²
    let lo = (expected as f64 * 0.95) as u64;
    let hi = (expected as f64 * 1.05) as u64;
    assert!(
        rus.total_area_m2 >= lo && rus.total_area_m2 <= hi,
        "RUS total_area_m2 = {} not within ±5% of {expected}",
        rus.total_area_m2
    );
}

#[test]
fn sum_of_country_areas_within_5pct_of_earth_land() {
    use memolanes_core::achievement::geo_entity::{GeoEntityId, GeoEntityKind};
    let t = load_table();
    let mut sum: u64 = 0;
    let mut i = 0u32;
    while let Some(e) = t.get_entity(GeoEntityId(i)) {
        if matches!(e.kind, GeoEntityKind::Country) {
            sum += e.total_area_m2;
        }
        i += 1;
    }
    // ≈ 149 M km² Earth land area. Tolerance widened to ±5% to absorb
    // coastline aliasing at ~611 m block resolution and the small set of
    // tiles that fall on either side of the Single/Border classification
    // boundary. The area-helper formula is checked separately by
    // `russia_total_area_within_5pct`; this test is an internal-consistency
    // gate against gross under/over-counting.
    let expected = 149_000_000_000_000_u64;
    let lo = (expected as f64 * 0.95) as u64;
    let hi = (expected as f64 * 1.05) as u64;
    assert!(
        sum >= lo && sum <= hi,
        "sum of country total_area_m2 = {sum} not within ±5% of {expected}"
    );
}
