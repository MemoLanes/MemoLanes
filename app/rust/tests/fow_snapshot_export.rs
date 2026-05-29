use memolanes_core::export_data;
use memolanes_core::import_data;
use memolanes_core::journey_bitmap::{Block, BlockKey, JourneyBitmap, Tile, TileKey};
use memolanes_core::renderer::MapRenderer;
use memolanes_core::utils;
use std::collections::BTreeSet;
use std::io::Cursor;

pub mod test_utils;

fn fixture_bitmap() -> JourneyBitmap {
    let mut block = Block::new();
    block.set_point(0, 0, true);
    block.set_point(1, 0, true);
    block.set_point(63, 63, true);

    let mut tile = Tile::new();
    tile.set(&BlockKey::from_x_y(5, 7), block);

    let mut bitmap = JourneyBitmap::new();
    bitmap.insert_tile(&TileKey::new(10, 20), tile);
    bitmap
}

fn export_fixture_fwss() -> Vec<u8> {
    let bitmap = fixture_bitmap();
    let mut fwss = Cursor::new(Vec::new());
    export_data::journey_bitmap_to_fwss_file(&bitmap, &mut fwss).unwrap();
    fwss.into_inner()
}

fn zip_entry_names(fwss: &[u8]) -> BTreeSet<String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();
    (0..zip.len())
        .map(|i| zip.by_index(i).unwrap().name().to_string())
        .collect()
}

fn render_fixture_tile(bitmap: JourneyBitmap) -> Vec<u8> {
    let (left, top) = utils::tile_x_y_to_lng_lat(10, 20, 9);
    let (right, bottom) = utils::tile_x_y_to_lng_lat(11, 21, 9);
    let mut renderer = MapRenderer::new(bitmap);
    test_utils::render_map_overlay(&mut renderer, 9, left, top, right, bottom).data
}

#[test]
fn generated_fwss_uses_stored_zip_entries() {
    let fwss = export_fixture_fwss();
    let mut zip = zip::ZipArchive::new(Cursor::new(fwss)).unwrap();

    for i in 0..zip.len() {
        let file = zip.by_index(i).unwrap();
        assert_eq!(file.compression(), zip::CompressionMethod::Stored);
    }
}

#[test]
fn generated_fwss_has_expected_model_entries() {
    let fwss = export_fixture_fwss();
    let entry_names = zip_entry_names(&fwss);

    assert!(entry_names.contains("Model/*/183flohsowe"));
    assert!(entry_names.contains("Model/#/1bdalohsozd"));
    assert!(entry_names.contains("Model/#/01abfc750a"));
    assert!(entry_names.contains("Model/#/3389dae361"));

    let expected_layers = [
        "Model/~/42fehskskk",
        "Model/~/8c7bkjhdd",
        "Model/~/65delhixz",
        "Model/~/19cawhxk",
        "Model/~/e4daoew",
        "Model/~/1679oek",
        "Model/~/8f14oen",
        "Model/~/c9f0oem",
        "Model/~/45c4oeo",
        "Model/~/d3d9oie",
        "Model/~/6512oii",
        "Model/~/c20aoiz",
        "Model/~/c51coix",
        "Model/~/aab3oid",
        "Model/~/9bf3oiw",
    ];

    for layer in expected_layers {
        assert!(
            entry_names.contains(layer),
            "missing expected layer {layer}"
        );
    }

    assert_eq!(entry_names.len(), 19);
}

#[test]
fn generated_fwss_roundtrips_through_importer() {
    let fwss = export_fixture_fwss();
    let path = "./tests/for_inspection/generated_fow_snapshot_fixture.fwss";
    std::fs::write(path, fwss).unwrap();

    let (bitmap, warnings) = import_data::load_fow_snapshot_data(path).unwrap();
    assert!(bitmap.contains_tile(&TileKey::new(10, 20)));
    assert_eq!(format!("{warnings:?}"), "None");
}

#[test]
fn generated_fwss_roundtrip_preserves_rendered_bitmap() {
    let original_bitmap = fixture_bitmap();
    let original_render = render_fixture_tile(original_bitmap);

    let fwss = export_fixture_fwss();
    let path = "./tests/for_inspection/generated_fow_snapshot_render_roundtrip.fwss";
    std::fs::write(path, fwss).unwrap();

    let (imported_bitmap, warnings) = import_data::load_fow_snapshot_data(path).unwrap();
    let imported_render = render_fixture_tile(imported_bitmap);

    assert_eq!(format!("{warnings:?}"), "None");
    assert_eq!(original_render, imported_render);
}
