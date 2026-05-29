use memolanes_core::export_data;
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::{JourneyBitmap, TileKey};
use memolanes_core::renderer::MapRenderer;
use memolanes_core::utils;
use std::collections::BTreeSet;
use std::fs::File;
use std::io::Cursor;

pub mod test_utils;

const SOURCE_SNAPSHOT_PATH: &str = "./tests/data/snapshot_test.fwss";
const EXPORTED_SNAPSHOT_PATH: &str = "./tests/for_inspection/generated_fow_snapshot.fwss";

fn load_source_bitmap() -> JourneyBitmap {
    let (bitmap, warnings) = import_data::load_fow_snapshot_data(SOURCE_SNAPSHOT_PATH).unwrap();
    assert_eq!(format!("{warnings:?}"), "None");
    bitmap
}

fn export_source_snapshot() -> Vec<u8> {
    let bitmap = load_source_bitmap();
    let mut fwss = Cursor::new(Vec::new());
    export_data::journey_bitmap_to_fwss_file(&bitmap, &mut fwss).unwrap();
    fwss.into_inner()
}

fn zip_entry_names_from_path(path: &str) -> BTreeSet<String> {
    let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect()
}

fn zip_entry_names_from_bytes(fwss: &[u8]) -> BTreeSet<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect()
}

fn render_first_tile(bitmap: JourneyBitmap) -> Vec<u8> {
    let tile_key = bitmap
        .all_tile_keys()
        .min_by_key(|tile_key| (tile_key.y, tile_key.x))
        .copied()
        .unwrap_or_else(|| panic!("snapshot fixture should contain at least one tile"));
    render_tile(bitmap, tile_key)
}

fn render_tile(bitmap: JourneyBitmap, tile_key: TileKey) -> Vec<u8> {
    let (left, top) = utils::tile_x_y_to_lng_lat(tile_key.x as i32, tile_key.y as i32, 9);
    let (right, bottom) =
        utils::tile_x_y_to_lng_lat(tile_key.x as i32 + 1, tile_key.y as i32 + 1, 9);
    let mut renderer = MapRenderer::new(bitmap);
    test_utils::render_map_overlay(&mut renderer, 9, left, top, right, bottom).data
}

#[test]
fn generated_fwss_uses_stored_zip_entries() {
    let fwss = export_source_snapshot();
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();

    for i in 0..zip.len() {
        let file = zip.by_index(i).unwrap();
        assert_eq!(file.compression(), zip::CompressionMethod::Stored);
    }
}

#[test]
fn generated_fwss_preserves_official_snapshot_model_entries() {
    let generated_fwss = export_source_snapshot();
    assert_eq!(
        zip_entry_names_from_bytes(&generated_fwss),
        zip_entry_names_from_path(SOURCE_SNAPSHOT_PATH)
    );
}

#[test]
fn generated_fwss_roundtrips_through_importer() {
    let bitmap = load_source_bitmap();
    let fwss = export_source_snapshot();
    std::fs::write(EXPORTED_SNAPSHOT_PATH, fwss).unwrap();

    let (roundtripped_bitmap, warnings) =
        import_data::load_fow_snapshot_data(EXPORTED_SNAPSHOT_PATH).unwrap();
    assert_eq!(bitmap, roundtripped_bitmap);
    assert_eq!(format!("{warnings:?}"), "None");
}

#[test]
fn generated_fwss_roundtrip_preserves_rendered_bitmap() {
    let bitmap = load_source_bitmap();
    let original_render = render_first_tile(bitmap.clone());

    let fwss = export_source_snapshot();
    std::fs::write(EXPORTED_SNAPSHOT_PATH, fwss).unwrap();

    let (roundtripped_bitmap, warnings) =
        import_data::load_fow_snapshot_data(EXPORTED_SNAPSHOT_PATH).unwrap();
    let roundtripped_render = render_first_tile(roundtripped_bitmap);

    assert_eq!(format!("{warnings:?}"), "None");
    assert_eq!(original_render, roundtripped_render);
}
